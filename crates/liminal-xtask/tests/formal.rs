//! Integration tests for the candidate formal command boundary.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use serde_json::{Value, json};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: Utf8PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let source = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Utf8Path::parent)
            .expect("workspace root");
        let root = Utf8PathBuf::from_path_buf(std::env::temp_dir())
            .expect("UTF-8 temp dir")
            .join(format!(
                "liminal-formal-test-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir(&root).expect("create exclusive fixture root");
        for relative in [
            "verification/obligations.json",
            "verification/assumptions.json",
            "spec/v4/liminal_master_architecture_plan_v4.md",
            "docs/adr/0007-hand-rolled-phase-minus-1-toy-store.md",
            "docs/adr/0020-require-high-assurance-phase-1-suite-qualification.md",
            "docs/adr/0021-stage-haqp-1-qualification-around-phase-1-authorization.md",
        ] {
            let target = root.join(relative);
            std::fs::create_dir_all(target.parent().expect("parent")).expect("create fixture dir");
            std::fs::copy(source.join(relative), &target).expect("copy fixture file");
        }
        let fixture = Self { root };
        liminal_xtask::formal::verify_registry_repo(&fixture.root)
            .expect("freshly copied fixture must be a valid positive control");
        fixture
    }

    fn obligations(&self) -> Value {
        self.read_json("verification/obligations.json")
    }

    fn assumptions(&self) -> Value {
        self.read_json("verification/assumptions.json")
    }

    fn read_json(&self, relative: &str) -> Value {
        serde_json::from_slice(&std::fs::read(self.root.join(relative)).expect("read fixture JSON"))
            .expect("parse fixture JSON")
    }

    fn write_json(&self, relative: &str, value: &Value) {
        std::fs::write(
            self.root.join(relative),
            serde_json::to_vec_pretty(value).expect("serialize fixture JSON"),
        )
        .expect("write fixture JSON");
    }

    fn rejects(&self) {
        let _ = self.error();
    }

    fn error(&self) -> String {
        let error = liminal_xtask::formal::verify_registry_repo(&self.root)
            .expect_err("mutated registry unexpectedly passed");
        format!("{error:#}")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).expect("remove isolated test fixture");
    }
}

fn xtask() -> Command {
    Command::new(env!("CARGO_BIN_EXE_liminal-xtask"))
}

#[test]
fn production_inventory_is_structurally_valid_but_not_qualification() {
    let output = xtask()
        .args(["formal", "check"])
        .output()
        .expect("run formal check");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(stdout.contains("NOT QUALIFICATION"), "stdout: {stdout}");

    let fixture = Fixture::new();
    let registry = fixture.obligations();
    let ids: Vec<_> = registry["obligations"]
        .as_array()
        .expect("obligation rows")
        .iter()
        .map(|row| row["id"].as_str().expect("obligation ID"))
        .collect();
    assert_eq!(ids.len(), 16);
    assert!(ids.contains(&"ilrp-apply"));
    assert!(ids.contains(&"ilrp-revert"));
}

#[test]
fn missing_apply_or_revert_is_rejected() {
    for missing in ["ilrp-apply", "ilrp-revert"] {
        let fixture = Fixture::new();
        let mut registry = fixture.obligations();
        let rows = registry["obligations"]
            .as_array_mut()
            .expect("obligation rows");
        let position = rows
            .iter()
            .position(|row| row["id"] == missing)
            .expect("required ILRP obligation");
        rows.remove(position);
        fixture.write_json("verification/obligations.json", &registry);
        fixture.rejects();
    }
}

#[test]
fn obligation_inventory_rejects_missing_extra_and_duplicate_rows() {
    for mutation in ["missing", "extra", "duplicate"] {
        let fixture = Fixture::new();
        let mut registry = fixture.obligations();
        let rows = registry["obligations"]
            .as_array_mut()
            .expect("obligation rows");
        match mutation {
            "missing" => {
                rows.pop();
            }
            "extra" => rows.push(json!({
                "id": "invented",
                "requirement_anchors": ["invented"],
                "applicability": "current-core",
                "assumption_ids": ["compiler-verifier"],
                "status": "not-established",
                "source_anchors": []
            })),
            "duplicate" => rows.push(rows[0].clone()),
            _ => unreachable!(),
        }
        fixture.write_json("verification/obligations.json", &registry);
        fixture.rejects();
    }
}

