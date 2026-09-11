//! Structural registry checks for the candidate formal-safety package.
//!
//! This candidate-only evidence contract traces the ILRP protocol in v4 §7.8,
//! while its source-handling boundary preserves the v4 §7.4 corpus restrictions.
//! Structural success is neither proof nor qualification.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8Path;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const OBLIGATIONS_FILE: &str = "verification/obligations.json";
const ASSUMPTIONS_FILE: &str = "verification/assumptions.json";
const SAS_REVISION: &str = "0.1.0-proposed.1";
const SAS_SHA256: &str = "53eb3ebf1616ae7017e2ed22e39acee9823c56f16b3c527f0035d769b582ec73";

const ALL_ASSUMPTIONS: &[&str] = &[
    "trusted-host",
    "filesystem-durability",
    "compiler-verifier",
    "cryptographic-integrity",
    "finite-model",
    "conditional-progress",
    "linux-only",
    "non-hostile-qualifier",
];

const BASE_ASSUMPTIONS: &[&str] = &["compiler-verifier", "non-hostile-qualifier"];
const ALL_ASSUMPTIONS_WITH_BASE: &[&str] = ALL_ASSUMPTIONS;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObligationRegistry {
    schema_version: u32,
    adoption: Adoption,
    obligations: Vec<Obligation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Adoption {
    state: String,
    accepted_sas_revision: String,
    accepted_sas_sha256: String,
    metadata_semantics: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Obligation {
    id: String,
    requirement_anchors: Vec<String>,
    applicability: String,
    assumption_ids: Vec<String>,
    status: String,
    source_anchors: Vec<SourceAnchor>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssumptionRegistry {
    schema_version: u32,
    anchor_semantics: String,
    assumptions: Vec<Assumption>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Assumption {
    id: String,
    rationale: String,
    owner_role: String,
    scope: String,
    source_anchor: SourceAnchor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct SourceAnchor {
    path: String,
    start_line: usize,
    end_line: usize,
    sha256: String,
}

#[derive(Clone, Copy)]
struct ExpectedAnchor {
    path: &'static str,
    start_line: usize,
    end_line: usize,
    sha256: &'static str,
    required_text: &'static str,
}

#[derive(Clone, Copy)]
struct ExpectedObligation {
    id: &'static str,
    requirement_anchors: &'static [&'static str],
    applicability: &'static str,
    extra_assumptions: &'static [&'static str],
    anchors: &'static [ExpectedAnchor],
}

const SPEC: &str = "spec/v4/liminal_master_architecture_plan_v4.md";
const ADR_0007: &str = "docs/adr/0007-hand-rolled-phase-minus-1-toy-store.md";
const ADR_0020: &str = "docs/adr/0020-require-high-assurance-phase-1-suite-qualification.md";
const ADR_0021: &str = "docs/adr/0021-stage-haqp-1-qualification-around-phase-1-authorization.md";

macro_rules! anchor {
    ($path:expr, $start:expr, $end:expr, $sha:expr, $text:expr) => {
        ExpectedAnchor {
            path: $path,
            start_line: $start,
            end_line: $end,
            sha256: $sha,
            required_text: $text,
        }
    };
}

const EXPECTED_OBLIGATIONS: &[ExpectedObligation] = &[
    ExpectedObligation {
        id: "identity-only",
        requirement_anchors: &["LIM-SAS-RQ-104", "Law 1"],
        applicability: "current-core",
        extra_assumptions: &[],
        anchors: &[anchor!(
            SPEC,
            85,
            87,
            "d4cafcbb1ab2d716001a02c92db347131f6e3412afe7f234ec7f753b582d1ce2",
            "Only Nodes and Relations are semantically fundamental"
        )],
    },
    ExpectedObligation {
        id: "authority",
        requirement_anchors: &[
            "LIM-SAS-RQ-107",
            "LIM-SAS-RQ-108",
            "LIM-SAS-RQ-110",
            "LIM-SAS-RQ-113",
            "LIM-SAS-RQ-114",
            "Law 3",
            "Law 3A",
            "Law 3C",
            "Law 3F",
            "Law 3G",
        ],
        applicability: "current-core",
        extra_assumptions: &["trusted-host"],
        anchors: &[anchor!(
            SPEC,
            97,
            131,
            "a9bcbca5b05b56347dc810e445dde08162a08beb4a13b20b60c43e6a8bb6d7ec",
            "Automatic repair is allowed only when every affected Jurisdiction authorizes its own mutation"
        )],
    },
    ExpectedObligation {
        id: "capture",
        requirement_anchors: &["LIM-SAS-RQ-109", "LIM-SAS-RQ-116", "Law 3B", "Law 3I"],
        applicability: "current-core",
        extra_assumptions: &["trusted-host", "filesystem-durability", "linux-only"],
        anchors: &[
            anchor!(
                SPEC,
                107,
                109,
                "a59112974bd8ea095a099231c374285039873ee3324038d747bafbd06648c83b",
                "Capture is never rejected"
            ),
            anchor!(
                SPEC,
                137,
                139,
                "bd486e000169968d2ecf3498bbe69b97827a3d7259742195e18ba76ed7c091c2",
                "Overlay debt remains visible until resolved"
            ),
        ],
    },
    ExpectedObligation {
        id: "basis",
        requirement_anchors: &["LIM-SAS-RQ-111", "LIM-SAS-RQ-117", "Law 3D", "Law 3J"],
        applicability: "current-core",
        extra_assumptions: &["cryptographic-integrity"],
        anchors: &[
            anchor!(
                SPEC,
                115,
                117,
                "a8d16455e2ddb7d7d46575d05005087e75c4d95077c7e8972919dbe81c910649",
                "One immutable Workspace Basis per computation"
            ),
            anchor!(
                SPEC,
                141,
                143,
                "a9add201768a663cff5ad78080bbfde627a1b6bf1ce0fa8eff83232edad60ef6",
                "never form a chimeric Basis"
            ),
        ],
    },
    ExpectedObligation {
        id: "ilrp-order",
        requirement_anchors: &["LIM-SAS-RQ-115", "Law 3H", "v4 7.7", "v4 7.8"],
        applicability: "current-core",
        extra_assumptions: &[],
        anchors: &[
            anchor!(
                SPEC,
                133,
                135,
                "ff23185fee6445ff702ea8b34146c90104f848500bde6ef61b694a11a0ed8408",
                "dependency DAG"
            ),
            anchor!(
                SPEC,
                695,
                718,
                "f9df59550699a9442a7cebe84149d2ebbc910d251c8f7318da6618c5ed2fb683",
                "ordered dependency graph"
            ),
        ],
    },
    ExpectedObligation {
        id: "ilrp-intent",
        requirement_anchors: &["LIM-SAS-RQ-115", "v4 7.8 Prepare"],
        applicability: "current-core",
        extra_assumptions: &["trusted-host", "filesystem-durability", "linux-only"],
        anchors: &[anchor!(
            SPEC,
            766,
            766,
            "ca085fa28859ffd044cc495b2f3749b355d6147a642db60ac3afcd7e1ee9242b",
            "Prepare."
        )],
    },
    ExpectedObligation {
        id: "ilrp-apply",
        requirement_anchors: &["LIM-SAS-RQ-115", "v4 7.8 Apply"],
        applicability: "current-core",
        extra_assumptions: &[
            "trusted-host",
            "filesystem-durability",
            "cryptographic-integrity",
            "linux-only",
        ],
        anchors: &[anchor!(
            SPEC,
            767,
            767,
            "ffb294356231cb39b032ac37eb1f5d9da7c2a466733fe1c729fddd15d7ed4a41",
            "Apply."
        )],
    },
    ExpectedObligation {
        id: "ilrp-ack",
        requirement_anchors: &["LIM-SAS-RQ-115", "v4 7.8 Acknowledge"],
        applicability: "current-core",
        extra_assumptions: &["trusted-host", "cryptographic-integrity"],
        anchors: &[anchor!(
            SPEC,
            768,
            768,
            "3f97ccadcb8e4cfff5791ddcae68db4cdf2932e4052f9efa08aad917c48126c8",
            "Acknowledge."
        )],
    },
    ExpectedObligation {
        id: "ilrp-finalize",
        requirement_anchors: &["LIM-SAS-RQ-115", "v4 7.8 Finalize"],
        applicability: "current-core",
        extra_assumptions: &["trusted-host", "filesystem-durability", "linux-only"],
        anchors: &[anchor!(
            SPEC,
            769,
            769,
            "e8cb8f13e6cae7818d4607abc49a2834ed23f19c6d9fa032480cf63ab3900885",
            "Finalize."
        )],
    },
    ExpectedObligation {
        id: "ilrp-recover",
        requirement_anchors: &["LIM-SAS-RQ-115", "v4 7.8 Recover"],
        applicability: "current-core",
        extra_assumptions: &[
            "trusted-host",
            "filesystem-durability",
            "conditional-progress",
            "finite-model",
            "linux-only",
        ],
        anchors: &[anchor!(
            SPEC,
            770,
            770,
            "cf82c227918e3dc826cfac4e96b8653719c85ecad5a56ba647cef1f2be1cc704",
            "Recover."
        )],
    },
    ExpectedObligation {
        id: "ilrp-revert",
        requirement_anchors: &["LIM-SAS-RQ-115", "v4 7.8 Revert"],
        applicability: "current-core",
        extra_assumptions: ALL_ASSUMPTIONS_WITH_BASE,
        anchors: &[anchor!(
            SPEC,
            771,
            771,
            "379c9dd7696fe7376b4c0479033df6f171fa2f89f4e000d6d093f3c1585f8882",
            "Revert."
        )],
    },
    ExpectedObligation {
        id: "store",
        requirement_anchors: &["v4 92", "ADR-0007", "TM-01", "TM-02"],
        applicability: "current-core",
        extra_assumptions: &[
            "trusted-host",
            "filesystem-durability",
            "cryptographic-integrity",
            "linux-only",
        ],
        anchors: &[
            anchor!(
                SPEC,
                2681,
                2696,
                "4d976b083da2d0ea550dab83f3d8a7ef797e42b2fb73cca7c4d76c5f12982dd1",
                "Crash consistency"
            ),
            anchor!(
                ADR_0007,
                28,
                40,
                "56a8e372a55c912750aa8ce72be931ed1b578e23b73a3d663daeb37028d094d1",
                "checksummed append-only NDJSON log"
            ),
        ],
    },
    ExpectedObligation {
        id: "receipt",
        requirement_anchors: &["v4 7.8", "TM-02", "TM-10"],
        applicability: "current-core",
        extra_assumptions: &["trusted-host", "filesystem-durability", "linux-only"],
        anchors: &[
            anchor!(
                SPEC,
                2683,
                2692,
                "38f73aeb4e12bce35f716ca514e0135f231a339f8b046586bab7da5f79cb45a1",
                "atomic replacement"
            ),
            anchor!(
                ADR_0007,
                30,
                35,
                "15e908db325db4e78d7abcd5d46bdf4d98d2ba845110eedc492a283f9f062023",
                "returning only after fsync"
            ),
        ],
    },
    ExpectedObligation {
        id: "evidence",
        requirement_anchors: &[
            "LIM-SAS-RQ-003",
            "LIM-SAS-RQ-005",
            "LIM-SAS-RQ-012",
            "LIM-SAS-RQ-017",
            "ADR-0020",
            "ADR-0021",
        ],
        applicability: "current-core",
        extra_assumptions: &["cryptographic-integrity"],
        anchors: &[
            anchor!(
                ADR_0020,
                23,
                40,
                "21d8cd3b6fa47ac965f4c43b135075dab47e73f31a34353ea810de53737b1d29",
                "evidence fails closed"
            ),
            anchor!(
                ADR_0020,
                52,
                64,
                "e62cc92795a922a957ceae8ebd805fefaee1cf0f864730dff54efa0c803bfea3",
                "Machine verification rejects duplicates"
            ),
            anchor!(
                ADR_0021,
                44,
                62,
                "3c821e3257bfe11f62a29f8f9a7c6671249612bce8e4d3f68dbe9c90e6192891",
                "ratify the Phase 1 suite"
            ),
        ],
    },
    ExpectedObligation {
        id: "phase1-extension",
        requirement_anchors: &[
            "LIM-SAS-RQ-006",
            "LIM-SAS-RQ-007",
            "LIM-SAS-RQ-008",
            "LIM-SAS-RQ-009",
            "LIM-SAS-RQ-010",
            "LIM-SAS-RQ-011",
            "LIM-SAS-RQ-012",
        ],
        applicability: "phase-1",
        extra_assumptions: ALL_ASSUMPTIONS_WITH_BASE,
        anchors: &[anchor!(
            SPEC,
            3379,
            3400,
            "3476430f027de17938b67097a186231b6b6d6205b0811e079fae8b3359b128e1",
            "Phase 1 — External-file Jurisdiction source-to-HTML vertical slice"
        )],
    },
    ExpectedObligation {
        id: "program-extension",
        requirement_anchors: &["LIM-SAS-RQ-018"],
        applicability: "future-phases",
        extra_assumptions: ALL_ASSUMPTIONS_WITH_BASE,
        anchors: &[anchor!(
            SPEC,
            3402,
            3421,
            "a4aef02b38a0f13c67f087e78f24f9bf50217108d5e4bb6e5665c9d9c152fee0",
            "Phase 2 — Persistent daemon and Neovim integration"
        )],
    },
];

const EXPECTED_ASSUMPTION_ANCHORS: &[(&str, ExpectedAnchor)] = &[
    (
        "trusted-host",
        anchor!(
            ADR_0007,
            30,
            35,
            "15e908db325db4e78d7abcd5d46bdf4d98d2ba845110eedc492a283f9f062023",
            "fsync"
        ),
    ),
    (
        "filesystem-durability",
        anchor!(
            SPEC,
            2681,
            2696,
            "4d976b083da2d0ea550dab83f3d8a7ef797e42b2fb73cca7c4d76c5f12982dd1",
            "Recover after interrupted writes"
        ),
    ),
    (
        "compiler-verifier",
        anchor!(
            SPEC,
            1942,
            1956,
            "7ab56de719b1445527b5ef8f702540c25ce922d2588f44eb2e21e93676897beb",
            "Tool sovereignty"
        ),
    ),
    (
        "cryptographic-integrity",
        anchor!(
            SPEC,
            2683,
            2692,
            "38f73aeb4e12bce35f716ca514e0135f231a339f8b046586bab7da5f79cb45a1",
            "Verify content-addressed objects"
        ),
    ),
    (
        "finite-model",
        anchor!(
            SPEC,
            764,
            770,
            "c8c713e06cad104503a3a7693323dff20cc04bf79228ab787479aca096d2099e",
            "Protocol:"
        ),
    ),
    (
        "conditional-progress",
        anchor!(
            SPEC,
            770,
            770,
            "cf82c227918e3dc826cfac4e96b8653719c85ecad5a56ba647cef1f2be1cc704",
            "resume"
        ),
    ),
    (
        "linux-only",
        anchor!(
            ADR_0007,
            30,
            35,
            "15e908db325db4e78d7abcd5d46bdf4d98d2ba845110eedc492a283f9f062023",
            "fsync(dir)"
        ),
    ),
    (
        "non-hostile-qualifier",
        anchor!(
            ADR_0020,
            34,
            40,
            "cb695767cabb219857701fbbf14d01daf0bb31ad4e623fe2cb444e29fee8d937",
            "fixed clean commit and source tree"
        ),
    ),
];

/// Validate the candidate v4 §7.8 registries against the fixed inventory.
///
/// Source traversal and corpus restrictions retain the boundary declared by
/// v4 §7.4. A successful result is structural only, never qualification.
pub fn verify_registry_repo(root: &Utf8Path) -> Result<()> {
    let obligations: ObligationRegistry = read_json(root, OBLIGATIONS_FILE)?;
    let assumptions: AssumptionRegistry = read_json(root, ASSUMPTIONS_FILE)?;
    ensure!(
        obligations.schema_version == 1,
        "unsupported obligation registry schema"
    );
    ensure!(
        assumptions.schema_version == 1,
        "unsupported assumption registry schema"
    );
    ensure!(
        assumptions.anchor_semantics == "affected-existing-contract",
        "assumption anchors must be declared affected-existing-contract coordinates, not assumption approval"
    );
    verify_adoption(&obligations.adoption)?;
    verify_assumptions(root, &assumptions.assumptions)?;
    verify_obligations(root, &obligations.obligations)?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(root: &Utf8Path, relative: &str) -> Result<T> {
    validate_safe_repo_file(root, relative)?;
    let bytes = std::fs::read(root.join(relative)).with_context(|| format!("read {relative}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {relative}"))
}

fn verify_adoption(adoption: &Adoption) -> Result<()> {
    ensure!(
        adoption.state == "pending",
        "formal registry adoption must remain pending"
    );
    ensure!(
        adoption.accepted_sas_revision == SAS_REVISION,
        "accepted SAS revision metadata drift"
    );
    ensure!(
        adoption.accepted_sas_sha256 == SAS_SHA256,
        "accepted SAS digest metadata drift"
    );
    ensure!(
        adoption.metadata_semantics == "historical-reference-only",
        "SAS metadata must be declared historical-reference-only"
    );
    Ok(())
}

fn verify_assumptions(root: &Utf8Path, assumptions: &[Assumption]) -> Result<()> {
    let mut actual = BTreeMap::new();
    for assumption in assumptions {
        ensure!(!assumption.id.is_empty(), "assumption ID must be nonempty");
        ensure!(
            actual.insert(assumption.id.as_str(), assumption).is_none(),
            "duplicate assumption ID: {}",
            assumption.id
        );
    }
    let expected: BTreeSet<_> = ALL_ASSUMPTIONS.iter().copied().collect();
    ensure!(
        actual.keys().copied().collect::<BTreeSet<_>>() == expected,
        "assumption inventory must contain exactly the closed eight IDs"
    );
    for (id, expected_anchor) in EXPECTED_ASSUMPTION_ANCHORS {
        let assumption = actual[id];
        let (expected_rationale, expected_scope) = expected_assumption_text(id);
        ensure!(
            assumption.rationale == expected_rationale,
            "assumption {id} rationale must match the fixed candidate mapping"
        );
        ensure!(
            assumption.owner_role == "human-T1",
            "assumption {id} owner_role must be human-T1"
        );
        ensure!(
            assumption.scope == expected_scope,
            "assumption {id} scope must match the fixed candidate mapping"
        );
        verify_exact_anchor(root, &assumption.source_anchor, *expected_anchor, id)?;
    }
    Ok(())
}

fn verify_obligations(root: &Utf8Path, obligations: &[Obligation]) -> Result<()> {
    let mut actual = BTreeMap::new();
    for obligation in obligations {
        ensure!(!obligation.id.is_empty(), "obligation ID must be nonempty");
        ensure!(
            actual.insert(obligation.id.as_str(), obligation).is_none(),
            "duplicate obligation ID: {}",
            obligation.id
        );
    }
    let expected: BTreeSet<_> = EXPECTED_OBLIGATIONS.iter().map(|row| row.id).collect();
    ensure!(
        actual.keys().copied().collect::<BTreeSet<_>>() == expected,
        "obligation inventory must contain exactly the closed sixteen IDs"
    );

    for expected_row in EXPECTED_OBLIGATIONS {
        let row = actual[expected_row.id];
        ensure!(
            row.status == "not-established",
            "obligation {} status must be not-established",
            row.id
        );
        ensure!(
            row.applicability == expected_row.applicability,
            "obligation {} has wrong applicability",
            row.id
        );
        ensure!(
            string_set(&row.requirement_anchors, "requirement anchor", &row.id)?
                == expected_row.requirement_anchors.iter().copied().collect(),
            "obligation {} has wrong requirement anchors",
            row.id
        );

        let mut expected_assumptions: BTreeSet<_> = BASE_ASSUMPTIONS.iter().copied().collect();
        expected_assumptions.extend(expected_row.extra_assumptions.iter().copied());
        ensure!(
            string_set(&row.assumption_ids, "assumption ID", &row.id)? == expected_assumptions,
            "obligation {} has wrong assumption mapping",
            row.id
        );
        ensure!(
            row.source_anchors.len() == expected_row.anchors.len(),
            "obligation {} has missing or extra source anchors",
            row.id
        );
        for (actual_anchor, expected_anchor) in row.source_anchors.iter().zip(expected_row.anchors)
        {
            verify_exact_anchor(root, actual_anchor, *expected_anchor, &row.id)?;
        }
    }
    Ok(())
}

fn string_set<'a>(values: &'a [String], kind: &str, owner: &str) -> Result<BTreeSet<&'a str>> {
    ensure!(!values.is_empty(), "{owner} {kind} list must be nonempty");
    let set: BTreeSet<_> = values.iter().map(String::as_str).collect();
    ensure!(set.len() == values.len(), "{owner} has duplicate {kind}");
    ensure!(
        values.iter().all(|value| !value.trim().is_empty()),
        "{owner} has empty {kind}"
    );
    Ok(set)
}

fn verify_exact_anchor(
    root: &Utf8Path,
    actual: &SourceAnchor,
    expected: ExpectedAnchor,
    owner: &str,
) -> Result<()> {
    validate_relative_path(&actual.path)?;
    ensure!(
        actual.start_line > 0 && actual.end_line >= actual.start_line,
        "{owner} source anchor range is invalid"
    );
    ensure!(
        actual.path == expected.path,
        "{owner} source anchor path does not match the authoritative path"
    );
    validate_safe_repo_file(root, &actual.path)?;
    let joined = root.join(&actual.path);
    let text =
        std::fs::read_to_string(&joined).with_context(|| format!("read anchor {}", actual.path))?;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    ensure!(
        actual.end_line <= lines.len(),
        "{owner} source anchor is outside the file"
    );
    ensure!(
        actual.start_line == expected.start_line
            && actual.end_line == expected.end_line
            && actual.sha256 == expected.sha256,
        "{owner} source anchor does not match the authoritative coordinate"
    );
    // SHA-256 covers the exact UTF-8 bytes of the inclusive 1-based source lines,
    // including each selected line's existing LF byte (and no synthesized LF).
    let selected = lines[actual.start_line - 1..actual.end_line].concat();
    ensure!(
        selected.contains(expected.required_text),
        "{owner} source span does not contain its required authoritative text"
    );
    let digest = format!("{:x}", Sha256::digest(selected.as_bytes()));
    ensure!(
        digest == actual.sha256,
        "{owner} source anchor SHA-256 mismatch"
    );
    Ok(())
}

fn expected_assumption_text(id: &str) -> (&'static str, &'static str) {
    match id {
        "trusted-host" => (
            "The formal boundary relies on the declared host faithfully executing qualified durability operations.",
            "Host OS and storage behavior used by current-core and adapter obligations.",
        ),
        "filesystem-durability" => (
            "Durability conclusions are conditional on the documented filesystem operations meeting their contracts.",
            "Durable intent, store recovery, receipts, capture, and Linux adapter claims.",
        ),
        "compiler-verifier" => (
            "All machine-checked conclusions depend on the pinned compiler and verifier preserving their documented meaning.",
            "Every formal obligation and produced executable binding.",
        ),
        "cryptographic-integrity" => (
            "Digest bindings assume collision and second-preimage resistance for the selected cryptographic hashes.",
            "Basis, acknowledgement, store, evidence, and source/evidence bindings.",
        ),
        "finite-model" => (
            "Finite state exploration covers only declared bounds and does not establish an unbounded implementation proof.",
            "Bounded ILRP recovery models and later explicitly bounded models.",
        ),
        "conditional-progress" => (
            "Recovery progress requires eventual availability; safety alone does not imply termination under perpetual failure.",
            "ILRP recovery liveness and later progress properties.",
        ),
        "linux-only" => (
            "Initial durability qualification is limited to declared Linux filesystem and syscall behavior.",
            "Current Linux durability adapters; no portability inheritance.",
        ),
        "non-hostile-qualifier" => (
            "Qualification binds a clean fixed source tree and does not cover hostile same-user mutation during the run.",
            "Qualification execution environment and evidence capture.",
        ),
        _ => unreachable!("closed assumption inventory was checked before text lookup"),
    }
}

fn validate_relative_path(path: &str) -> Result<()> {
    let path = Path::new(path);
    ensure!(!path.is_absolute(), "anchor path must be relative");
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            bail!("anchor path contains traversal or non-normal components");
        }
    }
    Ok(())
}

