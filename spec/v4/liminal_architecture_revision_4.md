# Liminal Architecture Revision 4

## Status

Revision 4 incorporates the second-order corrections discovered after Revision 3 stabilized the two-primitive constitution and the Jurisdiction model.

Revision 3 established the right conceptual laws:

- Nodes and Relations remain the only semantic primitives.
- A facet is only a schema address and must be reified when it needs independent identity, history, Relations, or Jurisdiction.
- Cross-Jurisdiction repair is mutation-local.
- Capture is never rejected; inadmissible writes become durable Overlays.
- Jurisdiction is the deliberate complexity sink of the architecture.

Revision 4 addresses the operational consequences of those laws:

1. How Jurisdiction ergonomics are measured without teaching to a self-authored test.
2. How Promotion and repair become one mechanism rather than parallel lifecycle systems.
3. How a unique result is distinguished from a safe result.
4. How graph and filesystem mutations remain recoverable across crashes.
5. How concurrent dirty buffers participate in Workspace Bases without producing chimeric computations.
6. How Overlay debt becomes visible before the agenda subsystem exists.
7. How the internal vocabulary remains precise without leaking into routine user experience.

---

## 1. Complexity-budget admission

The kernel remains conceptually tiny:

```text
Node
Relation
```

The total system is not tiny.

Jurisdiction is now explicitly acknowledged as the largest policy subsystem because it contains the complexity that otherwise leaks independently into editors, file watchers, rich frontends, synchronizers, external resolvers, AI tools, and publication backends.

The architectural claim is no longer:

```text
two primitives make the system simple
```

It is:

```text
two primitives give the system one language in which to locate its complexity
```

The design obligation is to contain Jurisdiction through profiles, interpretation, conformance tests, and silent healthy behavior—not to pretend the underlying problems disappeared.

---

## 2. Jurisdiction ergonomics are trace- and session-measured

### 2.1 The denominator

The previous statement that profiles should cover 99% of subjects was underspecified and easy to game.

Revision 4 defines a **Jurisdiction-sensitive operation** as a semantic read, write, repair, synchronization, export, publication, foreign-edit ingestion, or resolver event that must choose a Holder, Workspace Basis component, write route, or reconciliation rule.

The denominator is not:

- Every Node
- Every internal graph traversal
- Every keystroke
- A hand-selected set of easy examples

Raw editor keystrokes are coalesced into semantic editor transactions before measurement. Repeated downstream invalidations caused by one root event do not inflate the operation count or the visible-incident count.

### 2.2 The adversary

Each stable profile has three trace sets:

1. **Development corpus** — inspectable and tunable by profile authors.
2. **Held-out acceptance corpus** — independently sourced and locked for a version.
3. **Regression corpus** — every previously discovered failure, minimized and retained.

Held-out and adversarial traces should include:

- Real Git merge, rebase, move, rename, and conflict-resolution histories
- Consented and anonymized editor sessions
- Foreign formatter and rich-tool rewrites
- Offline/online transitions
- External resolver observations and failures
- Concurrent clients
- Generated identity-damage operations
- Process restarts and crash injection

A profile cannot graduate by passing only self-authored fixtures.

### 2.3 Event and session SLOs

For every stable profile:

```text
automatic resolution of held-out Jurisdiction-sensitive operations >= 99%
ordinary held-out sessions requiring no manual Jurisdiction interaction >= 99%
user-facing Jurisdiction diagnostics in every sound session = 0
checker output on an entirely sound workspace = empty
unexpected reconciliation items in every sound ordinary session = 0
visible incidents per root cause per session <= 1
manual Contract authoring in ordinary standard-profile sessions = 0
```

The session SLO is load-bearing. A system that succeeds on 99% of 10,000 operations but interrupts the user 100 times has failed ergonomically.

An operation does not count as automatically resolved merely because Liminal hid the failure inside an Overlay. An undeclared or unexpected Reconciliation Queue entry counts against the profile. Profile-declared transient drafts, such as offline changes awaiting automatic synchronization, are reported separately and become debt if they exceed their policy window.

The 99% figure is a graduation floor, not the complete product metric. The experienced target for a sound routine session is zero ownership UI.

---

## 3. User vocabulary is domain-native

The architecture retains precise internal terminology, but routine UI does not need to display it.

Internal explanatory terms:

```text
Jurisdiction
Holder
Overlay
Promotion
```

Ordinary interface language:

```text
Overlay   → draft, unsaved change, pending sync, recovered edit
Promotion → save, sync, accept, apply, publish
Holder    → file, document, repository, service, device
```

The word `Jurisdiction` may remain entirely inside:

```bash
lim jurisdiction explain
```

Healthy editing should require none of the internal vocabulary. The four terms are a ceiling for explanations, not a required user lexicon.

---

## 4. Promotion is repair

Revision 4 removes Promotion as an independent mutation or lifecycle mechanism.

