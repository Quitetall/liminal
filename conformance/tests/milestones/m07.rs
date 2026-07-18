//! M07 exit gate (Killer #4): the identity guarantee matrix and the
//! claims-never-exceed-evidence cap (v4 §-1.1, §19; Law 12/13).
//!
//! `matrix_matches_golden` freezes the demonstrated outcomes into
//! `conformance/golden/identity_matrix.md` (git-version line redacted for
//! cross-machine stability; the ≥2.44 floor is asserted separately, D07.6).
//! `claims_never_exceed_evidence` is Algorithm E: no profile's declared
//! `required_identity` exceeds what the corpus demonstrates, and every inline-id
//! degradation is either floor-bounded or loudly surfaced by the M03 checker
//! (DG-7.1).

use camino::Utf8PathBuf;
use liminal_conformance::identity::matrix::{Matrix, redact_git_line};
use liminal_conformance::identity::strategy::Outcome;
use liminal_conformance::identity::{Config, Strategy, claims, gitenv};
use liminal_id::IdentityGrade;

/// The workspace root (conformance/ has a parent).
fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_owned()
}

/// The committed golden path (v4 §-1.1; golden/README.md).
fn golden_path() -> Utf8PathBuf {
    repo_root().join("conformance/golden/identity_matrix.md")
}

/// Build the corpus config and drive the full matrix.
fn build_matrix() -> Matrix {
    let cfg = Config::load().expect("load identity config");
    Matrix::build(&cfg).expect("build identity matrix")
}

/// `matrix_matches_golden`: the demonstrated matrix matches the committed
/// published claim byte-for-byte, modulo the redacted `git:` header line. A
/// drift is a change to the project's identity claims and must be re-blessed
/// (`BLESS_IDENTITY_MATRIX=1`) and reviewed. Separately asserts the git floor.
#[test]
fn matrix_matches_golden() {
    // D07.6: the run FAILS (never skips) on a below-floor git.
    gitenv::assert_min_git("2.44").expect("git must meet the merge-ort floor (D07.6)");

    let matrix = build_matrix();
    let rendered = matrix.render();
    let path = golden_path();

    if std::env::var("BLESS_IDENTITY_MATRIX").is_ok() || !path.exists() {
        std::fs::write(&path, &rendered).expect("write golden");
    }
    let golden = std::fs::read_to_string(&path).expect("read identity_matrix.md golden");

    assert_eq!(
        redact_git_line(&rendered),
        redact_git_line(&golden),
        "identity matrix drifted from the committed golden; if this is an intended \
         claim change, regenerate with BLESS_IDENTITY_MATRIX=1 and review the diff"
    );
}

/// `claims_never_exceed_evidence` (Algorithm E): the three caps.
#[test]
fn claims_never_exceed_evidence() {
    let matrix = build_matrix();
    let ext_floor = claims::external_file_floor();
    let graph_floor = claims::graph_native_floor();

    // Assertion 1: the external-file floor is bounded by every file strategy's
    // column cap (`floor ≤ cap(col)`), so the profile never over-claims.
    for strat in [
        Strategy::InlineId,
        Strategy::Sidecar,
        Strategy::Structural,
        Strategy::RevisionAnchor,
    ] {
        let idx = claims::strategy_index(&matrix, strat).expect("file strategy column present");
        let cap = claims::column_cap(&matrix, idx);
        assert!(
            cap.satisfies(ext_floor),
            "{}: column cap {cap:?} does not satisfy the external-file floor {ext_floor:?}",
            strat.kebab()
        );
    }

    // Assertion 2: the graph-native floor is bounded by the managed-graph cap
    // computed over OBSERVED rows only (graph-native subjects have no file for
    // git to mangle — v4 §7.6).
    let mg = claims::strategy_index(&matrix, Strategy::ManagedGraph).expect("managed-graph column");
    let mg_cap = claims::column_cap(&matrix, mg);
    assert!(
        mg_cap.satisfies(graph_floor),
        "managed-graph observed cap {mg_cap:?} does not satisfy the graph-native floor \
         {graph_floor:?}"
    );

    // Assertion 3 (grade_of ceiling, DG-7.1): every inline-id cell whose cap is
    // below Explicit is either an `Ambiguous` collision the M03 checker surfaces
    // (JUR042 on replay — never silent) or a `Lost` id bounded by the declared
    // floor (information-theoretically unsurfaceable, v4 §19.2).
    let ii = claims::strategy_index(&matrix, Strategy::InlineId).expect("inline-id column");
    for (op_idx, op) in matrix.operations.iter().enumerate() {
        let cell = &matrix.cells[&(op_idx, ii)];
        if cell.outcome.cap().strength() >= IdentityGrade::Explicit.strength() {
            continue;
        }
        match &cell.outcome {
            Outcome::Ambiguous => {
                let findings =
                    claims::checker_findings(&cell.witness_file, &format!("inline-{}", op.name()));
                assert!(
                    findings >= 1,
                    "inline-id/{} demonstrates Ambiguous identity but the M03 checker stayed \
                     silent on its post-op file — a duplicate-id collision must surface \
                     (JUR041/JUR042)",
                    op.name()
                );
            }
            Outcome::Lost => {
                assert_eq!(
                    cell.outcome.cap(),
                    ext_floor,
                    "inline-id/{} Lost must be bounded by the external-file floor (v4 §19.2)",
                    op.name()
                );
            }
            other => panic!(
                "inline-id/{} has cap < Explicit but an unexpected outcome {other:?}",
                op.name()
            ),
        }
    }
}