#[test]
fn obligation_mapping_status_and_adoption_are_fixed() {
    for (pointer, replacement) in [
        ("/obligations/0/requirement_anchors/0", json!("wrong")),
        ("/obligations/0/applicability", json!("phase-1")),
        ("/obligations/0/status", json!("established")),
        ("/adoption/state", json!("adopted")),
        (
            "/obligations/0/assumption_ids",
            json!(["compiler-verifier"]),
        ),
    ] {
        let fixture = Fixture::new();
        let mut registry = fixture.obligations();
        *registry.pointer_mut(pointer).expect("mutation pointer") = replacement;
        fixture.write_json("verification/obligations.json", &registry);
        fixture.rejects();
    }
}

#[test]
fn recursively_unknown_fields_are_rejected() {
    for (file, pointer) in [
        ("verification/obligations.json", "/adoption"),
        (
            "verification/obligations.json",
            "/obligations/0/source_anchors/0",
        ),
        (
            "verification/assumptions.json",
            "/assumptions/0/source_anchor",
        ),
    ] {
        let fixture = Fixture::new();
        let mut registry = fixture.read_json(file);
        registry
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .expect("object pointer")
            .insert("unknown".into(), json!(true));
        fixture.write_json(file, &registry);
        fixture.rejects();
    }
}

#[test]
fn assumption_inventory_and_metadata_are_closed() {
    let fixture = Fixture::new();
    let mut registry = fixture.assumptions();
    registry["assumptions"]
        .as_array_mut()
        .expect("assumption rows")
        .pop();
    fixture.write_json("verification/assumptions.json", &registry);
    fixture.rejects();

    for mutation in ["extra", "duplicate"] {
        let fixture = Fixture::new();
        let mut registry = fixture.assumptions();
        let rows = registry["assumptions"]
            .as_array_mut()
            .expect("assumption rows");
        if mutation == "duplicate" {
            rows.push(rows[0].clone());
        } else {
            let mut extra = rows[0].clone();
            extra["id"] = json!("invented-assumption");
            rows.push(extra);
        }
        fixture.write_json("verification/assumptions.json", &registry);
        fixture.rejects();
    }

    for pointer in ["/assumptions/0/rationale", "/assumptions/0/scope"] {
        let fixture = Fixture::new();
        let mut registry = fixture.assumptions();
        *registry.pointer_mut(pointer).expect("mutation pointer") = json!("");
        fixture.write_json("verification/assumptions.json", &registry);
        fixture.rejects();
    }

    for (pointer, replacement, diagnostic) in [
        (
            "/assumptions/0/rationale",
            "A different but still nonempty rationale.",
            "trusted-host rationale must match",
        ),
        (
            "/assumptions/0/scope",
            "A different but still nonempty scope.",
            "trusted-host scope must match",
        ),
    ] {
        let fixture = Fixture::new();
        let mut registry = fixture.assumptions();
        *registry.pointer_mut(pointer).expect("mutation pointer") = json!(replacement);
        fixture.write_json("verification/assumptions.json", &registry);
        let error = fixture.error();
        assert!(error.contains(diagnostic), "unexpected error: {error}");
    }
}

#[test]
fn source_anchor_coordinate_hash_and_range_are_fixed() {
    for (pointer, replacement) in [
        ("/obligations/0/source_anchors", json!([])),
        (
            "/obligations/0/source_anchors/0/path",
            json!("docs/adr/0018-preserve-interpretive-jurisdiction-oracles.md"),
        ),
        ("/obligations/0/source_anchors/0/sha256", json!("00")),
        ("/obligations/0/source_anchors/0/start_line", json!(0)),
        ("/obligations/0/source_anchors/0/end_line", json!(999_999)),
        (
            "/obligations/0/source_anchors/0/path",
            json!("../liminal_master_architecture_plan_v4.md"),
        ),
        (
            "/obligations/0/source_anchors/0/path",
            json!("/tmp/liminal_master_architecture_plan_v4.md"),
        ),
    ] {
        let fixture = Fixture::new();
        let mut registry = fixture.obligations();
        *registry.pointer_mut(pointer).expect("mutation pointer") = replacement;
        fixture.write_json("verification/obligations.json", &registry);
        fixture.rejects();
    }
}

