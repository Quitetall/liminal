# Liminal — Master Architecture and Infrastructure Plan

**Revision:** 4 — measured Jurisdiction ergonomics, unified repair, crash-safe coordination, and client-scoped bases  
**Project type:** Open-source infrastructure and systems research project  
**Primary motivation:** Build the technically correct substrate for personal use and intellectual satisfaction; adoption is welcome but not required.  
**Core design rule:** **The semantic universe contains only Nodes and Relations. Everything else is a derived abstraction that may compile into any imperative subsystem required for performance.**

---

## 0. Executive thesis

Liminal is not another note-taking application and not a plan to rewrite Neovim, Word, Google Docs, Excel, PowerPoint, Typst, Pandoc, Jupyter, or a drawing program.

Liminal is the infrastructure layer beneath those categories:

> A local-first, multimodal, revisioned graph runtime in which human information is represented by Nodes and Relations, manipulated by programmable graph transformations, and compiled into human, machine, editor, web, print, synchronization, and AI projections.

A user should be able to inhabit one coherent workspace containing:

- Plain text and rich text
- Source code and software projects
- Books, fiction, essays, and long-form prose
- Academic papers, mathematics, citations, and bibliographies
- Tables, formulas, and spreadsheet-like computation
- Slides and spatial presentations
- Images, diagrams, and PDFs
- Handwriting, digital ink, and infinite canvases
- Audio, voice notes, lecture recordings, and transcription
- Video references and time-aligned annotations
- Tasks, schedules, agendas, and recurring plans
- Executable notebooks and generated outputs
- Local databases and remote data
- Live, cached, mirrored, frozen, and synchronized relations
- AI-readable semantic projections and AI-generated graph operations

Existing tools remain sovereign. Neovim edits text. Language servers understand programming languages. Cargo builds Rust. Browsers render DOM and CSS. Pandoc converts mature document formats. Typst or LaTeX may typeset. Speech models transcribe. Liminal provides the common graph, history, orchestration, projections, relations, and runtime contracts that let all of them operate as parts of one system.

Sovereignty is not merely an integration preference; it is part of the Jurisdiction model. In a Rust source file, the current editor buffer or persisted byte file may hold Jurisdiction over program text while Liminal's syntax and semantic graph are derived. In a graph-native manuscript or slide deck, the graph may hold Jurisdiction and text may be a serialization. In an audio recording, the immutable media object may hold Jurisdiction while transcript text is derived. Liminal therefore does not impose one global source-of-truth representation. It resolves one Jurisdiction for every independently governable Node or Relation at a Workspace Basis. Domain profiles supply the common contracts automatically; users encounter Jurisdiction only at boundaries, conflicts, or explicit inspection.

The project is intentionally ambitious. It is infrastructure built for fun, use, learning, and long-term reuse. The correct success condition is not immediate market adoption. The first success condition is that the architecture remains coherent when text, code, tables, images, handwriting, audio, synchronization, and AI are all placed under it without inventing new semantic foundations for each domain.

---

# Part I — Architectural constitution

## 1. Goals

Liminal should:

1. Preserve a tiny, formally understandable semantic kernel.
2. Support compact Markdown-like authorship with a fully explicit form.
3. Permit user-specific textual projections without changing meaning.
4. Compile to HTML quickly enough for continuous preview.
5. Support rich Word/Docs-like and canvas-like frontends under explicit domain Jurisdiction profiles; some domains may use graph-native Jurisdiction while others use external-file or immutable-resource Jurisdiction.
6. Treat code as ordinary source owned by editors and programming-language tools.
7. Support pseudo-stenographic macros and semantic transformations.
8. Work offline by default.
9. Allow external data to be pointers, snapshots, caches, mirrors, live values, or replicated state.
10. Support voice, images, pen input, tables, slides, and other media as native graph domains.
11. Produce model-specific AI representations rather than forcing AI to read the human projection.
12. Preserve every change through semantic transactions, snapshots, and optional exact source edits.
13. Expose Unix-style tools while allowing those same tools to fuse into one optimized process.
14. Reuse the Rust ecosystem, Cargo, Wasm, Lua, Cranelift, language servers, Pandoc, browsers, and other mature infrastructure rather than replacing them gratuitously.
15. Remain open, inspectable, exportable, recoverable, and independent of one cloud vendor.

## 2. Non-goals

Liminal does not initially need to:

- Replace Neovim or ship a mandatory Neovim distribution.
- Replace Helix, Emacs, VS Code, or their interaction models.
- Replace programming-language compilers, language servers, debuggers, or package managers.
- Reimplement every Pandoc reader and writer.
- Invent a new CRDT before evaluating existing approaches.
- Invent new image, audio, video, or PDF formats without necessity.
- Perfectly round-trip every historical DOCX or PPTX quirk.
- JIT-compile ordinary paragraph rendering.
- Force every domain into one physical storage layout.
- Expose raw generic graph operations as the only public API.
- Make Internet connectivity a prerequisite for ordinary use.
- Require users to understand the complete internal graph to write a normal note.

## 3. Architectural laws

### Law 1 — Only Nodes and Relations are semantically fundamental

Documents, text, resources, schemas, macros, transformations, views, operations, histories, tasks, formulas, citations, slides, transcripts, and Jurisdiction policies are all compositions of Nodes and Relations.

### Law 2 — Semantic minimalism does not imply physical uniformity

Text may be a Node semantically and a rope or piece tree physically. A table may be a graph semantically and columnar memory physically. A reference may be a Relation semantically and a packed adjacency entry physically.

### Law 2A — A tiny kernel does not make the total system simple

The two-primitive kernel minimizes ontology; it does not erase domain complexity. Identity, rich editing, replication, external effects, and ownership remain hard. Liminal deliberately concentrates ownership and write-routing complexity in the Jurisdiction subsystem so that no editor, renderer, resolver, or domain invents a competing source-of-truth model. The specification must admit and test that complexity rather than hide it behind the word "graph."

### Law 3 — Jurisdiction resolves every independently governable semantic address at a Workspace Basis

A workspace does not need one globally privileged representation. Every independently governable Node or Relation resolves to one Holder, one declared federated merge domain, or one provisional Overlay at a Workspace Basis. Every derived fact records the exact Basis from which it was computed. No address may have two undeclared writers.

A **facet** is not a third semantic primitive. It is schema shorthand for addressing part of a Node or Relation. If a value needs independent identity, history, Relations, or Jurisdiction, it must be reified as a Node or Relation. `FacetId` is therefore an interned schema address used by compilers and profiles, not a new entity class.

### Law 3A — No accidental Jurisdiction

A projection, cache, materialization, replica, local copy, generated output, or editor buffer does not acquire durable Jurisdiction merely by existing or being modified. Its domain profile must declare its role and write route.

### Law 3B — Capture is never rejected

Liminal must preserve typing, drawing, dictation, paste, and other capture even when the intended Holder is unavailable, read-only, ambiguous, or inconsistent. The edit becomes a durable local Overlay rather than being blocked or discarded.

### Law 3C — No silent acceptance or Holder change

An Overlay becomes accepted by a durable Holder only through an explicit, recorded repair whose acceptance conditions are declared by the relevant Jurisdictions. **Promotion** is the user-facing name for such an accepted repair; it is not an independent mutation mechanism. Importers, formatters, renderers, caches, and projections never transfer Jurisdiction implicitly.

### Law 3D — One immutable Workspace Basis per computation

Every parser query, compiler pass, renderer, export, AI operation, resolver decision, and repair plan runs against one captured Workspace Basis. Editor buffers participate through first-class, per-buffer generation components. A computation may finish on that Basis or restart on a newer one; it may not silently mix them.

### Law 3E — The common case requires zero Jurisdiction authoring and zero sound-state noise

Standard profiles must resolve ordinary work without authored Contracts or ownership ceremony. Conformance is measured over **Jurisdiction-sensitive read and write operations** replayed from held-out traces not authored or tuned by the profile implementer, including real Git histories, recorded editor sessions, and foreign-tool edits. Each stable profile must automatically resolve at least 99% of those operations, at least 99% of ordinary sessions must require no manual Jurisdiction interaction, and every sound session must emit exactly zero user-facing Jurisdiction diagnostics. Diagnostics from one root cause must be coalesced rather than amplified per keystroke or dependent operation.

The default UI should use familiar language such as **draft**, **save**, **sync**, **accept**, **source**, and **conflict**. The terms **Jurisdiction**, **Holder**, **Overlay**, and **Promotion** belong to advanced explanations and infrastructure documentation, not routine editing chrome.

### Law 3F — Cross-Jurisdiction repair is mutation-local

No Contract gains power over another subject merely because a Relation connects them. A repair is an ordered graph of proposed mutations, and each mutation is governed by the Jurisdiction of the Node or Relation it changes. The target's foreign-edit policy governs target mutation; the Relation's identity requirement and dangling-reference policy govern Relation mutation. Automatic repair is allowed only when every affected Jurisdiction authorizes its own mutation and all cross-subject requirements are met. Otherwise Liminal preserves the proposals as Overlays and creates one coalesced reconciliation item.

### Law 3G — Promotion is repair, and determinism is not safety

Every Promotion is a repair plan evaluated by the same authorization, identity, invariant, Basis, crash-recovery, and reversibility rules as any other repair. A unique textual or merge result is necessary but not sufficient for automatic acceptance. Automatic repair additionally requires a domain-specific safety check. For external-file domains, a textually clean merge is only a proposal unless the affected semantic Nodes are disjoint or a language/domain validator proves non-interference. Every automatically accepted repair must be recorded and one-command revertible.

### Law 3H — Cross-Holder repair is intent-logged, ordered, idempotent, and resumable

Files, graph stores, remote services, and other Holders do not share one atomic transaction. Any repair spanning more than one Holder must first commit a durable repair intent, execute an explicit dependency DAG with preconditions, postconditions, and idempotency keys, and then finalize the accepted graph revision. A crash may leave work pending, but may not make a half-applied repair invisible or unrecoverable.

### Law 3I — Overlay debt remains visible until resolved

Deferring reconciliation must not become burial. Every unresolved Overlay is durable, ages according to its profile, is automatically accepted only when the resulting repair is both deterministic and safe, and otherwise enters a core reconciliation queue. The CLI and contextual UI surface this queue from Phase -1; the later agenda domain may project the same items as tasks. Overlays surface when their target is opened and at sync, publication, or destructive-conversion boundaries. They are never silently garbage-collected.

### Law 3J — Independent dirty buffers never form a chimeric Basis

When multiple clients hold dirty buffers over the same durable subject, they represent distinct working claims. A computation must explicitly select the requesting client's working Basis, a durable-only Basis, a published Basis, or a declared federated frontier. Workspace-scoped consumers may not combine unrelated dirty buffers from different clients into one semantic snapshot.

### Law 4 — Personalization may change representation, never interpretation

One user may see compact syntax and another explicit syntax. Where a projection is declared reversible, both must resolve to the same governed facts at the same Basis. A projection that cannot guarantee this must be marked read-only, lossy, or import/export only.

### Law 5 — Effects are explicit

Network access, database access, process execution, microphone use, camera use, filesystem writes, and credential access occur through capability-controlled resolvers. Traversing a Relation must not unexpectedly perform an effect.

### Law 6 — High-level abstraction towers disappear before hot execution

Macros, defaults, aliases, relation policy composition, and declarative schemas must lower into compact plans, direct dispatch, specialized stores, or generated code—but only after their semantics survive the falsification prototypes.

### Law 7 — Existing tools may hold Jurisdiction, not merely integrate

Liminal integrates rather than needlessly replaces. An editor buffer may be the working Holder of program text. A file may be its durable Holder. A rich editor may write a graph-native transaction model. An external database may hold a remote fact. Liminal declares these boundaries instead of pretending its graph originated every value.

### Law 8 — Every conversion declares loss

A transformation states what it requires, preserves, introduces, and may discard.

### Law 9 — Offline is a normal operating condition

External availability and freshness are explicit. Last-known values, immutable observations, and local Overlays remain usable according to profile policy.

### Law 10 — Human and AI projections are separate compiler targets

Human syntax is optimized for authorship and understanding. AI input is optimized for semantic bandwidth, task relevance, provenance, and model behavior.

### Law 11 — Unix modularity is an interface property, not a demand for process overhead

Every subsystem may be independently callable, but production pipelines may link and fuse the same libraries in-process.

### Law 12 — Identity strength must match Relation durability

Liminal must never promise perfect persistent identity for anonymous text edited by arbitrary external tools. Durable cross-revision Relations require an identity mechanism strong enough for that guarantee: explicit identity, graph-managed identity, externally supplied identity, or a declared content address. Heuristic continuity is labeled as heuristic.

### Law 13 — Logical entity identity and immutable version identity are distinct

A mutable logical entity may retain an `EntityId` across accepted revisions. Each immutable payload or semantic version may have a content-derived `VersionId`. Human names and source anchors are aliases or locators, not substitutes for either identity.

### Law 14 — Risk-retirement order precedes dependency order

The build graph may require a parser before a rich editor, but the research program tests architecture-killing assumptions first. Identity round-trips, cross-Jurisdiction repair, annotated-source recovery, Overlay lifecycle, and bidirectional rich-editing lenses are Phase -1 falsification work, not late ADR cleanup.

### Law 15 — Generated information carries provenance

Transcripts, OCR, summaries, tables, computed outputs, AI text, and remote observations retain their source, time, tool or model, revision, and approval state.

### Law 16 — The system remains recoverable without its richest runtime

Canonical source and ordinary resources remain readable and exportable even if caches, indexes, or advanced frontends disappear.

---

# Part II — The semantic kernel

## 4. Node

A Node is an identifiable unit of state.

Conceptually:

```rust
pub struct Node {
    pub id: NodeId,
    pub kind: KindId,
    pub payload: PayloadRef,
    pub revision: RevisionId,
    pub flags: NodeFlags,
}
```