fn validate_safe_repo_file(root: &Utf8Path, relative: &str) -> Result<()> {
    validate_relative_path(relative)?;
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            bail!("repository path contains traversal or non-normal components");
        };
        current.push(component.to_string_lossy().as_ref());
        let metadata = std::fs::symlink_metadata(&current)
            .with_context(|| format!("inspect repository path {relative}"))?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "repository path must not contain a symlink: {relative}"
        );
    }
    ensure!(
        current.is_file(),
        "repository path is not a file: {relative}"
    );
    Ok(())
}

/// Refuse candidate v4 §§7.4 and 7.8 proof claims while evidence is unimplemented.
///
/// This command carries no proof or qualification meaning.
pub fn proof_unimplemented() -> Result<()> {
    bail!("formal proof not implemented: required proof evidence is missing")
}

/// Refuse candidate v4 §§7.4 and 7.8 model claims while evidence is unimplemented.
///
/// This command carries no proof or qualification meaning.
pub fn model_unimplemented() -> Result<()> {
    bail!("formal model not implemented: required finite-model evidence is missing")
}

/// Refuse candidate v4 §§7.4 and 7.8 adapter claims while evidence is unimplemented.
///
/// This command carries no proof or qualification meaning.
pub fn adapters_unimplemented() -> Result<()> {
    bail!("formal adapters not implemented: required adapter evidence is missing")
}

/// Refuse candidate v4 §§7.4 and 7.8 phase gates until adoption and evidence exist.
///
/// This command carries no proof, adoption, or qualification meaning.
pub fn gate_unimplemented(phase: u8) -> Result<()> {
    ensure!(
        phase <= 12,
        "unknown formal gate phase {phase}; allowed phases are 0 through 12"
    );
    bail!(
        "formal gate {phase} unavailable: registry adoption is pending and required evidence is missing"
    )
}