#[test]
fn unsafe_path_and_range_diagnostics_are_reachable() {
    for (pointer, replacement, diagnostic) in [
        (
            "/obligations/0/source_anchors/0/path",
            json!("../liminal_master_architecture_plan_v4.md"),
            "traversal or non-normal components",
        ),
        (
            "/obligations/0/source_anchors/0/path",
            json!("/tmp/liminal_master_architecture_plan_v4.md"),
            "anchor path must be relative",
        ),
        (
            "/obligations/0/source_anchors/0/start_line",
            json!(0),
            "source anchor range is invalid",
        ),
        (
            "/obligations/0/source_anchors/0/end_line",
            json!(999_999),
            "source anchor is outside the file",
        ),
    ] {
        let fixture = Fixture::new();
        let mut registry = fixture.obligations();
        *registry.pointer_mut(pointer).expect("mutation pointer") = replacement;
        fixture.write_json("verification/obligations.json", &registry);
        let error = fixture.error();
        assert!(error.contains(diagnostic), "unexpected error: {error}");
    }
}

#[test]
fn unapproved_existing_path_is_rejected_before_any_source_read() {
    let fixture = Fixture::new();
    let sentinel = fixture.root.join("spec/v4/unapproved-invalid.bin");
    std::fs::write(&sentinel, [0xff, 0xfe]).expect("write invalid UTF-8 sentinel");
    let mut registry = fixture.obligations();
    registry["obligations"][0]["source_anchors"][0]["path"] =
        json!("spec/v4/unapproved-invalid.bin");
    fixture.write_json("verification/obligations.json", &registry);

    let error = fixture.error();
    assert!(
        error.contains("source anchor path does not match the authoritative path"),
        "unexpected error (unapproved file may have been read): {error}"
    );
    assert!(
        !error.contains("UTF-8"),
        "unapproved sentinel was read: {error}"
    );
}

#[test]
fn source_anchor_hashes_the_actual_anchored_bytes() {
    let fixture = Fixture::new();
    let source = fixture
        .root
        .join("spec/v4/liminal_master_architecture_plan_v4.md");
    let text = std::fs::read_to_string(&source).expect("read copied spec");
    let mutated = text.replacen("Documents, text", "Documents,  text", 1);
    assert_ne!(mutated, text, "anchored source mutation must apply");
    std::fs::write(source, mutated).expect("mutate copied anchored bytes");

    let error = fixture.error();
    assert!(
        error.contains("identity-only source anchor SHA-256 mismatch"),
        "unexpected error: {error}"
    );
}

#[cfg(unix)]
#[test]
fn source_and_registry_symlink_indirection_is_rejected() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let spec = fixture
        .root
        .join("spec/v4/liminal_master_architecture_plan_v4.md");
    let saved = fixture.root.join("spec-source.md");
    std::fs::rename(&spec, &saved).expect("move source");
    symlink(&saved, &spec).expect("create file symlink");
    fixture.rejects();

    let fixture = Fixture::new();
    let v4 = fixture.root.join("spec/v4");
    let saved = fixture.root.join("real-v4");
    std::fs::rename(&v4, &saved).expect("move source directory");
    symlink(&saved, &v4).expect("create parent directory symlink");
    fixture.rejects();

    let fixture = Fixture::new();
    let registry = fixture.root.join("verification/obligations.json");
    let saved = fixture.root.join("obligations-saved.json");
    std::fs::rename(&registry, &saved).expect("move registry");
    symlink(&saved, &registry).expect("create registry symlink");
    fixture.rejects();
}

#[test]
fn unimplemented_commands_and_all_recognized_gates_fail_closed() {
    for args in [
        vec!["formal", "proof"],
        vec!["formal", "model"],
        vec!["formal", "adapters"],
        vec!["formal", "gate", "0"],
        vec!["formal", "gate", "10"],
        vec!["formal", "gate", "11"],
        vec!["formal", "gate", "12"],
    ] {
        let output = xtask().args(&args).output().expect("run formal command");
        assert!(!output.status.success(), "{args:?} unexpectedly succeeded");
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
        assert!(
            stderr.contains("missing") || stderr.contains("pending"),
            "stderr: {stderr}"
        );
        assert!(
            !stderr.contains("unknown formal gate phase"),
            "recognized gate rejected as unknown: {stderr}"
        );
    }

    let output = xtask()
        .args(["formal", "gate", "13"])
        .output()
        .expect("run unknown gate");
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .expect("UTF-8 stderr")
            .contains("unknown formal gate phase 13")
    );
}