`kind`, `revision`, and `flags` are physical fast paths. Semantically, even a kind may be understood as a Relation to a schema or dialect definition.

A Node may represent:

- A text fragment
- A paragraph or heading
- A document or workspace root
- A code block, source file, function, or symbol
- A table, row, cell, or formula
- A slide, shape, or animation keyframe
- An image placement or image region
- An audio recording or transcript segment
- A handwriting stroke group
- A citation or bibliography record
- A task or agenda item
- A remote identity
- A locally materialized observation
- A schema, macro, transformation, or compiler rule

### 4.1 Identity

Every node has a runtime identity. Nodes that participate in cross-revision or cross-document semantics receive a persistent identity.

Persistent identity must not depend on:

- File path
- Heading text
- Byte offset
- Line number
- Visible label

Human aliases may coexist with opaque persistent IDs.

A practical identity policy:

- Runtime-only nodes receive compact ephemeral IDs.
- Persistently referenced, annotated, synchronized, or historically tracked nodes receive durable IDs.
- Durable IDs may be written explicitly in canonical source or in a deterministic graph serialization.
- Personal compact projections may hide those IDs.

### 4.2 Payload

A payload is data physically associated with a Node. It is not a third semantic entity.

Payload storage may be:

- Inline scalar
- Interned symbol
- Shared text range
- Typed record slot
- Content-addressed object handle
- Opaque extension bytes
- Expression handle
- Resource descriptor

Small values should avoid allocation. Large values should live out-of-line.

### 4.3 Logical versus physical granularity

Logical structure and physical allocation are independent.

A paragraph may logically contain multiple inline Nodes while all text lives in one rope. Splitting a paragraph may create two logical Nodes that reference shared immutable text segments. An image Node may contain only a content hash and semantic placement metadata while the image bytes remain in an object store.

No design decision should require one heap allocation per logical node.

## 5. Relation

A Relation is a typed association, dependency, order, invariant, or contract between Nodes.

Conceptually:

```rust
pub struct Relation {
    pub id: RelationId,
    pub source: NodeId,
    pub target: Target,
    pub kind: KindId,
    pub payload: RelationPayloadRef,
    pub revision: RevisionId,
    pub flags: RelationFlags,
}
```

A Relation can represent:

- Containment
- Ordering
- Reference
- Hyperlink
- Citation
- Transclusion
- Comment or annotation
- Definition and use
- Formula dependency
- Build dependency
- Temporal alignment
- Provenance
- Layout constraint
- Synchronization contract
- Materialization
- Replication
- External identity

### 5.1 Binary edges and higher-order relationships

Binary Relations remain the optimized primitive. N-ary relations are represented by reification:

```text
[meeting-event]
  --participant--> [Alice]
  --participant--> [Bob]
  --document--> [agenda]
  --time--> [timestamp]
```

The event itself is a Node. This preserves the two-primitive model without making the Relation representation arbitrarily complex.

### 5.2 Containment and order

Containment and order are semantically Relations but physically privileged because nearly every editor depends on them.

A logical form might be:

```text
[parent] --contains(order=token)--> [child]
```

Physical implementations may use:

- Packed arrays
- Persistent B-trees
- Finger trees
- Order-maintenance labels
- Sequence CRDTs
- Parent and sibling indexes

### 5.3 Anchored Relations

Relations may target a range within a Node or resource:

- A sentence in text
- A region of an image
- A page rectangle in a PDF
- A time interval in audio or video
- A set of handwriting strokes

Anchors must be revision-aware and resilient to edits. Raw byte offsets may be cached, but they must not be the only persistent representation.

### 5.4 Relation as maintained invariant

A pointer says where something is. A Liminal Relation may also state what must remain true.

Examples:

```text
[formula-output] --depends-on--> [input-cell]
[local-progress] --mirrors--> [remote-progress]
[transclusion] --reflects--> [target-section]
[transcript] --aligns-with--> [audio interval]
```

The Relation remains declarative. Imperative resolvers maintain or materialize the invariant.

## 6. Everything else is derived

### Document

A Document is a rooted subgraph with edition, namespace, ownership, and revision policies.

### Workspace

A Workspace is a Document whose graph contains other documents, code projects, resources, outputs, integrations, and configuration.

### Text

Text is a specialized Node representation and performance subsystem.

### Resource

A Resource is a Node whose bulk payload is content-addressed or externally stored.

### Schema

A Schema is a graph of constraints, defaults, normalization rules, migration rules, formatting behavior, and lowering behavior supplied by a compiler dialect.

### Transformation

A Transformation is an executable graph rewrite represented by Nodes and Relations and executed by an imperative runtime.

### Macro

A Macro is a named transformation, a transformation generator, or compact syntax that expands into graph structure.

### Compiler

A Compiler is an ordered dependency graph of transformations.

### Formatter

A Formatter is a projection pass that writes a chosen textual or binary representation.

### View

A View is a compiled projection for a target, profile, revision, and viewport.

### History

History is a revision graph and transaction log over graph states.

### Synchronization

Synchronization is the maintenance of Relations across replicas, devices, processes, or external systems.

---

# Part III — Jurisdiction, source, identity, and projections

## 7. Jurisdiction: the deliberate complexity sink

Liminal's semantic kernel remains only Nodes and Relations. Its operational constitution is not correspondingly tiny. Universal workspaces fail when ownership, write routing, identity, external edits, and replication are implicit, so Jurisdiction is intentionally the largest policy subsystem.

This complexity must be **contained**:

- Domain and workspace profiles author nearly all Contracts.
- Routine UI uses familiar draft/save/sync language; Jurisdiction vocabulary is inspectable, not ambient.
- The checker says nothing when the workspace is sound.
- Full Contracts appear only at boundaries or under explicit inspection.
- No other subsystem may create an independent ownership model.
- The first implementation is an interpreter; optimized plans wait until the model survives Phase -1.

### 7.1 Vocabulary tiers

The architecture has precise internal terminology, but the ordinary interface should use concepts users already know.

#### Everyday interface vocabulary

- **Draft** — a local change that has not yet reached its durable destination.
- **Save**, **sync**, or **accept** — the context-appropriate action that attempts to reconcile that draft.
- **Source** — shown only when the user needs to know which file, service, replica, or graph record will receive the change.
- **Conflict** or **review needed** — a coalesced boundary condition, not a stream of policy diagnostics.

#### Advanced user and infrastructure vocabulary

- **Jurisdiction** — the rule deciding which system governs a piece of information.
- **Holder** — the source currently selected to provide or accept that information.
- **Overlay** — the durable internal form of an unaccepted draft.
- **Promotion** — the user-facing description of a repair that successfully places an Overlay under its intended Holder or merge domain.

Terms such as Contract, Basis, claim, custody, provenance, federation, materialization, transition, causal frontier, and resolution are maintainer or runtime vocabulary. They remain available through `lim jurisdiction explain`, diagnostic details, APIs, and design documentation. Routine editing interfaces should not require them.

### 7.2 Jurisdiction subjects and the reduction of facets

Jurisdiction ultimately governs Nodes and Relations:

```rust
pub enum JurisdictionSubject {
    Node(NodeId),
    Relation(RelationId),
}
```

A domain profile may use a schema selector such as `image.caption` or `code.analysis`, but that selector is compile-time shorthand over graph structure. It is not an independently identified third primitive.

Rule:

> If a so-called facet requires its own Holder, history, identity, Relations, or repair lifecycle, reify it as a Node or Relation.

For example, an image is not one giant independently governed record:

```text
[image placement node]
  --payload--> [immutable image object]
  --caption--> [caption node]
  --crop--> [crop relation or node]
  --ocr--> [derived OCR node]
```

The profile may assign different Jurisdictions to those Nodes and Relations without introducing `FacetId` as a semantic entity.

### 7.3 Jurisdiction Contracts are internal policy, not common-case authoring

A Contract is a schema over Nodes and Relations and is normally supplied by a domain profile. The internal model may remain rich, but it should be grouped by concern rather than exposed as a flat questionnaire:

```rust
pub struct JurisdictionContract {
    pub scope: SubjectSelector,
    pub resolution: HolderResolution,
    pub mutation: MutationPolicy,
    pub continuity: ContinuityPolicy,
    pub lifecycle: LifecyclePolicy,
}
```

Where the grouped policies cover:

```text
HolderResolution:
  candidate Holders, read precedence, fallback, federation/merge engine

MutationPolicy:
  write route, foreign edits, repair authorization, domain safety checks,
  idempotency and acceptance rules

ContinuityPolicy:
  identity requirement, revision behavior, dangling-reference behavior

LifecyclePolicy:
  Overlay durability, freshness, aging, surfacing, publication
```

This is still substantial complexity. The architecture's claim is not that it vanishes; it is that profiles centralize it and common users do not repeatedly author it. Promotion does not add another lifecycle engine: it is a successful `RepairPlan` rendered in user-facing language.

### 7.4 Profile coverage and ergonomics conformance

Every standard domain package provides a default Jurisdiction profile. Profiles are selected through file type, schema, domain, workspace configuration, or explicit override.

The denominator is not “all Nodes” and not raw keystrokes. A **Jurisdiction-sensitive operation** is a read, write, repair, sync, export, publication, foreign-edit ingestion, or resolver event that must choose a Holder, Basis, write route, or reconciliation rule. Repeated downstream computations caused by one root event do not each count as new user-facing incidents.

An **editing session** is one client/workspace activity interval from explicit open to explicit close, or—when the trace lacks lifecycle events—until a fixed conformance timeout of 30 minutes without activity. A **sound session** is one whose inputs remain within the declared profile invariants: required Holders are available, identities meet declared grades, and no unresolved foreign corruption or merge dispute is injected. Sound-session silence therefore tests checker ergonomics rather than hiding real boundary conditions.

Acceptance uses a versioned, held-out trace corpus that is separate from profile-development fixtures. It should include:

- Real Git histories containing merges, rebases, moves, renames, and conflict resolutions
- Consented and anonymized editor-session traces
- Foreign-tool formatting and rewrite traces
- Offline/online transitions and resolver observations
- Adversarial mutation traces produced by an independent harness

The implementer may tune against a training corpus, but may not alter the locked acceptance corpus after seeing profile results without creating a new version and retaining the old score.

For every stable profile:

```text
auto_resolution_rate =
  automatically resolved jurisdiction-sensitive operations
  / all jurisdiction-sensitive operations

auto_resolution_rate >= 99%
manual-intervention-free ordinary sessions >= 99%
user-facing diagnostics in every sound session = 0
checker output on an entirely sound workspace = empty
unexpected reconciliation items created in every sound ordinary session = 0
visible incidents per root cause per session <= 1
manual Contract authoring in ordinary sessions = 0
```

An operation counts as automatically resolved only if it reaches the profile's declared stable outcome without manual Contract authoring, an unexpected diagnostic, or unexpected reconciliation debt. Falling back to an undeclared Overlay does not count as success. A profile-declared transient draft—such as offline work awaiting automatic synchronization—is reported separately and must reconcile within its policy window or become debt.

Coverage and session SLOs are measured per profile and domain, not only as one aggregate that can hide a weak domain. A profile that misses any threshold remains experimental. A 99% operation score does not excuse a noisy workflow; the session-level SLO prevents thousands of harmless operations from hiding repeated interruptions.

The full Contract is surfaced only when:

- A Relation crosses Jurisdictions and automatic composition cannot prove safety.
- A foreign edit damages identity or structure.
- A Holder is unavailable or read-only.
- An Overlay cannot be reconciled automatically.
- A user asks `lim jurisdiction explain`.
- A plugin or domain author defines a new profile.

### 7.5 Workspace Basis Perspectives and buffer-granular revisions

Every compiler, renderer, query, export, AI operation, and repair plan captures one immutable `WorkspaceBasis`:

```rust
pub struct WorkspaceBasis {
    pub transaction: TransactionId,
    pub perspective: BasisPerspective,
    pub components: PersistentMap<JurisdictionKey, BasisComponent>,
}

pub enum BasisComponent {
    BufferGeneration {
        client: ClientId,
        buffer: BufferId,
        epoch: SessionEpoch,
        generation: u64,
        content_hash: Option<ContentHash>,
        base_file_hash: Option<ContentHash>,
    },
    FileContent {
        path: PathId,
        hash: ContentHash,
    },
    GitCommit {
        oid: ObjectId,
    },
    GraphSnapshot {
        revision: GraphRevisionId,
    },
    ObjectContent {
        hash: ContentHash,
    },
    ExternalRevision {
        source: SourceId,
        token: RevisionToken,
    },
    Observation {
        source: SourceId,
        observed_at: Timestamp,
        hash: ContentHash,
    },
}
```

Per-buffer generation is first-class because an unsaved buffer may be the working Holder even while the file remains the durable Holder. `epoch` prevents generation collisions across editor sessions. `generation` supplies exact ordering and LSP-compatible edit identity. A content hash, computed incrementally or lazily, permits cache reuse when different generations contain identical bytes.

The Basis is logically workspace-wide but physically persistent and dependency-granular. Queries record which components they read; editing buffer A must not invalidate computations that depend only on buffer B or an unrelated image object.

Saving is presented as Promotion, but operationally it is a repair from the buffer Overlay into the file Holder:

```text
BufferGeneration(g42) --accepted repair--> FileContent(hash F9)
```

If the bytes are identical, semantic query results may be reused even though the lifecycle state changes.

The daemon may know about many candidate buffer generations, but a captured computation Basis selects at most one independent working claim per durable subject. Every workspace-scoped consumer declares a `BasisPerspective`:

```rust
pub enum BasisPerspective {
    ClientScoped { client: ClientId },
    DurableOnly,
    Published { revision: PublicationId },
    Federated { domain: FederationId, frontier: CausalFrontier },
}
```

Resolution rules:

- An interactive preview, query, or AI request defaults to the requesting client's `ClientScoped` Basis.
- A background build, reproducible export, CI job, or publication defaults to `DurableOnly` or an explicit `Published` Basis.
- A shared collaborative surface may use `Federated` only when a declared merge runtime supplies one frontier.
- Two unrelated dirty buffers over the same file remain branch-like working claims. They are never combined into one Basis.
- If one client has multiple divergent buffers for the same durable subject, the client must select one buffer in its working-holder map; otherwise that subject falls back to `DurableOnly` and the ambiguity becomes one coalesced reconciliation item.
- A consumer without a requester and without an explicit mode must fail closed to `DurableOnly`, not guess among dirty clients.

Thus Neovim and a phone may each see their own unsaved work while a durable export continues to see only accepted file state. Cross-document queries and AI context compilation must record the selected mode in provenance.

### 7.6 Standard Jurisdiction profiles

#### External-file profile

Under `BasisPerspective::ClientScoped { client }`, the requesting client's selected editor buffer is the working Holder; file bytes are the durable Holder; a Git object may be the published Holder. Other clients' dirty buffers remain separate Overlay branches and are not visible through that Basis. Syntax trees, language analysis, graph structure, and previews are derived at an exact buffer or file Basis.

Use for code, configuration, portable Markdown, portable Org, and formats consumed directly by mature external tools.

#### Graph-native profile

Graph transactions hold Jurisdiction. Text is a managed projection or deterministic serialization.

Use for native Liminal prose, comments, tasks, tables, slides, and structures requiring durable graph identity.

#### Immutable-resource profile

Content-addressed bytes hold Jurisdiction over the exact media object. Metadata, annotations, descriptions, placements, transcripts, and OCR are separate Nodes and Relations.

Use for images, audio, video, PDFs, ink, and datasets.

#### External-service profile

A service or database is the Holder. Liminal stores pointers, immutable observations, caches, mirrors, or Overlays according to profile policy.

#### Derived profile

A declared transformation over an exact Basis governs the result. Manual edits to an output create an Overlay or a new authored Node; they do not silently rewrite inputs.

#### Federated profile

Several writers participate under one explicitly selected merge runtime. This is not a small enum pretending to solve collaboration. The profile delegates to a specialized CRDT, Git, OT, database, or domain merge engine and records its causal frontier and accepted operation set.

#### Annotated-source candidate profile

Raw text and a structured annotation surface are coordinated. This is the killer case, not an assumed solution. It remains experimental until Phase -1 proves anchor recovery, foreign-edit behavior, identity guarantees, and canonical round-trip laws. The Contract cannot make two coordinated surfaces coherent by declaration alone.

### 7.7 Repair plans, Promotion, and safety

When a graph-native Relation targets an external-file Node and a foreign edit damages the target, no single Contract governs the whole repair. Promotion is not a separate mechanism; it is the user-facing name for a repair whose result has been accepted by the intended Holder or merge domain. At the semantic level, a Promotion has one terminal acceptance mutation; the repair plan may contain ordered prerequisite mutations or external steps required to make that terminal mutation valid.

A repair plan is an ordered dependency graph, not an unordered list:

```rust
pub struct RepairPlan {
    pub id: RepairId,
    pub basis: WorkspaceBasis,
    pub steps: BTreeMap<RepairStepId, ProposedMutation>,
    pub dependencies: Vec<RepairDependency>,
    pub inverse: Option<InverseRepairPlan>,
}

pub struct ProposedMutation {
    pub id: RepairStepId,
    pub subject: JurisdictionSubject,
    pub operation: Operation,
    pub expected_prestate: StatePredicate,
    pub expected_poststate: StatePredicate,
    pub idempotency_key: IdempotencyKey,
}

pub struct RepairDependency {
    pub before: RepairStepId,
    pub after: RepairStepId,
}
```

The composition rule is conjunctive:

```text
auto-apply repair
iff
  the plan has one valid result at its captured Basis
and
  for every proposed mutation:
    that subject's Jurisdiction authorizes the mutation
and
  all cross-subject identity and invariant requirements are satisfied
and
  the domain safety validator proves the result safe enough for automatic acceptance
and
  the plan is idempotent and revertible or reconstructible
```

Consequences:

- The target's foreign-edit policy controls whether the target may be modified, reidentified, or annotated.
- The Relation's identity requirement controls whether a candidate target is acceptable.
- Mutating the Relation endpoint is governed by the Relation's Holder.
- Inserting an ID into file bytes is governed by the file Node's Holder.
- Neither Contract may commandeer the other subject.
- A unique textual merge is not a safety proof.
- External-file auto-acceptance requires semantic/node-level disjointness or a domain-aware validator that proves non-interference. Otherwise Liminal offers a reviewable repair.
- Every automatically accepted repair records preimages or an inverse and supports `lim repair undo <repair-id>`.
- If one required mutation cannot be accepted, preserve the proposals as Overlays and create one linked reconciliation item.

Example:

```text
Graph-native comment Relation
  → external-file paragraph
  → Git merge removes or duplicates paragraph ID
```

If the explicit ID survives uniquely, the Relation remains sound. If only a heuristic match exists while the Relation requires explicit identity, Liminal does not reattach silently. It retains the comment, creates a proposed Relation-endpoint Overlay, optionally creates a source-ID insertion Overlay, and surfaces one reconciliation item describing the dependency between them.

### 7.8 Intent-Logged Repair Protocol

A repair that spans graph state and any nontransactional or independently transactional Holder uses the **Intent-Logged Repair Protocol (ILRP)**. This does not make the filesystem transactional. It makes partial work visible, ordered, idempotent, and recoverable.

Protocol:

1. **Prepare.** In one graph-store transaction, record the `RepairPlan`, captured Basis, dependency DAG, expected prestates and poststates, idempotency keys, required capabilities, and inverse/preimage data. Mark the intent `Prepared`.
2. **Apply.** Execute steps in topological order. Before each step, verify its prestate. For files, stage a temporary file, flush it, and use atomic replacement where the platform supports it. For services, use the repair step's idempotency key when supported.
3. **Acknowledge.** Record each completed step and observed poststate durably. Repeating a completed step must be harmless or detected as already applied.
4. **Finalize.** After all external prerequisites are satisfied, commit the graph mutations and mark the repair `Committed` in one graph transaction.
5. **Recover.** On restart, scan nonterminal intents. If the world matches a prestate, resume. If it matches a poststate, acknowledge and continue. If it matches neither, stop, preserve all data, and create a reconciliation Overlay rather than guessing.
6. **Revert.** When requested and still valid, execute the inverse plan through the same protocol.

Suggested states:

```text
Prepared → Applying → ExternalApplied → Finalizing → Committed
                     ↘ NeedsReview
Prepared/Applying    ↘ Aborted
```

The guarantee is not impossible cross-system atomicity. It is:

```text
at-least-once idempotent external application
+ exactly-once accepted graph finalization
+ durable evidence of every partial state
```

Dependency ordering is essential. Inserting a persistent ID into source bytes must precede reattaching a Relation that depends on that ID. Phase -1 must kill the daemon after each protocol step and prove resumability.

### 7.9 Jurisdiction Checker

The first checker is an interpretive reference implementation, not a compiled execution plan. It answers:

1. Which Holder does this subject resolve to at this Basis?
2. Where would a write go?
3. Is a merge runtime required?
4. Does identity satisfy every incoming durable Relation?
5. Are there unresolved Overlays?
6. Which `BasisPerspective` governs this computation?
7. Can a proposed repair be applied without crossing another Jurisdiction illegally?
8. Is the repair deterministic, safe, ordered, recoverable, and reversible at the captured Basis?

Normal behavior is silence:

```text
sound workspace → exit 0, no output, no badge, no notification
```

Detailed internal diagnostic codes may exist for tests and maintainers, but ordinary UI reduces them to familiar situations:

- Draft not yet saved or synced
- Conflicting changes need review
- Source unavailable or stale
- Repair could not be applied safely

Optimization into `CompiledJurisdictionPlan`, indexed dispatch, or generated policy code is explicitly deferred until the Phase -1 identity, repair, crash, and projection experiments stop reshaping the Contract.

### 7.10 Overlay lifecycle and Jurisdiction debt

An Overlay is durable immediately. It records its subject, base Basis, edit or operation, creation time, last activity, profile, and proposed repair policy.

Required lifecycle behavior:

1. **Attempt safe repair automatically.** If the Holder becomes available and the repair is authorized, uniquely determined, domain-safe, idempotent, and revertible, accept it and record the repair.
2. **Coalesce without erasing history.** Repeated local edits to the same unresolved subject may be presented as one current draft while retaining the operation chain.
3. **Age explicitly.** Profiles define `surface_after` and `escalate_after` thresholds.
4. **Maintain a core reconciliation queue from Phase -1.** Each aging Overlay produces or updates one graph-native reconciliation item accessible through `lim overlays` and contextual views. This queue cannot depend on the later agenda subsystem.
5. **Project into the agenda when available.** The agenda domain may render the same reconciliation items as tasks, preserving identity and status rather than copying them.
6. **Surface contextually.** Show the draft or conflict when its subject is opened and before sync, publication, destructive conversion, or workspace cleanup.
7. **Never silently collect it.** An Overlay leaves the active queue only through accepted repair, explicit discard, or an explicit archival action that remains searchable.
8. **Report debt, not noise.** Default UI uses “draft,” “pending sync,” or “review needed.” `lim overlays` and advanced Jurisdiction views expose count, age, affected domains, repair blockers, and exact Contracts.

This turns “never block capture” into deferred but visible work rather than a conflicted-copy graveyard.

### 7.11 Initial domain Jurisdiction matrix

| Domain or semantic address | Working Holder | Durable Holder | Graph role | Identity strategy | Merge/runtime |
|---|---|---|---|---|---|
| Rust or other program source | Requesting client's selected buffer generation under `BasisPerspective::ClientScoped` | File bytes; Git object for publication | Derived syntax/semantic index plus graph-native annotations | Content version + language symbol; explicit/managed ID only for durable external Relations | Text/Git or language-aware patching |
| Portable Markdown or Org | Requesting client's selected buffer generation under `BasisPerspective::ClientScoped` | Canonical text file | Derived graph; optional experimental annotations | Explicit IDs for durable blocks; otherwise anchored or heuristic continuity | Text/Git |
| Native Liminal prose | Graph transaction | Graph snapshot + operation log | Holds Jurisdiction | Stable `EntityId` + immutable `VersionId` | Graph transactions or delegated CRDT |
| Table, spreadsheet, slide | Graph transaction | Graph snapshot | Holds Jurisdiction | Stable entity + immutable version | Domain-specific engine |
| Image, audio, video, raw ink | Capture stream while recording | Content-addressed object | Metadata and Relations are separate graph subjects | Content hash for bytes; stable graph entities for placements and annotations | Append/finalize, object deduplication |
| Transcript, OCR, AI summary | Proposal or derived process | Derived Node until Promotion; authored Node after Promotion | Derived or authored according to lifecycle | Derivation ID + source Basis; new entity on Promotion when appropriate | Regenerate, compare, or human Promotion |
| External database value | Local Overlay while disconnected | External service plus immutable local observations | Pointer, observation, cache, mirror, or replica | External key + observation/version identity | Resolver or delegated federation engine |

This matrix is a starting profile set, not a hard-coded ontology.


## 8. Editing and projection modes

### 8.1 Portable file mode

An ordinary editor modifies Holder-controlled bytes. Liminal observes buffer or file revisions, parses them, and produces graph deltas. Semantic transformations are accepted only after they can be serialized into a valid source patch and reparsed to the intended result.

### 8.2 Managed textual projection mode

An editor opens a Liminal-managed virtual buffer generated from a graph-native Jurisdiction domain. Text edits become graph transactions; canonical source is regenerated from the accepted revision. This mode may hide persistent IDs and show user-specific aliases because external byte equality is not the merge protocol.

### 8.3 Rich graph mode

A browser or native rich editor emits graph transactions directly. It may use a domain-native transaction or CRDT model. Text is an export or optional managed projection, not a competing Holder.

### 8.4 Projection capability levels

Every projection declares one of these levels:

0. **Render-only** — no edits map back.
1. **Import/export** — conversion is supported with declared loss.
2. **Canonical round-trip** — `parse(emit(graph))` preserves the supported graph subset and `emit(parse(source))` canonicalizes source.
3. **Incremental bidirectional lens** — individual source or rich edits map to equivalent graph transactions with stable anchors.
4. **Collaborative bidirectional lens** — concurrent edits, marks, selections, undo, and merge preserve declared intent properties.

Liminal must not describe a Level 1 or Level 2 adapter as if it were a Level 4 editing surface.

### 8.5 Exact source preservation and foreign edits

The concrete syntax representation preserves tokens, whitespace, comments, delimiters, aliases, malformed regions, and source ranges for a specific source basis.

When file-held bytes change outside Liminal:

1. Record the new file and, where applicable, buffer Basis component.
2. Incrementally update the CST.
3. Derive a semantic delta.
4. Reuse identity only at the strength allowed by the domain's continuity policy.
5. Mark uncertain continuity as inferred rather than silently asserting it.
6. Build a mutation-local repair DAG for affected Relations. Each proposed mutation remains governed by its own subject's Jurisdiction.
7. Validate both determinism and domain safety; a clean textual merge alone is not sufficient.
8. Apply cross-Holder work through ILRP, respecting dependency ordering and exact prestates.
9. Preserve unresolved work as linked Overlays and one core reconciliation item; the agenda may project it later.
10. Commit the accepted result as a new Workspace Basis and retain a one-command inverse where possible.

Exact source holds Jurisdiction only in external-file subjects. In graph-native subjects it is provenance or a projection.

### 8.6 Pure incremental engine and effect reactor

The incremental compiler is a deterministic function of accepted Jurisdiction inputs. Network calls, database reads, transcription jobs, and other effects occur outside it.

```text
editor/file/resolver/capture event
  ↓
Jurisdiction transaction
  ↓
pure incremental query engine
  ↓
derived graph and backend outputs
```

