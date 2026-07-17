# Corpora

Two corpus families live here, with opposite mutation rules. The
governing law is v4 §7.4 / R4 §2.2; the held-out lock is spelled out in
`heldout/POLICY.md` and that document is binding.

## Layout

```text
corpora/
├── README.md                  # this file
├── heldout/
│   ├── POLICY.md              # the lock law — read before touching anything below
│   └── v1/                    # populated at M11 (the -1.5 trace harness)
│       ├── MANIFEST.b3    # BLAKE3 of every file below; CI-enforced
│       ├── git/               # real Git histories as *.bundle
│       │   └── *.bundle       #   merges, rebases, moves, renames, conflict resolutions
│       ├── editor/            # consented + anonymized editor sessions
│       │   └── *.trace.ndjson
│       ├── formatter/         # foreign formatter and rich-tool rewrites
│       │   └── *.trace.ndjson
│       ├── offline/           # offline/online transitions, resolver observations
│       │   └── *.trace.ndjson
│       └── adversarial/      # independently generated identity/boundary mutations
│           └── *.trace.ndjson
└── regression/                # append-only: every discovered failure, minimized
```

## `heldout/` — locked acceptance corpora

Versioned as `heldout/v<N>/`. Each version carries a `MANIFEST.b3` (BLAKE3)
covering every byte of the corpus; the day-one CI test
`tests/classes/trace_replay.rs::heldout_manifest_locked` recomputes the
hashes and fails on any drift. Once profile scores against `v<N>` have
been seen, `v<N>` is frozen forever — corrections and additions create
`v<N+1>`, and old scores are retained (v4 §7.4). `v1` is populated at
M11, when the -1.5 harness defines the NDJSON trace format and the
frozen operation/session denominators.

Traces carry operation and session labels (v4 §114) so the R4 §2.3
scorecard — auto-resolution rate, intervention-free session rate,
sound-session diagnostics, unexpected reconciliation items, incidents
per root cause, manual Contract authoring — computes from the corpus
without interpretation slack.

## `regression/` — the failure archive

Every falsified assumption, fuzz find, crash, and field failure lands
here minimized, forever. See `regression/README.md`.
