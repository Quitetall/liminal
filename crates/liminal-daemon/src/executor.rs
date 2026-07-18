//! File-system external executor for ILRP (M02 Algorithm C; M04.5).
//!
//! `FsExecutor` implements `ExternalExecutor` against real files in a workspace
//! root, using `liminal-source::{stage, observe}` for hash-verified staged
//! writes. It handles both file-holder step kinds:
//!
//! - `WriteFile` carries the full new bytes (M02).
//! - `InsertSourceId` (M04.5) reads the file, resolves the step's `entity` to
//!   its `{#alias}` via the `entity → alias` snapshot captured in
//!   [`FsExecutor::with_store`] (AM-4.1), inserts ` {#alias}` at the marker
//!   offset, and stages the result. The D04.4 graph-before-file ordering rule
//!   is enforced in `IlrpDriver::prepare`, not here.

use camino::{Utf8Path, Utf8PathBuf};
use std::collections::BTreeMap;

use liminal_graph::GraphStore;
use liminal_id::{EntityId, Timestamp};
use liminal_jurisdiction::ProposedMutation;
use liminal_jurisdiction::{
    ExternalExecutor, IlrpError, PrestateMatch, RepairOperation, StatePredicate, StepAck,
};
use liminal_source::{self as file, FileObservation};

/// File-system executor: stages, commits, and verifies against `root`.
///
/// Carries an `entity → alias` snapshot of `JUR_ALIAS` captured at construction
/// (AM-4.1: the executor needs to resolve which durable id an `InsertSourceId`
/// step serializes). Rebuilt fresh per exec/recover, so it is current.
#[derive(Debug)]
pub struct FsExecutor {
    root: Utf8PathBuf,
    entity_alias: BTreeMap<EntityId, String>,
}

impl FsExecutor {
    /// Create an executor rooted at `root` with an empty alias map (WriteFile
    /// steps do not need it).
    #[must_use]
    pub fn new(root: Utf8PathBuf) -> Self {
        Self {
            root,
            entity_alias: BTreeMap::new(),
        }
    }

    /// Create an executor whose alias map is a snapshot of `JUR_ALIAS` in
    /// `store` (needed for `InsertSourceId` execution).
    pub fn with_store(root: Utf8PathBuf, store: &GraphStore) -> Result<Self, IlrpError> {
        let mut entity_alias = BTreeMap::new();
        for (alias, value) in store
            .scan_aux(liminal_graph::ns::JUR_ALIAS)
            .map_err(IlrpError::Store)?
        {
            if let Some(entity_str) = value.get("entity").and_then(|v| v.as_str())
                && let Ok(entity) = entity_str.parse::<EntityId>()
            {
                entity_alias.insert(entity, alias);
            }
        }
        Ok(Self { root, entity_alias })
    }

    /// Absolute path from a workspace-relative path.
    fn abs(&self, path: &Utf8Path) -> Utf8PathBuf {
        self.root.join(path)
    }
}

/// Observe a file and convert to a StatePredicate match.
fn matches_predicate(pred: &StatePredicate, obs: Option<&FileObservation>) -> bool {
    match (pred, obs) {
        (StatePredicate::FileContent { hash, .. }, Some(o)) => o.hash == *hash,
        (StatePredicate::FileAbsent { .. } | StatePredicate::FileContent { .. }, None) => {
            matches!(pred, StatePredicate::FileAbsent { .. })
        }
        (StatePredicate::Any, _) => true,
        _ => false,
    }
}

/// The target file path of a file-holder step (WriteFile / InsertSourceId).
fn step_path(op: &RepairOperation) -> Result<&liminal_id::PathId, IlrpError> {
    match op {
        RepairOperation::WriteFile { path, .. } | RepairOperation::InsertSourceId { path, .. } => {
            Ok(path)
        }
        RepairOperation::External { .. } => Err(IlrpError::Executor(
            "External deferred to a later phase".into(),
        )),
        RepairOperation::Graph(_) => Err(IlrpError::Executor(
            "non-file predicate in file step".into(),
        )),
    }
}