An effectful resolver may observe the outside world and submit a transaction containing a new input or materialization. It must not run invisibly inside a tracked query. This makes invalidation, replay, testing, and reproducible freezing tractable.

---

# Part IV — The multi-level IR

## 9. L0 — Bytes and Concrete Syntax Tree

The CST preserves:

- Exact bytes
- Tokens
- Whitespace
- Comments
- Delimiters
- Attribute order
- Alias spelling
- Invalid syntax
- Source ranges
- Embedded language boundaries

Recommended properties:

- Immutable green-tree representation
- Cheap red-tree or cursor views
- Structural sharing across revisions
- Rope or piece-tree-backed source
- Error-tolerant parsing
- Incremental subtree replacement

## 10. L1 — Liminal Human IR

The Human IR preserves authorial intent and domain sugar.

It contains:

- Semantic Node identities at their declared identity grades
- Source anchors
- High-level kinds
- Unexpanded macros where useful for diagnostics
- Symbolic defaults
- Unresolved references
- Authorship provenance
- Domain-specific structures

This is the main plugin and editor-semantic layer.

## 11. L2 — Liminal Resolved Graph IR

The Resolved Graph IR is the normalized Node-and-Relation graph at an explicit Workspace Basis. It is Liminal's semantic normal form, but it does not imply that the graph originated or owns every domain payload. In external-file Jurisdiction domains it is a derived semantic materialization; in graph-native domains the graph holds Jurisdiction.

It contains:

- Expanded macros
- Canonical symbols
- Materialized semantic defaults
- Validated domain constraints
- Resolved local references
- Namespaced unknown extensions
- Explicit resource identities
- Explicit provenance
- Explicit relation policies
- Explicit effect requests
- Normalized tables, tasks, citations, and timelines

This is the preferred layer for:

- Semantic diffing
- Search and indexing
- Synchronization
- History snapshots
- Conversion
- AI lowering
- Static analysis
- Build orchestration

## 12. L3 — Domain dialect IRs

A universal graph does not eliminate domain-specific intermediate representations.

Standard dialects may include:

- Prose IR
- Org/task IR
- Academic IR
- Code IR
- Notebook IR
- Table/formula IR
- Slide IR
- Canvas/ink IR
- Media timeline IR
- Relation-resolution IR
- Build IR

These are derived views over the same resolved graph normal form at a declared basis and may use specialized structures.

## 13. L4 — Backend IRs

Targets receive their own lower IRs:

- HTML/DOM IR
- CSS/layout IR
- PDF display-list IR
- Pandoc AST adapter IR
- Typst or LaTeX adapter IR
- DOCX adapter IR
- Terminal-cell IR
- Accessibility IR
- AI MIR
- Search-index IR
- Sync plan IR
- Execution IR
- Wasm component IR

The architecture is MLIR-like in spirit: preserve the abstractions needed at each stage, then lower deliberately.

## 14. Transform contracts

Every pass declares:

```text
requires
preserves
introduces
may_discard
is_deterministic
is_reversible
has_effects
required_capabilities
```

Example:

```text
CommonMark export

preserves:
  textual content
  ordinary hierarchy
  basic emphasis
  links

may discard:
  comments
  formula executability
  slide animation
  live relation policies
  detailed layout constraints
```

The compiler must warn before a destructive conversion unless the user explicitly accepts the contract.

---

# Part V — Human source language and built-in Ruff

## 15. Surface syntax goals

The native syntax should be:

- Markdown-compatible for ordinary prose
- Compact
- Unambiguous under a declared edition
- Error tolerant
- Incrementally parseable
- Pleasant in modal editors
- Easy for AI and ordinary tools to ingest when advanced features are absent
- Fully expressible in an explicit form
- Extensible without changing the kernel

Liminal should accept ordinary Markdown as a frontend dialect. The native dialect may use `.lim`, `.lim.md`, or an edition marker once the grammar stabilizes.

## 16. Compact and explicit forms

Compact:

```markdown
# Fourier Analysis

Convolution becomes **multiplication** in the frequency domain.

- [ ] Derive the scaling property.
```

Explicit:

```liminal
node heading(level=1) {
  node text("Fourier Analysis")
}

node paragraph {
  node text("Convolution becomes ")
  node strong {
    node text("multiplication")
  }
  node text(" in the frequency domain.")
}

node task(status=open) {
  node text("Derive the scaling property.")
}
```

Both lower to the same resolved graph normal form at the same Workspace Basis.

## 17. Syntax kernel

The surface grammar needs only a few general forms:

```text
literal
node construction
relation construction
attribute assignment
ordered block
reference
expression
macro invocation
```

Domain sugar compiles into those forms.

## 18. Defaults

Every implicit default must have an explicit representation and an explanation path.

Commands:

```bash
lim expand note.lim
lim explain node:<id>
lim fmt --profile explicit note.lim
```

The canonical formatter may choose to write all semantic defaults, only non-default values, or values required for stable interpretation. The repository declares that policy.

## 19. Identity model: entity, version, anchor, and alias

Persistent identity is a load-bearing architectural boundary, not a formatting detail.

Liminal separates four concepts:

```text
EntityId     logical continuity across accepted revisions
VersionId    immutable identity of exact canonical content and dependencies
Anchor       location within one source/resource basis
Alias        human-readable name, path, heading slug, symbol name, or label
```

A Unison-like content hash is excellent for exact immutable versions and derived artifacts. It cannot by itself say that a paragraph before and after an edit is "the same paragraph." A stable `EntityId` provides that lineage when the Jurisdiction profile can guarantee it.

### 19.1 Identity grades

Every Node exposed across revisions has a declared identity grade:

- **Ephemeral** — valid only inside one parse or view.
- **Anchored** — located within one exact source/resource basis.
- **Inferred** — continuity across revisions was matched heuristically and carries confidence/provenance.
- **Explicit** — a durable identifier is serialized in Holder-controlled source.
- **Managed** — a graph-native Holder preserves the logical identity.
- **External** — an outside Holder supplies the identifier.
- **Content-addressed** — identifies one immutable semantic or byte version.

Relations declare the minimum grade they require. A transient syntax highlight may accept anchored identity. A durable cross-document transclusion or comment must require explicit, managed, or external identity. Reproducible dependencies may require a content-addressed version.

### 19.2 No magical round-trip guarantee

If arbitrary external tools may delete, duplicate, split, reorder, or recreate anonymous text, perfect logical identity is information-theoretically unavailable unless identity is present in the Holder-controlled representation or maintained by a Holder that observes the operation.

Therefore:

- Inline IDs are the strongest portable option, but remain visible outside managed projections.
- Sidecars are permitted only with explicit repair and integrity semantics; they are not assumed infallible.
- Structural paths, content similarity, Git lineage, and neighboring anchors are recovery heuristics, never proof.
- Managed projections may hide IDs because graph transactions preserve identity.
- Documents that choose portable anonymous text choose weaker identity guarantees.

Possible compact explicit identity:

```markdown
# Fourier Analysis {#fourier-analysis}
```

Possible fully explicit form:

```liminal
node heading(entity="01...") level=1 { ... }
```

### 19.3 Immutable versions and lineage

Each accepted Node version may be content-addressed after canonicalization:

```text
VersionId = hash(kind, canonical payload, semantic attributes, dependency versions)
```

The logical entity points to its current version, and revisions record lineage:

```text
Entity E at revision R1 → Version V1
Entity E at revision R2 → Version V2
```

For externally edited external-file Jurisdiction domains, Liminal may infer that a new version continues an old entity. The fact that continuity was inferred remains visible and reviewable.

## 20. Formatter and linter commands

Liminal should have a built-in Ruff-like toolchain:

```bash
lim fmt       # deterministic source formatting
lim check     # schema, relation, capability, and invariant validation
lim fix       # apply safe automatic transformations
lim lint      # style and semantic diagnostics
lim expand    # show desugared graph-oriented source
lim explain   # show why a node, default, or render result exists
lim migrate   # update syntax edition or schema version
lim diff      # semantic graph diff
lim graph     # inspect or visualize a subgraph
lim trace     # inspect incremental passes and resolver activity
lim doctor    # integrity and environment checks
lim verify    # reproducibility and artifact verification
```

Formatting must be idempotent. Parsing canonical formatting must recover the same semantic graph.

---

# Part VI — Compiler and incremental runtime

## 21. Compilation pipeline

```text
source bytes
  ↓
coarse structural scan
  ↓
lossless CST
  ↓
Human IR elaboration
  ↓
macro expansion
  ↓
schema/default resolution
  ↓
Resolved Graph IR
  ↓
domain analysis and relation resolution
  ↓
backend lowering
  ↓
HTML / rich view / PDF / AI / sync plan / executable output
```

## 22. Coarse structural scan

The first pass should cheaply identify:

- Files and document roots
- Block boundaries
- Headings
- Lists
- Fences
- Directives
- Embedded language regions
- Resource references
- Potential node boundaries
- Order

Unneeded regions remain stubs:

```rust
pub struct OpaqueBlock {
    pub range: SourceRange,
    pub hash: ContentHash,
    pub coarse_kind: CoarseKind,
}
```

## 23. Demand-driven parsing

Fully parse only what is:

- Visible
- Edited
- Queried
- Referenced
- Required by a pass
- Needed by a backend

Large books, codebases, lecture archives, and media-heavy workspaces should not be eagerly elaborated in full.

## 24. Incremental query engine

Every expensive pure computation should be a revisioned query over an explicit Workspace Basis:

```text
parse_block(block_id, source_basis)
elaborate_node(node_id, graph_basis)
validate_subgraph(root_id, dialect_set, graph_basis)
render_node(node_id, target, profile, graph_basis)
compile_ai_context(query, model_profile, graph_basis)
calculate_formula(cell_id, dependency_basis)
```

External resolution is deliberately absent from this list. A resolver performs effects outside the query engine and submits a new Jurisdiction transaction; pure queries then consume that accepted input.

Properties:

- Deterministic results for a declared basis
- Immutable results
- Dependency tracking
- Memoization
- Precise invalidation
- Parallel scheduling where safe
- Equality checks to prevent cascading unchanged results
- Replay from frozen Jurisdiction inputs
- Diagnostics when a query mixes incompatible bases

### 24.1 Durability and volatility

Inputs receive a durability class describing how often they are expected to change and how broadly that change should invalidate work:

```text
Immutable   content-addressed objects and frozen schema versions
High        dialect definitions, formatter editions, project manifests
Medium      persisted document and code files
Low         live external observations, working buffers, device state
Ephemeral   cursor, viewport, transient selection, streaming partials
```

Durability is an invalidation optimization, not a truth or freshness policy. A stock observation may be low durability but still the selected Holder value for the exact observation time. Freshness remains a separate Relation policy.

An edit should perform approximately:

```text
working-Holder edit
→ new buffer basis
→ nearby relex
→ affected subtree reparse
→ semantic delta at that basis
→ invalidate dependents by durability and dependency edges
→ lower requested affected nodes
→ patch visible output
```

## 25. Pass manager

Passes declare dependencies, contracts, and whether they can fuse.

Optimizations include:

- Macro expansion erasure
- Default constant folding
- Alias canonicalization
- Dead transform elimination
- Traversal fusion
- Direct dispatch specialization
- Relation policy compilation
- Cached render fragments
- Cached resource derivatives
- Backend-specific lowering

## 26. Lazy dialect loading

A dialect package contributes:

- Syntax extensions
- Node kinds
- Relation kinds
- Validation rules
- Defaults
- Transformations
- Commands
- Formatters
- Renderers
- AI projections
- Migrations
- Required capabilities

The compiler loads a dialect only when the manifest, source, graph, or active target requires it.

## 27. HTML fast path

The first-class hot path is:

```text
Markdown-compatible source → incremental graph update → HTML/DOM patch
```

A paragraph edit should not recompile the entire document. The native HTML backend should generate either:

- A complete deterministic HTML document for builds, or
- Minimal DOM patches for previews and rich views.

Pandoc remains available for broad conversion, but ordinary HTML preview should not require spawning Pandoc after every keystroke.

---

# Part VII — Transformation and macro system

## 28. Graph rewrite calculus

The transformation runtime is the Turing-complete operational core of editing.

A transformation may:

- Match Nodes and Relations
- Bind values
- Read payloads
- Create Nodes
- Delete Nodes
- Add or remove Relations
- Replace payloads
- Move ordered children
- Branch
- Iterate
- Recurse
- Compose transformations
- Request effects through capabilities

This can model every ordinary editing operation, compiler pass, formatter, formula, query, macro, refactoring, and AI edit.

The transformation runtime is imperative infrastructure. It is not a third semantic primitive because programs and their results are represented as graph structures.

## 29. Macro levels

### Literal macros

```text
Ctrl+Space, p → println!();
```

### Snippet macros

Insert templates with typed placeholders and cursor stops.

### Syntax-aware macros

Use parser and language-server context to insert a valid construct.

### Structural macros

Transform selected syntax or document Nodes instead of splicing strings.

### Graph macros

Create or rewrite arbitrary Nodes and Relations.

### Workflow macros

Compose editor commands, document transformations, build actions, relation resolutions, and views.

## 30. Pseudo-stenographic command grammar

The macro system should support compositional phrases:

```text
construct + modifier + modifier + target
```

Example:

```text
function + public + async + Result
```

Rust pack:

```text
p       println
pd      debug println
f       function
fa      async function
fpr     public Result-returning function
m       match
mo      match Option
mr      match Result
is      if let Some
io      if let Ok
tt      unit test
ta      async test
```

The editor owns keybindings and chord activation. Liminal owns portable semantic commands and macro definitions.

## 31. Context and precedence

Macro resolution order:

```text
buffer override
workspace override
framework pack
language pack
document-domain pack
user pack
global fallback
```

Context includes:

- Active language
- Syntax node
- Selection
- Nearby schema
- Project conventions
- Editor mode
- Active view

## 32. Discoverability