Promotion is now:

> The user-facing name for a RepairPlan whose accepted result causes an Overlay to become governed by its intended durable Holder or merge domain.

The same machinery handles:

- Save
- Synchronization
- Reattachment
- Merge
- Recovery
- Writeback
- Promotion

This reduces duplicated policy and ensures Promotions receive the same:

- Mutation-local authorization
- Identity checks
- Invariant checks
- Workspace Basis preconditions
- Safety validation
- Crash recovery
- Decision logging
- Revert support

A simple Promotion may contain one mutation. A coordinated Promotion may contain several physical mutations because acceptance can require source changes, Relation changes, graph finalization, and external acknowledgements.

---

## 5. Repair plans are dependency DAGs

An unordered list of proposed mutations is insufficient.

Example:

```text
1. Insert or restore durable paragraph ID in source file.
2. Reattach graph-native comment Relation to that ID.
```

Mutation 2 is invalid before mutation 1 succeeds.

Revision 4 therefore defines a repair as a dependency DAG:

```rust
pub struct RepairPlan {
    pub id: RepairId,
    pub basis: WorkspaceBasis,
    pub steps: BTreeMap<RepairStepId, ProposedMutation>,
    pub dependencies: Vec<RepairDependency>,
    pub inverse: Option<InverseRepairPlan>,
}
```

Each mutation carries:

- Its governed subject
- Expected prestate
- Expected poststate
- Idempotency key
- Operation
- Optional inverse or preimage

Authorization remains conjunctive and mutation-local. No Relation Contract gains permission to modify its target, and no target Contract gains permission to redirect the Relation.

---

## 6. Determinism is not safety

A merge engine can produce one unique result and still be semantically wrong.

Revision 4 requires both:

```text
deterministic candidate
+
domain-specific safety evidence
```

Automatic application requires:

1. One valid result at the captured Basis.
2. Authorization by every mutated subject's Jurisdiction.
3. Matching preconditions.
4. Preserved identity and semantic invariants.
5. A profile-specific safety predicate.
6. Idempotent execution.
7. A recorded revert path.

For external-file profiles, a clean textual merge is not sufficient. Automatic acceptance requires one of:

- Structural disjointness at the relevant Node, syntax, or stable-anchor level
- A language/domain validator proving non-interference
- Explicit human approval

Every automatic repair records:

```text
input Basis
selected rule
safety evidence
applied steps
resulting Basis
inverse or preimages
```

It must support:

```bash
lim repair undo <repair-id>
```

If later edits make direct reversal unsafe, the undo request becomes another RepairPlan rather than overwriting current state with stale bytes.

---

## 7. Intent-Logged Repair Protocol

A file and a graph store cannot participate in one ordinary atomic transaction. Revision 4 explicitly rejects the fiction that `Vec<ProposedMutation>` somehow creates atomicity.

Cross-Holder repairs use the **Intent-Logged Repair Protocol (ILRP)**.

### 7.1 Prepare

The transactional graph store records:

- RepairPlan
- Captured Workspace Basis
- Dependency DAG
- Expected prestates and poststates
- Idempotency keys
- Required capabilities
- Safety evidence
- Inverse or preimage data

The intent enters `Prepared` state.

### 7.2 Apply

Steps execute in topological order.

For a file mutation:

1. Verify expected file hash.
2. Produce staged content.
3. Flush staged content.
4. Use atomic replacement where the platform supports it.
5. Observe resulting file hash.

For a remote service, use an idempotency key when supported and record the observed external revision or response.

### 7.3 Acknowledge

Each completed external step and its poststate are persisted to the intent log. A repeated step must be harmless or detected as already applied.

### 7.4 Finalize

Once prerequisites are satisfied, graph mutations and intent completion are committed in one graph transaction. The accepted Workspace Basis advances only at finalization.

### 7.5 Recover

On restart:

- If the world matches the prestate, resume.
- If it matches the poststate, acknowledge and continue.
- If it matches neither, stop and preserve the remaining work as contested Overlays.
- Never guess.

ILRP is a crash-consistent saga, not global two-phase commit. Its guarantee is:

```text
visible durable intent
+ ordered idempotent external application
+ logically exactly-once graph acceptance
+ recoverable evidence of every partial state
```

Phase -1 injects daemon termination after every durable protocol boundary.

---

## 8. Workspace Basis Perspectives

The daemon may know about several dirty buffers over one file. A computation cannot read all of them as one state.

Revision 4 adds **Basis Perspectives**:

```rust
pub enum BasisPerspective {
    ClientScoped { client: ClientId },
    DurableOnly,
    Published { revision: PublicationId },
    Federated { domain: FederationId, frontier: CausalFrontier },
}
```

Rules:

### ClientScoped

Use the requesting client's selected dirty buffer where available, then durable fallbacks elsewhere. Never use another client's dirty buffer.

Default for:

- Interactive preview
- Interactive cross-document query
- AI context requested by that client
- Editor diagnostics

### DurableOnly

Ignore all provisional dirty buffers.

Default for:

- Background builds
- Unattended exports
- CI-like checks
- Publication
- Requester-less workspace jobs

### Published

Use one immutable outward-facing revision.

### Federated

Use only where a declared merge runtime has supplied one accepted causal frontier.

Two clients editing the same file create two working branches over one durable Holder. They do not create a shared chimeric working truth.

Per-buffer Basis components remain first-class:

```rust
BufferGeneration {
    client,
    buffer,
    epoch,
    generation,
    content_hash,
    base_file_hash,
}
```

This sets precise incremental-cache granularity. Client A's edit invalidates A-scoped computations that read it, not client B's private preview or durable-only publication output.

---

## 9. Reconciliation Queue precedes agenda

Overlay lifecycle cannot depend on a Phase 5 agenda subsystem.

Revision 4 defines a core **Reconciliation Queue** available in Phase -1.

Minimum surfacing:

```bash
lim overlays
lim repairs
```

Contextual surfacing occurs:

- When the affected subject is opened
- Before sync
- Before publication
- Before destructive conversion
- Before workspace cleanup

When the agenda domain exists, it projects the same reconciliation items as tasks. It does not copy them and is not their storage authority.

This keeps `never block capture` honest:

```text
capture immediately
preserve durably
reconcile automatically when safe
surface aging debt
never bury it silently
```

---

## 10. Revised Phase -1 toy

The first implementation remains interpretive and intentionally small.

Profiles:

- External-file
- Graph-native

Toy workspace:

- One file-held paragraph with durable identity
- One graph-native comment Relation targeting it
- Neovim dirty buffer over the file
- Phone dirty buffer over the same file
- One foreign edit damaging or duplicating identity
- One unavailable Holder producing a durable draft
- One unique-but-unsafe merge candidate
- One structurally disjoint safe repair
- One repair DAG requiring ID insertion before Relation reattachment

The daemon is terminated after every ILRP durable boundary:

- Intent commit
- External apply
- Acknowledgement
- Before graph finalization
- After graph finalization but before notification

Required results:

- Sound states are silent.
- No edit is rejected.
- Promotion and repair use one interpreter.
- Unique-but-unsafe candidates are not auto-accepted.
- Safe repairs are logged and one-command reversible.
- Every crash recovers to `Committed`, `NeedsReview`, or `Aborted` without hidden half-state.
- `ClientScoped(neovim)`, `ClientScoped(phone)`, and `DurableOnly` produce three intentional snapshots.
- No computation sees both dirty buffers as one state.
- Reconciliation debt is visible through core CLI without agenda.
- Incremental invalidation follows the selected Basis components.

Compiled Jurisdiction plans, indexed dispatch, generated policy code, and custom collaboration machinery remain forbidden until the identity, projection, crash, and trace experiments stabilize the reference semantics.

---

## 11. Remaining research questions

Revision 4 closes the constitutional shape but does not solve the domains it names.

### 11.1 Semantic disjointness

Each external-file domain must define what counts as structurally disjoint:

- Markdown blocks
- Rust syntax Nodes
- Table cells
- Citation records
- Presentation objects

This is a domain contract, not one universal textual heuristic.

### 11.2 Non-idempotent remote services

Some services do not provide idempotency keys or exact revision tokens. Their repair profile may have to prohibit automatic writeback, create immutable observations, or require explicit human acceptance.

### 11.3 Revert after subsequent edits

An inverse is only safe against compatible current state. The repair engine must treat undo as another Basis-checked repair rather than as unconditional time reversal.

### 11.4 Second-client save

When client A saves and changes the durable file while client B remains dirty, B's later save needs an explicit rebase, domain merge, or review path. `last writer wins` cannot be the silent default.

### 11.5 Trace privacy and representativeness

Recorded editor sessions require consent, minimization, anonymization, and a corpus design that does not become biased toward the project's own authoring habits.

### 11.6 Annotated-source viability

The annotated-source profile remains the highest-risk portable-document case. A Contract declaration does not prove anchor recovery, identity continuity, or rich-editor round trips under foreign edits.

### 11.7 Federated domains

A Federated profile delegates to an actual merge runtime. Jurisdiction can name and constrain that runtime; it cannot replace the CRDT, OT, Git, database, or domain-specific algorithm.

---

## 12. Final Revision 4 law

The strongest formulation is:

> Liminal's kernel is small; its ownership, repair, and synchronization problems are not. Jurisdiction gives those problems one typed home. Profiles make the common case invisible. Repair plans make changes mutation-local. ILRP makes cross-Holder work recoverable. Basis Perspectives prevent concurrent working state from becoming a chimera. The Reconciliation Queue prevents nonblocking capture from turning into buried data loss.

Revision 4 is ready to serve as the constitution for the Phase -1 falsification laboratory.