impl FsExecutor {
    /// Compute the new bytes an `InsertSourceId` step produces: insert
    /// ` {#<alias>}` at the marker byte offset (M04 Algorithm A).
    fn insert_source_id_bytes(
        &self,
        bytes: &[u8],
        at: liminal_source::SourceRange,
        entity: EntityId,
    ) -> Result<Vec<u8>, IlrpError> {
        let alias = self
            .entity_alias
            .get(&entity)
            .ok_or_else(|| IlrpError::Executor(format!("no alias for entity {entity}")))?;
        let split = usize::try_from(at.start)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        let mut out = Vec::with_capacity(bytes.len() + alias.len() + 4);
        out.extend_from_slice(&bytes[..split]);
        out.extend_from_slice(format!(" {{#{alias}}}").as_bytes());
        out.extend_from_slice(&bytes[split..]);
        Ok(out)
    }
}

impl ExternalExecutor for FsExecutor {
    fn verify(&self, mutation: &ProposedMutation) -> Result<PrestateMatch, IlrpError> {
        let path = step_path(&mutation.operation)?;
        let abs = self.abs(&path.0);
        let obs = file::observe(&abs).map_err(|e| IlrpError::Executor(e.to_string()))?;

        // Prestate checked FIRST — if pre and post both match, re-apply wins.
        if matches_predicate(&mutation.expected_prestate, obs.as_ref()) {
            return Ok(PrestateMatch::Prestate);
        }
        if matches_predicate(&mutation.expected_poststate, obs.as_ref()) {
            return Ok(PrestateMatch::Poststate);
        }
        Ok(PrestateMatch::Neither)
    }

    fn apply(&self, mutation: &ProposedMutation) -> Result<StepAck, IlrpError> {
        // Materialize the bytes this step writes (WriteFile carries them;
        // InsertSourceId computes them from the marker offset + alias).
        let (path, contents) = match &mutation.operation {
            RepairOperation::WriteFile { path, contents } => (path.clone(), contents.clone()),
            RepairOperation::InsertSourceId { path, at, entity } => {
                let abs = self.abs(&path.0);
                let bytes = std::fs::read(&abs).map_err(|e| IlrpError::Executor(e.to_string()))?;
                let new_bytes = self.insert_source_id_bytes(&bytes, *at, *entity)?;
                (path.clone(), new_bytes)
            }
            _ => {
                return Err(IlrpError::Executor(
                    "only WriteFile / InsertSourceId are file steps".into(),
                ));
            }
        };

        let abs = self.abs(&path.0);
        let staged =
            file::stage(&abs, &contents).map_err(|e| IlrpError::Executor(e.to_string()))?;

        let expected_pre = match &mutation.expected_prestate {
            StatePredicate::FileContent { hash, .. } => Some(*hash),
            StatePredicate::FileAbsent { .. } => None,
            StatePredicate::Any => {
                // TOCTOU window accepted in the toy; documented.
                file::observe(&abs)
                    .map_err(|e| IlrpError::Executor(e.to_string()))?
                    .map(|o| o.hash)
            }
            _ => {
                return Err(IlrpError::Executor(
                    "non-file predicate in file step".into(),
                ));
            }
        };

        match staged.commit_if(expected_pre) {
            Ok(obs) => Ok(StepAck {
                step: mutation.id,
                observed_poststate: StatePredicate::FileContent {
                    path: path.clone(),
                    hash: obs.hash,
                },
                at: Timestamp::now(),
            }),
            Err(file::StageError::PrestateMismatch { .. }) => {
                // Best-effort remove the staged sibling (path matches
                // liminal_source::stage: `<target>.lim-staged`).
                let _ = std::fs::remove_file(Utf8PathBuf::from(format!("{abs}.lim-staged")));
                Err(IlrpError::Contested { step: mutation.id })
            }
            Err(file::StageError::Io(e)) => Err(IlrpError::Executor(e.to_string())),
        }
    }
}