An optional Which-Key-style menu shows valid continuations after a chord. Ranking may use context, frequency, recency, and project conventions.

## 33. Semantic macro recording

Traditional editor keystroke macros remain editor-owned.

Liminal additionally records semantic command sequences:

```text
select enclosing function
add async modifier
wrap return in Result
insert tracing attribute
format scope
```

These replay intent across structurally different documents.

## 34. Macro implementation tiers

- Declarative expansion rules for common macros
- Lua for trusted local scripting
- Wasm for portable third-party transformations
- Native Rust for trusted hot paths
- Process plugins for heavyweight tools

## 35. Macro optimization

Do not execute a nested tower of wrappers.

```text
high-level macro composition
  ↓ expansion
normalized transformation plan
  ↓ specialization and fusion
compact executable operations
```

Repeated macros may compile to Wasm or native code. Ordinary one-shot operations should remain interpreted or directly dispatched unless profiling proves otherwise.

---

# Part VIII — Intelligent Relations and external state

## 36. Relation modes

The difference between a Wikipedia link, a stock price, and book progress is not a need for new primitives. It is a difference in Relation policy.

Standard policies:

### Direct reference

The target exists locally.

### External pointer

The target is identified but not possessed.

### Snapshot

An immutable local copy of external state.

### Cache

A replaceable local copy with freshness policy.

### Mirror

A persistent local copy synchronized from an Holder-controlled source.

### Live relation

A value expected to reflect current external state when available.

### Materialized relation

A persistent local Node produced from a query, computation, or remote source.

### Replicated relation

Local and remote state are peers expected to converge.

### Derived relation

A local output depends on other local graph state.

### Transclusion

A view embeds the current target while preserving target identity.

### Copy macro

A new snapshot Node is created with a provenance Relation to the source. Duplication therefore becomes an explicit policy rather than an ugly untracked copy.

## 37. Relation declaration

A Relation policy may specify:

- External identity
- Jurisdiction
- Local materialization
- Availability
- Freshness
- Refresh trigger
- Offline behavior
- Conflict policy
- History policy
- Security capabilities
- Reproducibility mode
- Rendering behavior

## 38. Examples

### Wikipedia

```liminal
@external(
  uri="https://...",
  materialize=metadata,
  refresh=manual,
  offline=cached-summary
)
```

### Last-known stock price

```liminal
@market-price(
  ticker="AAPL",
  materialize=persistent,
  refresh="15m",
  stale-after="5m",
  offline=last-known,
  history=append-observation
)
```

### Reading progress

```liminal
@reading-progress(
  book=@book-42,
  sync=bidirectional,
  offline=editable,
  conflict=max-progress
)
```

These macros expand into ordinary Nodes, Relations, policy Nodes, and compiled resolver plans.

## 39. Effect isolation

A Relation never performs network work merely because it is inspected.

```text
declarative relation
  ↓ compiler
resolver request plan
  ↓ capability runtime
network/database/process effect
  ↓ transaction
materialized graph update
```

## 40. Availability and freshness

Resolution exposes distinct dimensions:

```rust
pub enum Availability {
    Absent,
    MetadataOnly,
    Partial,
    Materialized,
}

pub enum Freshness {
    Current,
    Stale { since: Timestamp },
    Frozen,
    Unknown,
}
```

Identity, local possession, and currentness must never be conflated.

## 41. Ownership and conflict

Ownership modes:

- External-Holder
- Local-Holder
- Bidirectional
- Append-only
- Derived
- Frozen snapshot
- Ephemeral cache

Conflict policies:

- Last writer wins
- Maximum or minimum
- Set union
- Three-way merge
- Sequence CRDT merge
- Manual review
- Custom transformation

## 42. Reproducible freeze

A command should convert live or remote Relations into frozen materializations for publication, archival, or deterministic builds:

```bash
lim freeze paper.lim
```

The frozen graph records source identity, retrieval time, content hash, resolver version, and provenance.

---

# Part IX — Physical representation and zero-cost abstractions

## 43. Logical graph, specialized stores

The logical graph remains uniform. Physical stores do not.

Potential stores:

- Node header arena
- Text store
- Ordered container store
- Scalar store
- Resource store
- Table store
- Expression store
- Ink store
- Media timeline store
- Generic extension store

## 44. Node layout

Avoid universal string-keyed maps in hot paths.

```rust
pub struct NodeHeader {
    pub kind: KindId,
    pub storage_class: StorageClass,
    pub flags: NodeFlags,
    pub revision: RevisionId,
}
```

Common data uses typed columns and compact indexes. Rare extension properties may fall back to sparse generic storage.

Persistent 128-bit IDs may map to dense 32- or 64-bit in-memory handles.

## 45. Relation layout

Different relation families compile to different structures:

- Containment/order → persistent sequence
- Local references → adjacency arrays
- Reverse references → inverted indexes
- Formula dependencies → DAG-oriented graph
- Text annotations → interval or anchor index
- Temporal alignment → interval tree
- External relations → resolver-plan table
- Collaborative text/order → CRDT-specialized sequence
- Layout constraints → constraint graph

## 46. Text subsystem

Text is physically privileged.

Required capabilities:

- UTF-8 storage
- Grapheme-aware navigation
- Fast line lookup
- Efficient insertion and deletion
- Stable anchors
- Shared immutable slices
- Incremental parsing
- Interval marks
- Minimal-copy split and join
- Collaboration support
- Undo summaries

Candidate structures may include ropes, piece trees, persistent B-trees, and small-buffer specializations. The implementation should benchmark rather than commit ideologically.

Individual characters must not become ordinary graph allocations.

## 47. Resources

Large binary payloads use a content-addressed object store:

```text
Node → content hash → chunked object
```

Benefits:

- Deduplication
- Integrity verification
- Incremental synchronization
- Immutable caching
- Reproducible outputs
- Shared thumbnails and derivatives

## 48. Zero-cost abstraction definition

In Liminal, “zero cost” means:

- Surface sugar disappears after elaboration.
- Defaults are folded.
- Aliases become interned canonical symbols.
- Relation policy composition becomes one compiled plan.
- Common schemas become direct layouts and dispatch tables.
- Unused domains are not loaded.
- Repeated passes fuse where semantics allow.
- Hot plugins may be statically linked or AOT-compiled.
- The generic graph fallback is not forced into every common operation.

It does not mean every semantic feature consumes literally zero CPU or bytes.

## 49. JIT and Cranelift

Cranelift is reserved for workloads with demonstrated repeated computation:

- Large formulas
- Repeated graph queries
- Data transformations
- Custom visualization kernels
- Hot macro pipelines
- Domain-specific expressions
- Layout calculations that actually benefit

Tiering:

```text
expression or transformation
→ interpreter/direct execution
→ profiling threshold
→ Cranelift native code
```

Web targets lower to Wasm. Ordinary parsing and HTML rendering remain optimized Rust and cached queries.

## 50. Memory and startup

Design targets:

- Memory-map large immutable snapshots where useful.
- Load document skeletons before payloads.
- Load visible nodes and nearby nodes first.
- Defer resource decoding.
- Persist compiled schema layouts.
- Preload only configured common dialects.
- Use Cargo features and profile compilation for small personal binaries.

---

# Part X — Workspace, build system, and orchestration

## 51. One workspace, many domains

A workspace graph can contain:

```text
workspace/
├── software/
│   ├── Rust source
│   ├── architecture notes
│   └── generated API docs
├── research/
│   ├── papers
│   ├── datasets
│   ├── notebooks
│   └── citations
├── book/
│   ├── manuscript
│   ├── world notes
│   └── illustrations
└── operations/
    ├── tasks
    ├── meetings
    └── roadmap
```

Relations connect them:

- Source symbol → design note
- Paper claim → dataset
- Figure → notebook output
- Slide → paper section
- Task → code issue
- Lecture timestamp → handwritten equation
- Book scene → character node

## 52. Build graph

`lim build` constructs a dependency graph across documents, code, data, and external tools.

Build units may include:

- HTML site
- PDF paper
- EPUB book
- Slide deck
- Rust executable
- Notebook output
- Generated figure
- Search index
- AI context package
- Frozen archive

Build caching uses content hashes, graph revisions, tool versions, capability inputs, and declared environment state.

## 53. Tool sovereignty

Liminal orchestrates:

- Cargo for Rust
- Other language build systems for their languages
- LSPs for code analysis
- Pandoc for broad conversion
- Typst or LaTeX for typesetting where chosen
- Jupyter kernels or process adapters for notebooks
- External transcription engines
- OCR and handwriting engines
- Git for repository history

Liminal does not pretend those are simple internal macros when process isolation or tool-specific semantics are valuable.

## 54. Cargo usage

Use Cargo heavily for the implementation:

- Workspace management
- Feature selection
- Build profiles
- Testing
- Benchmarks
- Documentation
- Packaging
- Dependency auditing
- Wasm targets
- Static native plugin integration

Documents themselves use `Liminal.toml`, not `Cargo.toml`, unless they are also Rust projects.

## 55. Unix tools and fused paths

Standalone tools:

```text
lim-parse
lim-fmt
lim-check
lim-query
lim-render
lim-build
lim-run
lim-sync
lim-index
lim-ai
lim-serve
lim-transcribe
lim-resolve
lim-pack
```

Composable shell use:

```bash
lim-parse note.lim | lim-check | lim-render --to html
```

Production use:

```bash
lim build
```

The fused command calls the same library crates directly, avoids serialization, shares caches, and fuses passes.

## 56. Deployment modes

### Library-fused

One optimized binary links selected domains and hot plugins.

### Persistent daemon

`liminald` owns:

- Graph revisions
- Parsed trees
- File watchers
- Incremental query cache
- Indexes
- Resolver runtime
- Plugin runtime
- Build state
- Sync state
- Resource handles

### Process pipeline

Used for shell composition, debugging, isolation, and heavyweight external tools.

---

# Part XI — Editors, protocols, and interaction modes

## 57. Editor boundary

Editors own:

- Modes
- Motions
- Selections
- Registers
- Windows
- Buffers
- Keymaps
- UI layout
- Native macros
- Cursor presentation

Liminal owns:

- Semantic graph
- Document parsing
- Cross-document references
- Formatting
- Graph commands
- Preview compilation
- Resource insertion
- History and provenance
- Relation resolution
- Workspace orchestration
- AI projections

## 58. Liminal Document Protocol

An editor-neutral protocol should expose:

```text
open_document
apply_text_edit
apply_graph_transaction
get_diagnostics
get_semantic_tokens
resolve_reference
find_references
format_document
format_range
run_command
compile_preview
insert_resource
query_graph
inspect_node
subscribe_revision
```

Transports:

- Embedded Rust API
- Unix socket
- MessagePack RPC
- JSON-RPC debug mode
- WebSocket
- Browser worker messages
- Mobile FFI

## 59. Neovim

Neovim is the first-class text frontend because the user already values its action-first grammar and uses LazyVim.

The Liminal plugin should be thin Lua:

- Connect to `liminald`
- Publish text changes
- Display diagnostics and semantic tokens
- Open previews, graph inspectors, agendas, timelines, and relation status
- Invoke semantic macro commands
- Expose embedded code blocks to normal language servers

Liminal does not own the Neovim setup. It may publish an optional `liminal-brian` configuration pack and example keymaps.

## 60. Helix and other modal editors

Helix, Emacs, VS Code, and future Rust-native modal editors should connect through the same protocol. An “evil Helix” or Rust modal frontend can exist later without changing the graph or compiler.

## 61. Rich web editor

A web editor may use a mature rich-editing transaction engine as an adapter. It maps rich selections and structural edits to Liminal graph transactions.

The browser should not become the only runtime.

## 62. Desktop workspace mode

A combined desktop session may present:

```text
┌──────────────────────────┬─────────────────────────┐
│ Neovim / modal text      │ Pen canvas / PDF       │
│ editor                   │ annotation             │
├──────────────────────────┴─────────────────────────┤
│ Audio waveform · transcript · markers · preview  │
└────────────────────────────────────────────────────┘
```

These are separate tools coordinated through one graph and daemon.

## 63. Phone mode

Capture-first interface:

```text
[ Type ] [ Speak ] [ Photo ] [ Scan ] [ Draw ]
```

Capabilities:

- Instant inbox capture
- Phone keyboard editing
- Voice notes and transcription
- Camera and document capture
- Quick tasks
- Search and reading
- Offline queue
- Optional external-keyboard modal mode

Organization must never block capture.

## 64. Tablet mode

- Page-based notes
- Infinite canvas
- Pen input
- PDF annotation
- Lecture audio and transcript
- Typed blocks mixed with ink
- Spatial diagrams linked to ordinary text Nodes

## 65. Headless and CI mode

The full compiler, checker, formatter, renderer, indexer, and test runner must work without a GUI.

---

# Part XII — Domain subsystems

## 66. Prose and rich text

Capabilities:

- Paragraphs, headings, lists, emphasis
- Annotations and comments
- Styles as semantic roles
- Outline and folding
- Transclusion
- Backlinks
- Word count and revision views
- HTML, print, and ebook compilation

## 67. Org-style planning

Tasks may include:

- Status
- Priority
- Scheduled time
- Deadline
- Recurrence
- Effort estimate
- Clocked time
- Dependencies
- Project
- Context
- Assignee
- Completion time

The agenda is a graph query, not a privileged file.

Other capabilities:

- Recurring tasks
- Clocking
- Capture templates
- Query views
- Calendar adapters
- Spaced-repetition and flashcard domain package

## 68. Code and programming

Code remains text owned by the editor and language ecosystem.

Liminal provides:

- Code Nodes and embedded-language regions
- Virtual documents for LSPs
- Cross-links between code and prose
- Build orchestration
- Diagnostics as anchored Relations
- Execution outputs with provenance
- Semantic code macros through LSP or parser adapters
- Generated API documentation links

## 69. Notebook and literate computation

A notebook graph contains:

- Prose
- Code
- Inputs
- Outputs
- Visualizations
- Environment metadata

Execution records:

- Code hash
- Input hashes
- Environment identity
- Tool version
- Capability set
- Output hash
- Cache state
- Reproducibility status

Generated output never silently replaces authored source.

## 70. Academic publishing

Capabilities:

- Citations and bibliography records
- Equations and math ASTs
- Cross-references
- Figures and tables
- Footnotes and endnotes
- Author metadata
- Abstracts
- Templates
- Journal styles
- Frozen external data
- PDF, HTML, DOCX, LaTeX, and Typst/Pandoc adapters

Academic semantics remain separate from page layout.

## 71. Books and long-form writing

Capabilities:

- Chapters, scenes, sections
- Character, place, event, and timeline graphs
- Research notes
- Outline views
- Revision history
- Comments and suggestions
- Word-count goals
- Print, EPUB, HTML, and manuscript export
- AI context by chapter, arc, character, or scene relation

## 72. Tables and spreadsheet computation

A table is logically Nodes and Relations but physically may use columnar storage.

Capabilities:

- Typed columns and cells
- Formulas
- Named ranges
- Dependency graph
- Recalculation
- Array and query outputs
- CSV and spreadsheet import/export
- Grid view
- Provenance
- AI projection

Formula evaluation uses a specialized imperative subsystem and may be JIT-compiled only when profitable.

## 73. Slides and presentations

A presentation graph contains:

- Slides
- Shapes
- Text
- Images
- Tables
- Notes
- Themes
- Layout constraints
- Transitions
- Animations
- Temporal relations

Spatial selection, hit testing, snapping, constraint solving, and animation are specialized subsystems. They do not require new semantic atoms.

## 74. Images and PDFs

Image Nodes support:

- Content-addressed originals
- Alt text
- Captions
- Cropping and transforms
- Region references
- OCR text
- AI descriptions
- Thumbnails
- Version history
- Provenance

PDF support includes:

- Page references
- Region annotations
- Text extraction when available
- Ink overlays
- Citations and links

The original media object holds Jurisdiction over its bytes; OCR and descriptions are derived subjects.

## 75. Handwriting, drawing, and canvas

Ink stores:

- Coordinates
- Pressure
- Tilt
- Timing
- Stroke grouping
- Device calibration
- Page/canvas transform

Derived representations:

- SVG preview
- Raster preview
- Recognized text
- Recognized mathematics
- Diagram structure

Raw strokes are never discarded by recognition.

Modes:

### Page mode

For lectures, assignments, print, and PDF annotation.

### Canvas mode

For diagrams, concept maps, architecture, and brainstorming.

## 76. Audio, voice, and transcription

Capabilities:

- Voice notes
- Dictation
- Live transcription
- Deferred high-quality transcription
- Speaker segmentation
- Timestamp alignment
- Vocabulary packs
- Voice commands
- Audio annotations
- Search over speech

The audio object holds Jurisdiction over the captured signal. Transcript Nodes retain:

- Source interval
- Model identity and version
- Confidence
- Creation time
- Human correction state
- Provenance

## 77. Lecture mode

One command should:

1. Create a course document.
2. Start audio recording.
3. Start draft transcription.
4. Timestamp typed Nodes.
5. Timestamp ink strokes.
6. Record importance markers.
7. Capture slides, URLs, images, and files.
8. Align transcript segments with note sections.
9. Run a higher-quality post-lecture pass.
10. Propose summaries, tasks, and flashcards without overwriting source material.

Typed notes, handwriting, slides, transcript, and audio form one temporal graph.

## 78. Video

Video Nodes support:

- Content or external identity
- Time ranges
- Captions or transcripts
- Frame references
- Annotations
- Linked notes
- Derived thumbnails
- Provenance

---

# Part XIII — AI-native compilation

## 79. AI is a separate compiler target

The AI subsystem should generally run as a separate process or service because models, embeddings, and media handling have different memory, privacy, and dependency requirements.

```text
Resolved Graph IR
  ↓ context selection
AI MIR
  ↓ model profile
Markdown / tagged text / JSON / graph stream / multimodal bundle
```

## 80. Optimization objective

Do not optimize solely for tokenizer length.

Optimize:

```text
semantic clarity
+ relation visibility
+ provenance
+ task relevance
+ structural regularity
+ model familiarity
- ambiguity
- irrelevant context
- token cost
```

The goal is maximum useful semantic bandwidth per token.

## 81. AI MIR

AI MIR may contain:

- Document and section boundaries
- Text
- Code
- Tables
- Citations
- Tasks
- Resource descriptions
- Provenance
- Confidence
- Relation edges
- Omission markers
- Stable short IDs
- Source revision

## 82. Model-specific lowering

Profiles specify:

- Context budget
- Preferred syntax
- Tokenizer
- Image and audio support
- ID strategy
- Table strategy
- Provenance detail
- Reference expansion policy
- Summarization policy

Possible outputs:

- Normalized Markdown
- XML-like tags
- Compact S-expressions
- JSON tool input
- Edge tables
- Native multimodal attachments

A compact schema dictionary may be emitted once, followed by terse node records to reduce repetitive tokens.

## 83. Context planning

The AI compiler selects a relevant subgraph based on:

- User request
- Current document and cursor
- Reference graph
- Search
- Recency
- Provenance
- Security policy
- Token budget

It may:

- Expand abbreviations
- Resolve links
- Flatten transclusions
- Normalize tables
- Include relevant image descriptions
- Include aligned transcript intervals
- Omit irrelevant visual layout
- Mark omitted subgraphs explicitly

## 84. AI output

AI should return graph operations with revision preconditions:

```text
insert node after target
replace node payload
add relation
set task status
attach citation
create summary node
```

Raw text patches remain a fallback.

AI-generated content is marked derived until approved. Human approval may convert it into authored content while preserving provenance.

## 85. Evaluation harness

Benchmark projections by:

- Tokens per semantic fact
- Retrieval accuracy
- Reference resolution
- Structural understanding
- Table question accuracy
- Code-edit correctness
- Graph-operation success
- Hallucination rate
- Provenance retention

---

# Part XIV — History, synchronization, cloud, and storage

## 86. Transactions and operations

Every accepted semantic change is a transaction containing operations such as:

- Create Node
- Delete Node
- Change payload
- Add Relation
- Remove Relation
- Insert ordered child
- Move child
- Attach resource
- Materialize external value
- Apply transformation

A transaction records:

- ID
- Actor
- Parent revision
- Logical time
- Origin device
- Provenance
- Capability authorization
- Optional inverse
- Optional exact text edit
- Human, plugin, AI, or remote origin

## 87. Perfect recall

History uses:

```text
snapshot + append-only transaction segments
```

It supports:

- Undo and redo
- Branching
- Semantic diff
- Blame and provenance
- Time travel
- Replay
- Optional compaction policies

The user may keep full history indefinitely. Compaction must never be mandatory.

## 88. Live synchronization

Live sync operates over semantic operations and specialized collaborative sequences.

Requirements:

- Offline edits
- Eventual convergence
- Device identities
- Encrypted transport
- Partial workspace sync
- Resource chunking
- Conflict diagnostics
- Presence as ephemeral view state

Existing CRDT engines should be evaluated before custom implementation.

## 89. Git

Git is used for:

- Canonical source
- Configuration
- Human-readable milestones
- Branching
- Publishing
- Code and schema history

Git is not the only live collaboration protocol.

## 90. Cloud adapters

Replaceable transports may include:

- Dumb encrypted blob relay
- S3-compatible storage
- WebDAV
- LAN peer-to-peer
- User-selected cloud providers
- Self-hosted servers

The server should not require plaintext semantic access unless the user deliberately enables server-side functionality.

## 91. Backup

Sync is not backup.

Support:

- Local snapshots
- External-drive backup
- Encrypted remote backup
- Object integrity scans
- Recovery drills
- Rebuild tests from source and resources

## 92. Crash consistency

- Append graph operations atomically.
- Use checksummed segments.
- Write snapshots with atomic replacement.
- Verify content-addressed objects.
- Recover after interrupted writes.
- Provide salvage and `lim doctor` modes.
- Coordinate every cross-Holder repair through ILRP rather than implying a distributed transaction.
- Persist the repair intent before the first external mutation.
- Give every repair step an idempotency key plus expected prestate and poststate.
- Stage and atomically replace individual files where supported; never claim multi-file atomicity.
- Resume, finalize, revert, or surface `NeedsReview` after a crash according to observed state.
- Run daemon-kill tests at every repair transition in CI.

The graph store acts as coordinator because it can transactionally retain intent and progress. This grants recoverability, not supranational Jurisdiction over the files or services being changed.

---

# Part XV — Plugins and capabilities

## 93. Plugin categories

Plugins may contribute:

- Dialects
- Syntax sugar
- Schemas
- Macros
- Transformations
- Renderers
- Importers and exporters
- Resolvers
- Indexers
- AI projections
- Editor commands
- Build adapters
- Device integrations

## 94. Lua

Use for:

- Trusted personal customization
- Macro definitions
- Quick graph transforms
- Formatting preferences
- Editor-facing commands
- Configuration logic

Lua APIs must expose stable semantic operations, not internal Rust pointers.

## 95. WebAssembly

Default third-party runtime:

- Sandboxed
- Portable
- Browser compatible
- Capability controlled
- Language independent
- Versioned through a component interface

Use the Wasm component model and explicit interfaces where practical.

## 96. Native Rust

Use for:

- Core parsers
- Hot renderers
- Storage engines
- Compression
- Hardware integrations
- Trusted internal dialects

Prefer static Cargo integration or out-of-process boundaries over unstable dynamic Rust ABIs.

## 97. Process plugins

Use for:

- Compilers
- Language runtimes
- Transcription models
- OCR
- Databases
- GPU workloads
- Hardware services
- Untrusted or crash-prone tools

## 98. Capabilities

Possible capabilities:

- Read graph
- Read selected subgraph
- Write derived Nodes
- Write authored Nodes
- Add Relations
- Read or write resources
- Read filesystem paths
- Write filesystem paths
- Network access to allowed domains
- Microphone
- Camera
- Process execution
- Credentials by named handle
- Database access

Effects are logged and attributable.

---

# Part XVI — File format and project layout

## 99. Project layout

```text
project/
├── Liminal.toml
├── Liminal.lock
├── documents/
│   ├── index.lim.md
│   └── notes.lim.md
├── objects/
│   └── ab/cdef...
├── state/
│   ├── snapshots/
│   └── operations/
├── cache/
│   ├── graph/
│   ├── render/
│   └── ai/
├── index/
│   ├── text/
│   ├── relations/
│   └── embeddings/
└── config/
    ├── macros/
    └── profiles/
```

Indexes and caches are rebuildable. Holder-controlled source or graph state, persistent resources, declared identity, Relations, Workspace Bases, and required history are not silently disposable.

## 100. Packed interchange

A `.limpack` archive may contain:

```text
manifest
canonical documents
resolved graph snapshot with Workspace Basis
selected operation history
content-addressed objects
schema and plugin lock data
signatures
```

The archive is for transfer and preservation, not the only editable form.

## 101. Manifest

```toml
[project]
name = "example"
edition = "2027"
root = "documents/index.lim.md"

[domains]
prose = "1"
academic = "1"
rust = "1"
table = "1"

[plugins]
citations = { version = "2", runtime = "wasm" }
diagram = { version = "1", runtime = "wasm" }

[format]
canonical_profile = "explicit-stable"
line_width = 100
persistent_ids = "when-required"

[sync]
provider = "local-first"
encryption = true
```

---

# Part XVII — Configuration and Brian’s optimized profile

## 102. Configuration layers

Precedence:

```text
built-in defaults
→ global installation
→ user profile
→ device profile
→ workspace
→ document
→ target
→ temporary command override
```

## 103. Personal profile

```toml
[profile.brian]
editor = "nvim"
syntax = "compact"
canonical_format = "on-save"
preview = "incremental-web"
macro_packs = ["rust", "prose", "academic", "lecture"]

[profile.brian.performance]
persistent_daemon = true
fuse_passes = true
incremental_indexes = true
preload = ["rust", "markdown", "html"]
static_plugins = ["core-prose", "core-rust", "core-html"]
lazy_domains = true
```

The core does not require this configuration. It is an optimized personal distribution.

## 104. Compiled configuration

`lim config compile brian` may:

- Validate configuration
- Resolve plugin versions
- Precompile macro tries
- Precompute schema layouts
- AOT-compile Wasm plugins
- Select Cargo features
- Generate a fused runtime profile
- Omit unused domains

This creates a tight personal path without corrupting the modular architecture.

---

# Part XVIII — Interoperability

## 105. Initial adapters

- CommonMark and GitHub-flavored Markdown
- HTML
- Pandoc AST
- Org mode
- LaTeX
- Typst
- DOCX through Pandoc or dedicated adapter
- PDF output through chosen typesetter
- EPUB
- CSV
- Jupyter notebooks
- CSL JSON and BibTeX
- Standard image formats
- Opus and common audio formats
- SVG
- Standard calendar/task export where useful

## 106. Embedded programming tools

- LSP
- DAP where appropriate
- Jupyter kernel protocol or process adapter
- Cargo and other build systems
- Formatter and linter integrations

## 107. Unknown constructs

Importers must preserve unsupported constructs as namespaced opaque Nodes with source payload and declared loss status. Round-trip degradation must be explicit rather than silent.

---

# Part XIX — Security, provenance, and reproducibility

## 108. Security rules

- No implicit network access.
- Credentials are referenced by named secure handles, never embedded in documents.
- Remote content is untrusted.
- HTML is sanitized according to target policy.
- Wasm plugins receive minimal capabilities.
- Native plugins are explicitly trusted.
- Process plugins are isolated where practical.
- Resolver effects are logged.
- AI receives only authorized subgraphs.

## 109. Provenance

Every derived Node may record:

