//! Scripted per-client sessions with first-class `epoch + generation` buffer
//! Basis components (v4 §7.5). "neovim" and "phone" in the toy are exactly
//! these — no real editors exist in Phase -1.

use liminal_graph::ns::SYS_BLOB;
use liminal_id::{BufferId, ClientId, ContentHash, JurisdictionKey, PathId, RepairId};
use liminal_revision::BasisComponent;

use crate::workspace::{BufferState, SaveError, ToyWorkspace};

/// One client's dirty-buffer session.
#[derive(Debug)]
pub struct ClientSession<'w> {
    workspace: &'w mut ToyWorkspace,
    client: ClientId,
}

impl<'w> ClientSession<'w> {
    pub(crate) fn new(workspace: &'w mut ToyWorkspace, client: ClientId) -> Self {
        Self { workspace, client }
    }

    /// The client this session belongs to.
    #[must_use]
    pub fn client(&self) -> ClientId {
        self.client
    }

    /// Open a buffer over a durable file subject (M06 Algorithm B). Mints a
    /// `BufferId`, reads the file base hash, persists the generation-0 blob,
    /// registers the buffer, and publishes its `BufferGeneration` component —
    /// UNLESS this client already has a divergent buffer over the same path
    /// with no selection, in which case the D06.2 ambiguity procedure runs:
    /// the subject falls back to durable state and one coalesced
    /// `ambiguity:` reconciliation item is surfaced.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "frozen surface: open_buffer(path: PathId) (M06 §Frozen surfaces)"
    )]
    pub fn open_buffer(&mut self, path: PathId) -> BufferId {
        let buffer = BufferId::new();
        let key = JurisdictionKey::Path(path.clone());

        // Read the durable base bytes/hash and persist the gen-0 blob.
        let abs = self.workspace.root().join(&path.0);
        let base_bytes = std::fs::read(&abs).unwrap_or_default();
        let base_hash = ContentHash::of(&base_bytes);
        self.persist_buffer_blob(buffer, 0, &base_bytes);

        self.workspace.register_buffer(
            buffer,
            BufferState {
                client: self.client,
                path: path.clone(),
                generation: 0,
                base_file_hash: base_hash,
            },
        );

        // D06.2: a SECOND divergent buffer over an already-buffered path with
        // no selection makes the subject ambiguous. Drop the working claim
        // (fall back to durable) and surface one coalesced item.
        if self.workspace.has_working(self.client, &key) {
            self.workspace.unpublish_working(self.client, &key);
            self.surface_ambiguity(&path);
            return buffer;
        }

        let component = self.buffer_component(buffer, 0, base_hash, base_hash);
        self.workspace.publish_working(self.client, key, component);
        buffer
    }

    /// Apply an edit, producing a new `BufferGeneration` component (epoch +
    /// generation + content hash). Never rejected (Law 3B). Republishes the
    /// component when this buffer is the selected/only one over its subject.
    pub fn edit(&mut self, buffer: BufferId, contents: &str) -> BasisComponent {
        let (path, base_file_hash, generation) = {
            let state = self
                .workspace
                .buffer_mut(buffer)
                .expect("edit on an unopened buffer");
            state.generation += 1;
            (state.path.clone(), state.base_file_hash, state.generation)
        };
        let content_hash = ContentHash::of(contents.as_bytes());
        self.persist_buffer_blob(buffer, generation, contents.as_bytes());

        let component = self.buffer_component(buffer, generation, content_hash, base_file_hash);
        let key = JurisdictionKey::Path(path);
        // Republish unless this subject is ambiguous (another divergent buffer
        // with no selection dropped the claim).
        if self.workspace.has_working(self.client, &key) || !self.subject_is_ambiguous(&key) {
            self.workspace
                .publish_working(self.client, key, component.clone());
        }
        component
    }

    /// Record a working-holder selection over a subject (AM-6.2, v4 §7.5).
    /// Republishes the selected buffer's current component and resolves the
    /// ambiguity item.
    pub fn select_working(&mut self, path: &PathId, buffer: BufferId) {
        let key = JurisdictionKey::Path(path.clone());
        let state = self
            .workspace
            .buffer(buffer)
            .expect("select on an unopened buffer")
            .clone();
        self.workspace
            .select_working(self.client, key.clone(), buffer);
        // Republish the selected buffer's current generation component.
        let content_hash = self.buffer_content_hash(buffer, state.generation);
        let component =
            self.buffer_component(buffer, state.generation, content_hash, state.base_file_hash);
        self.workspace.publish_working(self.client, key, component);
        self.resolve_ambiguity(path);
    }

    /// Build a `BufferGeneration` component for this session.
    fn buffer_component(
        &self,
        buffer: BufferId,
        generation: u64,
        content_hash: ContentHash,
        base_file_hash: ContentHash,
    ) -> BasisComponent {
        BasisComponent::BufferGeneration {
            client: self.client,
            buffer,
            epoch: self.workspace.epoch(),
            generation,
            content_hash: Some(content_hash),
            base_file_hash: Some(base_file_hash),
        }
    }

    /// The content hash of a persisted buffer generation blob.
    fn buffer_content_hash(&self, buffer: BufferId, generation: u64) -> ContentHash {
        let key = format!("buf/{}/{buffer}/{generation}", self.client);
        self.workspace
            .store()
            .get_aux(SYS_BLOB, &key)
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(|s| ContentHash::of(s.as_bytes())))
            .unwrap_or_else(|| ContentHash::of(b""))
    }

    /// Persist `SYS_BLOB["buf/<client>/<buffer>/<generation>"] = bytes`
    /// (D06.4) so memoized queries can read exact generation bytes. This is an
    /// EPHEMERAL working-state write (M08): it does NOT advance the graph
    /// revision, so a buffer edit never invalidates graph-reading queries.
    fn persist_buffer_blob(&self, buffer: BufferId, generation: u64, bytes: &[u8]) {
        let _ = self.workspace.working_capture().put_buffer(
            self.client,
            buffer,
            generation,
            &String::from_utf8_lossy(bytes),
        );
    }

    /// The `ambiguity:` root-cause key for a subject (D05.5 grammar).
    fn ambiguity_root_cause(&self, path: &PathId) -> String {
        format!("ambiguity:client:{}:path:{path}", self.client)
    }

    /// Surface one coalesced `ambiguity:` reconciliation item (D06.2): the
    /// client has divergent buffers over `path` and has selected none, so the
    /// subject fell back to durable state and the ambiguity is visible debt.
    fn surface_ambiguity(&self, path: &PathId) {
        let store = self.workspace.store();
        let root_cause = self.ambiguity_root_cause(path);
        let queue = liminal_jurisdiction::ReconciliationQueue { store };
        let item = liminal_jurisdiction::ReconciliationItem {
            id: liminal_id::ReconciliationItemId::new(),
            root_cause,
            subjects: vec![],
            overlays: vec![],
            repairs: vec![],
            created_at: liminal_id::Timestamp::now(),
            status: liminal_jurisdiction::ReconciliationStatus::Pending,
        };
        let writer = self.workspace.reconciliation_writer();
        if let Ok(mut txn) = writer.begin() {
            let _ = queue.upsert_coalesced(&mut txn, item);
            let _ = txn.commit(liminal_graph::TxnMeta {
                actor: None,
                origin: liminal_graph::Origin::Human,
                at: liminal_id::Timestamp::now(),
                provenance: Some("ambiguity:surface".into()),
                inverse: None,
            });
        }
    }

    /// Whether a subject currently has a live `ambiguity:` item for this client.
    fn subject_is_ambiguous(&self, key: &JurisdictionKey) -> bool {
        let JurisdictionKey::Path(path) = key else {
            return false;
        };
        let root_cause = self.ambiguity_root_cause(path);
        let queue = liminal_jurisdiction::ReconciliationQueue {
            store: self.workspace.store(),
        };
        queue.items().is_ok_and(|items| {
            items.iter().any(|i| {
                i.root_cause == root_cause
                    && i.status == liminal_jurisdiction::ReconciliationStatus::Pending
            })
        })
    }

    /// Mark the `ambiguity:` item for `path` resolved (AM-6.2: a selection was
    /// made, so the subject is no longer ambiguous).
    fn resolve_ambiguity(&self, path: &PathId) {
        let store = self.workspace.store();
        let root_cause = self.ambiguity_root_cause(path);
        let queue = liminal_jurisdiction::ReconciliationQueue { store };
        let Ok(items) = queue.items() else {
            return;
        };
        for mut item in items {
            if item.root_cause == root_cause
                && item.status == liminal_jurisdiction::ReconciliationStatus::Pending
            {
                item.status = liminal_jurisdiction::ReconciliationStatus::Resolved;
                let writer = self.workspace.reconciliation_writer();
                if let Ok(mut txn) = writer.begin() {
                    let _ = txn.put_aux(
                        liminal_jurisdiction::reconcile::RECONCILE_NS,
                        &item.id.to_string(),
                        serde_json::to_value(&item).unwrap_or_default(),
                    );
                    let _ = txn.commit(liminal_graph::TxnMeta {
                        actor: None,
                        origin: liminal_graph::Origin::Human,
                        at: liminal_id::Timestamp::now(),
                        provenance: Some("ambiguity:resolve".into()),
                        inverse: None,
                    });
                }
            }
        }
    }

    /// Save the buffer. Save IS Promotion IS a `RepairPlan` through the one
    /// interpreter (R4 §4): operationally a repair from the buffer Overlay
    /// into the file Holder, executed via ILRP. An unavailable Holder still
    /// succeeds — the edit becomes a durable Overlay (Law 3B).
    pub fn save(&mut self, buffer: BufferId) -> Result<RepairId, SaveError> {
        let _ = buffer;
        let _ = &self.workspace;
        // ── STUB (M04.6, T2). Spec = M04 Algorithm C. ──
        //
        // save IS Promotion IS a RepairPlan through the ONE IlrpDriver (R4 §4).
        // 1. (path, base_hash, ours) = buffer state (M6 moves this into
        //    AvailableInputs; until then read from the runner's buffer map).
        // 2. persist SYS_BLOB[blake3(ours)] (base blob persisted at open).
        // 3. if !holder_available(store, path) → M5 branch (stub: always avail).
        // 4. outcome = merge::three_way(SYS_BLOB[base], ours, current file bytes):
        //      Disjoint | UniqueOverlap → plan = save-promotion builder (merged);
        //      Conflict → persist JUR_PLAN[draft plan carrying ours] (+ M5
        //          Overlay + item); return Ok(plan.id) — capture never rejected.
        // 5. one txn: JUR_PLAN[plan] + JUR_DECISION[checker.evaluate_repair(&plan)].
        // 6. AutoApply{evidence} → driver.prepare(plan, evidence); driver.run
        //    → expect Committed; return Ok(plan.id).
        // 7. NeedsReview{reasons} → (M5 adds Overlay + reconciliation item);
        //    return Ok(plan.id).
        //
        // Save-promotion builder (Algorithm A): one WriteFile step; prestate =
        // observed base hash (or FileAbsent); poststate = merged-bytes hash;
        // inverse = Some(InverseRepairPlan(one WriteFile of the OLD bytes,
        // prestate = new hash, poststate = old hash)); subject = the FILE node.
        // The runner's inline save logic (runner::build_save_plan) is the M02
        // seed to fold into this path.
        todo!("Phase -1 M4: save-as-Promotion through IlrpDriver (Algorithm C; R4 §4)")
    }
}
