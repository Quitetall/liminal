//! File-system external executor for ILRP (M02 Algorithm C).
//!
//! `FsExecutor` implements `ExternalExecutor` against real files in a workspace
//! root. It uses `liminal-source::file::{stage, observe}` for hash-verified
//! staged writes.
//!
//! ── STUB SURFACE (M04.5, T2): `InsertSourceId` execution + AM-4.1 store handle.
//!
//! `WriteFile` is real (M02). `InsertSourceId` currently returns
//! `Executor("InsertSourceId deferred to M4")`. To implement per M04
//! Algorithm A ("InsertSourceId execution"), change `FsExecutor` to
//! `FsExecutor<'s> { root, store: &'s GraphStore }` (AM-4.1 second half — the
//! executor needs the store to resolve entity→alias), then in `apply` do:
//! read bytes (hash must equal the step prestate, else `verify` already
//! classified `Neither`); find alias `a` by scanning `ns::JUR_ALIAS` for the
//! entry whose `.entity == m.entity`; build `new_bytes = bytes[..at.start] ++
//! " {#a}" ++ bytes[at.start..]` (the `at: SourceRange` was computed against
//! prestate bytes at plan time — the hash check validates the offset);
//! `stage(&abs, &new_bytes)` then `commit_if(Some(prestate hash))`; ack with
//! the observed poststate hash. `verify` for `InsertSourceId` mirrors
//! `WriteFile` (prestate/poststate hash classification against the on-disk
//! file). Also add the D04.4 rule to `IlrpDriver::run` (in
//! liminal-jurisdiction): a plan where any file/external step depends on a
//! `Graph(_)` step is refused with `Executor("graph-before-file ordering
//! cannot be executed under ILRP")`.

use camino::{Utf8Path, Utf8PathBuf};
use liminal_id::Timestamp;
use liminal_jurisdiction::ProposedMutation;
use liminal_jurisdiction::{
    ExternalExecutor, IlrpError, PrestateMatch, RepairOperation, StatePredicate, StepAck,
};
use liminal_source::{self as file, FileObservation};

/// File-system executor: stages, commits, and verifies against `root`.
#[derive(Debug)]
pub struct FsExecutor {
    root: Utf8PathBuf,
}

impl FsExecutor {
    /// Create a new executor rooted at `root`.
    #[must_use]
    pub fn new(root: Utf8PathBuf) -> Self {
        Self { root }
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

impl ExternalExecutor for FsExecutor {
    fn verify(&self, mutation: &ProposedMutation) -> Result<PrestateMatch, IlrpError> {
        let path = match &mutation.operation {
            RepairOperation::WriteFile { path, .. } => path,
            RepairOperation::InsertSourceId { .. } => {
                return Err(IlrpError::Executor("InsertSourceId deferred to M4".into()));
            }
            RepairOperation::External { .. } => {
                return Err(IlrpError::Executor("External deferred to M4".into()));
            }
            RepairOperation::Graph(_) => {
                return Err(IlrpError::Executor(
                    "non-file predicate in file step".into(),
                ));
            }
        };

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
        let RepairOperation::WriteFile { path, contents } = &mutation.operation else {
            return Err(IlrpError::Executor(
                "only WriteFile is supported at M2".into(),
            ));
        };

        let abs = self.abs(&path.0);
        let staged = file::stage(&abs, contents).map_err(|e| IlrpError::Executor(e.to_string()))?;

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