- Source Node or resource
- Transformation or model
- Version
- Time
- Actor
- Input revision
- Confidence
- Approval state
- Content hash

## 110. Deterministic mode

A deterministic build requires:

- Frozen external Relations
- Locked plugin and schema versions
- Declared tool versions
- Content-addressed inputs
- Stable formatting and ordering
- Controlled time, randomness, locale, and environment inputs

## 111. Accessibility

Accessibility is a compiler target and semantic requirement:

- Alt text
- Heading hierarchy
- Table semantics
- Reading order
- Captions
- Transcript alignment
- ARIA and screen-reader lowering
- Keyboard-accessible rich views

---

# Part XX — Testing and hardening

## 112. Correctness properties

Required properties:

```text
parse(format(parse(x))) ≡ parse(x)
incremental_compile(x, edits) ≡ full_compile(apply(x, edits))
semantic_diff(apply(tx, g), g) matches tx
resolver replay with frozen inputs is deterministic
sync replicas converge under supported operation schedules
captured Workspace Bases never combine independent dirty buffers for one durable subject
repair recovery after any injected crash reaches Committed, NeedsReview, or Aborted without hidden half-state
automatic repair is authorized, domain-safe, idempotent, and one-command revertible
Promotion semantics are observationally equivalent to the corresponding accepted RepairPlan
```

## 113. Test classes

- Unit tests
- Property-based tests
- Parser fuzzing
- Formatter fuzzing
- Malformed source recovery tests
- Semantic round-trip tests
- Differential conversion tests
- Golden HTML/PDF/render tests
- Snapshot migration tests
- Plugin capability tests
- Sync simulation and partition tests
- Crash-recovery tests with daemon termination after every ILRP step
- Concurrent-client Basis-selection tests
- Held-out Git/editor/foreign-tool trace replay
- Automatic-repair revert tests
- Object corruption tests
- AI-operation validation tests
- Large-workspace performance tests

## 114. Conformance suite

Maintain a separate corpus containing:

- Syntax fixtures
- Graph fixtures
- Expected diagnostics
- Format contracts
- Conversion-loss expectations
- Plugin ABI tests
- Relation resolver tests
- Incremental equivalence tests
- Versioned held-out Jurisdiction traces with operation and session labels
- Expected BasisPerspective selection for concurrent-client scenarios
- Repair DAG, ILRP recovery, and inverse-plan fixtures
- Sound-session silence assertions

The specification and reference implementation should not be inseparable. Profile-development fixtures and locked acceptance traces must remain separate.

## 115. CI and supply-chain hardening

Recommended checks:

- `cargo fmt`
- Clippy
- Tests on major platforms
- Wasm target builds
- Fuzz smoke tests
- Benchmark regression checks
- Dependency policy and license checks
- Semver checks
- SBOM generation
- Reproducible package smoke tests
- Documentation builds
- Migration compatibility tests

## 116. Performance benchmarks

Benchmark:

- Cold startup
- Warm daemon startup
- Keystroke-to-CST update
- Keystroke-to-HTML patch
- Large-file scrolling and lazy loading
- Relation traversal
- Graph query latency
- Table recalculation
- Sync merge
- Resource loading
- AI context compilation
- Macro expansion
- Memory per logical Node and Relation

Optimization decisions must be benchmark-driven.

---

# Part XXI — Repository and crate architecture

## 117. Monorepo structure

```text
liminal/
├── spec/
│   ├── kernel.md
│   ├── syntax.md
│   ├── ir.md
│   ├── transforms.md
│   ├── protocol.md
│   └── rfc/
├── crates/
│   ├── liminal-id
│   ├── liminal-graph
│   ├── liminal-revision
│   ├── liminal-text
│   ├── liminal-source
│   ├── liminal-cst
│   ├── liminal-hir
│   ├── liminal-cir
│   ├── liminal-query
│   ├── liminal-transform
│   ├── liminal-format
│   ├── liminal-resource
│   ├── liminal-history
│   ├── liminal-resolver
│   ├── liminal-sync
│   ├── liminal-plugin-api
│   ├── liminal-wasm-host
│   ├── liminal-lua
│   ├── liminal-protocol
│   ├── liminal-daemon
│   └── liminal-cli
├── domains/
│   ├── prose
│   ├── org
│   ├── academic
│   ├── code
│   ├── notebook
│   ├── table
│   ├── slide
│   ├── image
│   ├── ink
│   ├── audio
│   ├── lecture
│   └── ai
├── backends/
│   ├── html
│   ├── pandoc
│   ├── terminal
│   ├── web
│   ├── ai
│   └── pack
├── integrations/
│   ├── nvim
│   ├── helix
│   ├── git
│   ├── cargo
│   ├── lsp
│   └── jupyter
├── apps/
│   ├── web
│   ├── mobile
│   └── tablet
├── conformance/
└── benches/
```

Start in one monorepo. Split only when release cadence, ownership, or build cost justifies it.

## 118. Public API shape

Keep raw graph APIs available for infrastructure work, but publish rich domain APIs:

```rust
paragraph.append_text(...)
task.schedule(...)
citation.attach(...)
table.set_formula(...)
lecture.mark_timestamp(...)
```

These APIs compile to Node and Relation operations. Plugin authors should not need to hand-author low-level edges for ordinary work.

---


## 118A. Prior-art commitments

Liminal should mine mature systems by adopting their strongest boundary rather than cloning their entire implementation.

- **Salsa:** adopt the pure tracked-query versus external input-mutation boundary, revisioned dependency tracking, and durability-informed invalidation. Extend the input model with Workspace Basis and freshness rather than performing effects inside queries.
- **Automerge and Peritext:** use as primary experimental references for graph-native collaborative rich text, stable positions, marks, and intent-preserving merge. Do not assume a plain text CRDT automatically solves rich structure.
- **AtJSON:** borrow the separation of raw content from offset annotations and parse-token annotations for external-file or annotated-source domains. Replace raw-offset persistence with revision-aware anchors where durable identity is required.
- **Pandoc:** treat the Pandoc AST as a mature interoperability adapter between Liminal dialects and its reader/writer ecosystem, not as the canonical Liminal graph.
- **Unison:** borrow content-addressed immutable version identity and the separation of names from content identity. Retain a separate logical `EntityId` for mutable human artifacts.
- **Datomic:** borrow immutable database values, transaction basis, as-of/history queries, and the repository-versus-checkout mental model. Extend a scalar basis with an Workspace Basis vector for mixed local and external sources.

These are architectural inputs and conformance targets, not mandatory dependencies. Phase -1 prototypes decide where direct reuse is superior to reimplementation.


---

# Part XXII — Implementation roadmap with architectural gates

This is not a commercial MVP roadmap. It has two orderings:

1. **Information order:** experiments that can falsify the architecture run first.
2. **Dependency order:** implementation then proceeds from kernel to domains and clients.

Known-territory engineering must not postpone architecture-killing questions.

## Phase -1 — Jurisdiction, identity, repair, and projection falsification laboratory

This phase is ordered by information gain rather than implementation dependency. It begins with a deliberately small interpretive checker and does **not** build `CompiledJurisdictionPlan`, indexed policy dispatch, a custom CRDT, or production optimization.

### -1.0 Interpretive Jurisdiction and repair toy

Implement only two profiles:

- External-file
- Graph-native

Construct one toy workspace containing:

- A file-held paragraph edited through a Neovim buffer with first-class `epoch + generation` Basis components
- A second phone/client buffer dirty over the same durable file
- A graph-native comment Relation targeting that paragraph
- One foreign edit that moves, duplicates, or damages the paragraph identity
- One unavailable write route that forces a durable Overlay
- One single-step accepted repair presented as Promotion
- One two-step repair DAG in which source-ID insertion must precede Relation reattachment
- Daemon termination injected after every ILRP durable boundary: intent commit, external apply, acknowledgement, graph finalization, and completion notification

The reference checker must produce expected silent success and boundary diagnostics without rejecting any edit.

Exit gate:

- Sound states produce no output.
- The cross-Jurisdiction repair follows the mutation-local composition rule.
- Promotion uses the same `RepairPlan` interpreter as other repairs.
- A unique textual result is not automatically accepted without the domain safety condition.
- The offline edit is durable as an Overlay and appears in the core `lim overlays` queue without requiring the agenda domain.
- ILRP resumes correctly after termination at every tested boundary and reaches `Committed`, `NeedsReview`, or `Aborted` without hidden half-state, duplication, or lost work.
- The accepted automatic repair is one-command revertible.
- `ClientScoped(neovim)`, `ClientScoped(phone)`, and `DurableOnly` Bases produce three intentional snapshots; no computation observes a chimera of both dirty buffers.
- Buffer generations invalidate only dependent queries.

### -1.1 Persistent identity torture corpus

Test inline IDs, sidecars, immutable content hashes, structural matching, revision anchors, and managed graph IDs under:

- Rename and move
- Split and merge
- Copy and paste
- Duplicate identical blocks
- Delete and recreate
- Formatter rewrites
- Arbitrary external-editor changes
- Git merge, rebase, cherry-pick, and conflict resolution

Output an identity guarantee matrix. No strategy may claim stronger continuity than the corpus demonstrates.

### -1.2 Annotated-source and rich-editing prototypes

Prototype independently:

```text
source bytes ↔ source-preserving annotations/HIR ↔ Resolved Graph IR
```

and:

```text
graph/replica operations ↔ rich editor transaction model
```

Exercise inline marks, links, comments, list restructuring, selections, undo, malformed source, and concurrent edits. The annotated-source profile remains experimental unless anchor recovery under foreign edits passes its declared conformance level.

### -1.3 Revision, Basis selection, and invalidation prototype

Combine:

- Immutable point-in-time graph snapshots and as-of queries
- A pure incremental query engine
- Explicit durability and freshness inputs
- An effect reactor that injects observations as transactions
- A persistent available-input map with per-client buffer generations
- Explicit `ClientScoped(client)`, `DurableOnly`, `Published`, and `Federated` Basis Perspectives
- Dependency tracking on individual selected Basis components rather than the whole available-input map

Test an AI context build, a cross-document query, an interactive preview, and a durable export while two clients have different dirty buffers over one file.

### -1.4 Adapter-boundary prototype

Lower representative Resolved Graph IR into a Pandoc adapter AST and back. Record semantic loss, ID survival, foreign-node preservation, and the exact projection capability level achieved.

### -1.5 Held-out ergonomics and adversarial trace harness

Define the exact operation and session denominators before profile tuning. Build importers or trace generators for:

- Real external Git histories and merge resolutions
- Consented/anonymized editor sessions
- Foreign formatter and rich-tool rewrites
- Offline/online transitions
- Independent adversarial identity and boundary mutations

Split training and locked acceptance corpora. Report per profile:

```text
auto-resolution rate over Jurisdiction-sensitive operations
manual-intervention-free session rate
sound-session diagnostic count
unexpected reconciliation items created per session
unresolved Overlay/repair-intent debt at session end
visible incidents per root cause
```

A profile cannot enter the stable set by passing only self-authored fixtures.

Final Phase -1 gate:

- The architecture can state exactly which Holder governs every toy subject.
- Facets needing independent governance have been reified as Nodes or Relations.
- Anonymous arbitrary text is not falsely promised perfect identity.
- At least one source projection reaches canonical round-trip, and richer lens limits are measured honestly.
- Cross-Holder repair is ordered, crash-resumable, idempotent, and revertible.
- Two independent dirty buffers never enter one Workspace Basis.
- Overlay aging and core CLI/contextual surfacing are demonstrated without the agenda domain.
- Pure queries replay deterministically from frozen Workspace Bases.
- The held-out trace harness has frozen denominators and a locked acceptance split.
- Failure of any gate revises the Contract before production parser, daemon, or compiled Jurisdiction optimization begins.


## Phase 0 — Constitution, Jurisdiction profiles, corpus, and conformance laws

Deliver:

- Kernel specification for Node and Relation
- Jurisdiction Contract interpreter and Workspace Basis specification
- Tiered vocabulary: familiar default UI language plus advanced Jurisdiction terminology
- Operation- and session-denominated profile-coverage conformance tests over held-out traces
- Mutation-local cross-Jurisdiction repair law
- Unified RepairPlan/Promotion semantics with domain safety checks and one-command revert
- ILRP specification and recovery fixtures
- Overlay aging, core reconciliation-queue surfacing, and later agenda projection rules
- Explicit requester-scoped, durable-only, published, and federated Basis Perspectives
- Identity grades and dual entity/version identity model
- Projection capability levels and lens laws
- Transform contract model
- Initial source-language principles
- Conformance fixture format
- Benchmark and torture corpus
- ADR process
- Threat model
- Prior-art adoption decisions

Exit gate:

- The same small kernel can represent prose, a code block, a table, an image reference, an audio interval, and a synchronized external value without adding a new primitive.
- Every example resolves through a standard profile or an explicitly marked boundary Contract and has a declared identity guarantee.
- Each stable profile automatically resolves at least 99% of held-out Jurisdiction-sensitive operations and leaves at least 99% of ordinary sessions free of manual Jurisdiction interaction.
- Every sound session produces exactly zero user-facing Jurisdiction diagnostics, zero unexpected reconciliation items, and zero requests for user-authored Contracts.

## Phase 1 — External-file Jurisdiction source-to-HTML vertical slice

Deliver:

- Rope-backed Holder-controlled source
- Error-tolerant CST
- AtJSON-inspired annotation experiment or equivalent source-map layer
- Human IR
- Derived resolved graph snapshot with source basis
- Compact Markdown-compatible syntax
- Explicit syntax
- `lim fmt`, `lim check`, `lim expand`
- Native HTML backend
- Full and incremental compile equivalence tests
- Basic CLI

Exit gate:

- Editing a paragraph incrementally updates only its semantic and HTML dependencies.
- Canonical formatting is deterministic and idempotent.
- The graph is explicitly treated as derived for this profile, with no dual-Jurisdiction ambiguity.
- The interpretive Jurisdiction Checker and RepairPlan interpreter remain the reference semantics; optimized plans are still deferred.

## Phase 2 — Persistent daemon and Neovim integration

Deliver:

- `liminald`
- Liminal Document Protocol
- Buffer-version, file-hash, and Git-basis tracking
- File watching
- Revisioned pure query engine
- Diagnostics
- Incremental HTML preview
- Neovim Lua client
- Semantic node inspection
- Identity-grade reporting and repair diagnostics
- Basic references and backlinks

Exit gate:

- A normal Markdown-compatible file can be edited in Neovim while live preview and semantic indexes remain synchronized to the exact buffer or file basis.
- Saving, external file modification, and Git checkout produce explicit Promotions or Holder changes.

## Phase 3 — Transform and macro infrastructure

Deliver:

- Graph rewrite IR
- Declarative macros
- Lua host
- Basic semantic command API
- Pseudo-stenographic macro packs for Rust and prose
- Macro expansion tracing
- Structural edit operations
- Pass fusion prototype

Exit gate:

- The same semantic macro can be invoked from Neovim, CLI, and a test harness without depending on one keymap.
- In external-file Jurisdiction domains, accepted semantic edits round-trip through source and reparse to the intended result.

## Phase 4 — Workspace and build graph

Deliver:

- `Liminal.toml` and lockfile
- Multi-document workspace graph
- Build dependency graph
- Cargo adapter
- LSP virtual-document adapter
- Pandoc AST adapter
- Content-addressed resources
- Search and relation indexes
- `lim build`, `lim serve`, `lim query`

Exit gate:

- A workspace can contain Rust code, documentation, images, and an academic note, compile to HTML, and link source symbols to prose Nodes while preserving each domain's Jurisdiction Contract.

## Phase 5 — Core document domains and graph-native pilot

Deliver:

- Prose and outline domain
- Org/task and agenda domain
- Academic citations and equations
- Table/formula domain
- Notebook execution metadata
- Book/long-form structures
- One graph-native document profile
- Conversion-loss diagnostics

Exit gate:

- One workspace supports notes, tasks, a paper, code, and computed tables without creating undeclared competing Holders.
- The graph-native pilot satisfies its declared projection capability level.

## Phase 6 — History and local-first synchronization

Deliver:

- Datomic-inspired immutable graph revisions and as-of/history queries
- Semantic transaction log
- Snapshots and compaction
- Undo/redo and revision browsing
- Git snapshot integration
- Replica identity
- Automerge/Peritext evaluation and adapter where appropriate
- Encrypted resource synchronization
- Conflict diagnostics
- Backup and recovery tooling

Exit gate:

- Two offline replicas can edit the Jurisdiction profiles explicitly supported by the chosen merge model, reconnect, converge, and retain auditable history.
- Unsupported cross-profile merges fail visibly rather than pretending to be safe.

## Phase 7 — Intelligent external Relations

Deliver:

- Effect reactor and resolver runtime
- Capability model
- Pointer, snapshot, cache, mirror, live, materialized, replicated, and derived policies
- Freshness and availability state
- Workspace Basis recording
- Freeze/reproducibility command
- Database and HTTP example resolvers
- Stock-price and reading-progress demonstration Relations

Exit gate:

- A document remains useful offline, exposes staleness honestly, and synchronizes external state according to declared policy without hidden effects.
- Frozen resolver inputs replay deterministically through the pure query engine.

## Phase 8 — Multimodal resource and lecture stack

Deliver:

- Image and region model
- PDF annotation relations
- Audio recording and chunking
- Transcript Nodes and timeline relations
- Lecture mode
- Raw ink store
- SVG preview
- Page and canvas models
- Voice commands and dictation adapter

Exit gate:

- A lecture session combines typed notes, handwriting, audio, transcript, slides, and timestamps in one graph and reconstructs the session offline.
- Raw-media Jurisdiction and derived recognition/transcript Jurisdiction remain distinct.

## Phase 9 — Rich web, phone, and tablet clients

Deliver:

- Core compiled to Wasm where practical
- Worker-based compiler
- Rich graph transaction adapter
- Incremental DOM patches
- Capture-first phone client
- Pen-first tablet client
- Partial graph and resource synchronization

Exit gate:

- Supported graph-native documents can be edited through rich web, phone, and tablet clients without semantic divergence.
- External-file Jurisdiction documents expose only the projection capability level proven in Phase -1 rather than promising universal bidirectionality.

## Phase 10 — AI compiler and semantic operation loop

Deliver:

- AI MIR
- Model profiles
- Context planner
- Markdown, tagged, JSON, and graph projections
- Multimodal bundle support
- Semantic operation output
- Provenance and approval workflow
- Evaluation harness
- Optional local model and remote provider adapters

Exit gate:

- AI can receive a task-specific subgraph with exact Workspace Basis and return revision-safe graph or source operations whose result is validated under the target domain's Jurisdiction Contract.

## Phase 11 — Stable plugin ecosystem

Deliver:

- Versioned Wasm component API
- Stable Lua semantic API
- Trusted native integration guide
- Process plugin protocol
- Package resolution and lockfile
- Conformance certification
- Migration framework
- Documentation and examples

Exit gate:

- A third party can add a dialect, resolver, renderer, Jurisdiction profile, or transformation without depending on internal Rust layouts.

## Phase 12 — Advanced spatial, spreadsheet, and presentation runtimes

Deliver as independent specialized subsystems:

- Large-scale table engine
- Constraint layout
- Slide animation timeline
- Diagram routing
- Advanced handwriting recognition
- Data visualizations
- Optional Cranelift acceleration

Exit gate:

- Performance-critical domains prove that semantic uniformity can coexist with radically specialized execution and explicit Jurisdiction.

---

# Part XXIII — Required ADRs and open research questions

## 119. Source grammar

Decide between:

- Strict CommonMark-compatible superset
- Separate native `.lim` grammar
- Dual frontend with shared explicit core

Recommendation: begin with a constrained Markdown-compatible frontend and a fully explicit native debug form. Let real requirements determine whether a distinct native syntax earns its cost.

## 120. Persistent identity and external round-trips

This is a Phase -1 architecture gate, not a deferred serialization choice.

Fixed constraints:

- Exact immutable versions may be content-addressed.
- Logical continuity requires a separate entity identity.
- Arbitrary external editing cannot perfectly preserve invisible logical identity.
- Durable Relations must declare a minimum identity grade.
- Inline IDs, graph-managed IDs, external IDs, sidecars, and heuristics are different guarantees and must never be conflated.

The remaining decision is which identity grades each Jurisdiction profile requires by default, informed by the torture corpus.

## 121. Graph serialization

Evaluate:

- CBOR-like initial snapshots
- Custom packed binary
- Memory-mappable layout
- Stable debug JSON

Do not design a final binary format before the in-memory model and migration story are proven.

## 122. Incremental and revision engine

Evaluate direct use of Salsa versus a Liminal-specific engine while preserving these boundaries:

- Immutable syntax and graph values
- Deterministic tracked queries over explicit Jurisdiction inputs
- Mutation and effects outside tracked queries
- Durability-informed invalidation
- Workspace Basis vectors
- Demand-driven computation
- Interned symbols
- Diagnostic accumulation
- Datomic-like as-of/history views

## 123. Collaboration

Evaluate Automerge rich text, Peritext, and other operation models before writing a custom CRDT. Rich text, block structure, tables, canvas objects, and ordinary source files may require different merge types. The semantic graph should not be forced to equal one vendor's CRDT layout, and a domain must declare whether it is replicated, file-merged, or single-writer.

## 124. Rich editor mapping and projection lenses

This is a Phase -1 architecture gate.

Define and test:

- `parse(emit(graph))` laws for supported graph subsets
- `emit(parse(source))` canonicalization laws
- Incremental text-edit versus graph-transaction equivalence
- Selection and anchor survival
- Inline mark boundary behavior
- Block restructuring
- Undo and concurrent merge
- Explicit projection capability levels

AtJSON-style annotations, Automerge/Peritext rich text, and a ProseMirror-class transaction model are primary prototype references. No universal bidirectional promise is permitted without conformance evidence.

## 125. Jurisdiction and repair optimization boundary

Do not compile Contracts, repair DAGs, Basis selection, or ILRP execution into indexed dispatch or generated policy code until Phase -1 and Phase 0 freeze their observable semantics. The Jurisdiction and RepairPlan interpreters are the conformance oracles. Optimization may begin only after differential tests prove the compiled path equivalent, including injected-crash recovery and concurrent-buffer Basis cases.

## 126. Relation policy language

Decide how declarative relation policies are authored, validated, and compiled without turning every graph traversal into dynamic policy interpretation.

## 127. Schema and dialect ABI

Determine what is stable:

- Names and versions
- Node/Relation contracts
- Transformation ABI
- Renderer interfaces
- Migration rules

## 128. Layout delegation

Decide which targets use native layout and which delegate to browsers, Typst, LaTeX, or other engines.

## 129. AI projection metrics

Define benchmark tasks before inventing tokenizer-optimized encodings. Markdown may remain best for many models even when a more compact encoding exists.

## 130. History retention

Perfect recall is desirable, but operation retention, compaction, encryption, and privacy policies must be explicit.

## 131. Licensing and governance

Recommended starting point:

- Rust-style permissive code licensing
- Public specification
- RFC and ADR process
- Conformance suite
- No mandatory central registry

The exact license remains a project decision.

---

# Part XXIV — Success criteria

## 132. Architectural success

Liminal succeeds architecturally when:

- No new modality forces a third semantic primitive.
- Domain APIs remain pleasant despite the minimal kernel.
- Common paths do not pay generic graph overhead.
- The graph can be serialized, reconstructed, migrated, and inspected.
- Every independently governable Node or Relation resolves through a Jurisdiction profile or boundary Contract at an exact Workspace Basis.
- Identity guarantees are declared rather than inferred silently.
- Personal projections preserve one meaning at their declared capability level.
- Effects remain explicit and auditable.
- Lossy conversion is visible.

## 133. Personal-use success

Liminal succeeds for its creator when it can become the default substrate for:

- Coding notes in Neovim
- Project documentation
- Academic papers and research
- Long-form fiction
- Lecture recording and review
- Handwritten equations and diagrams
- Tasks and planning
- Tables and experiments
- AI-assisted retrieval and transformation
- Access from desktop, phone, and tablet

## 134. Infrastructure success

Even without broad adoption, the project succeeds if it produces reusable components such as:

- A stable document graph IR
- An incremental Markdown-compatible compiler
- A semantic formatter
- A portable macro engine
- A relation-resolution runtime
- A multimodal timeline model
- An AI context compiler
- A Neovim document protocol
- A capability-safe Wasm plugin interface

## 135. Ultimate terminal state

The ultimate Liminal workspace behaves like this:

1. Brian opens Neovim and writes compact Markdown-like source.
2. `liminald` records the editor-buffer basis and incrementally updates the domain's graph materialization.
3. Rust code bytes remain governed by the editor/file/toolchain Jurisdiction profile, while rust-analyzer and Cargo retain language responsibility.
4. HTML preview updates immediately.
5. A pen tablet adds diagrams and equations linked to text Nodes.
6. A lecture recording creates audio, transcript, typed notes, and ink on one timeline.
7. Tasks appear in an Org-like agenda generated from graph queries.
8. A remote book-progress Relation works offline and synchronizes later.
9. A stock-price Node shows the last known observation with visible freshness.
10. A paper compiles through native HTML and Pandoc/Typst adapters.
11. The phone captures voice, text, photos, and tasks into the same workspace.
12. The AI compiler selects the relevant subgraph and emits a model-optimized context package.
13. AI returns semantic operations with provenance rather than blindly rewriting files.
14. Git records milestones for external-file Jurisdiction domains, while graph-native and replicated domains use their declared transaction or CRDT histories.
15. Personal config fuses the common Rust, prose, academic, HTML, and macro paths into a small optimized binary.
16. Every advanced abstraction still lowers to Nodes and Relations.

---

# Final definition

Liminal is a **universal open information compiler and runtime** built around an attributed, ordered, versioned graph.

Its semantic floor is:

```text
Node
Relation
```

Its computational model is:

```text
Graph state
+ programmable graph rewrites
+ explicit effect resolvers
```

Its performance model is:

```text
minimal logical ontology
+ domain-specific physical stores
+ incremental queries
+ lazy dialect loading
+ fused optimized paths
```

Its user model is:

```text
write compactly
inspect explicitly
edit through any suitable frontend
compile to any declared target
retain local ownership
```

Its infrastructure principle is:

> Keep the center almost absurdly small. Permit the edges to become as specialized, imperative, optimized, multimodal, and powerful as reality requires.


---

## Revision 3 summary

Revision 3 replaced the former formal term with Jurisdiction; admitted Jurisdiction as the deliberate complexity sink; reduced facets to schema addresses over Nodes and Relations; established a small diagnostic vocabulary and silent healthy behavior; defined mutation-local cross-Jurisdiction repair; gave Overlays an explicit lifecycle; marked annotated-source and federated profiles as delegated hard subsystems rather than solved declarations; moved compiled Jurisdiction plans after falsification; and made per-buffer editor generations first-class Workspace Basis components.

## Revision 4 summary

Revision 4 makes ergonomics measurable over held-out operations and complete sessions; prevents hidden Overlay debt from counting as success; makes ordinary UI domain-native rather than Jurisdiction-heavy; unifies Promotion with the repair engine; separates deterministic results from safe automatic application; replaces unordered repair mutations with dependency DAGs; adds reversible decision records; defines the crash-resumable Intent-Logged Repair Protocol for graph-plus-filesystem changes; introduces explicit requester-scoped and durable `BasisPerspective` selection so concurrent dirty buffers never form a chimera; moves Overlay surfacing into a Phase -1 core Reconciliation Queue with agenda as a later projection; and expands the falsification toy to include daemon termination at every repair boundary.