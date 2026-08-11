//! §113 malformed-source recovery class: parsing never panics, always yields
//! an error-tolerant CST preserving the invalid regions (v4 §9, §22).

use sha2::Digest;

/// Every fixture under `fixtures/malformed-source/` parses without panic into
/// a CST that preserves invalid regions as opaque blocks with exact ranges,
/// and `emit` round-trips the damaged bytes losslessly (L0 is lossless).
#[test]
fn malformed_source_never_panics_and_round_trips() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/malformed-source");
    let mut count = 0;
    for entry in std::fs::read_dir(&root).expect("malformed fixture directory") {
        let path = entry.expect("fixture entry").path();
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).expect("fixture bytes");
        let source = std::str::from_utf8(&bytes).expect("fixture is UTF-8");
        let basis = liminal_source::SourceBasis {
            source: liminal_id::SourceId::from_name(path.to_string_lossy().as_ref()),
            content_hash: liminal_id::ContentHash::of(source.as_bytes()),
        };
        let view = liminal_source::Utf8HolderView::from_bytes(basis, source.as_bytes())
            .expect("fixture is UTF-8");
        let document = liminal_cst::parse(&view);
        assert_eq!(document.emit_lossless().as_bytes(), source.as_bytes());
        count += 1;
    }
    assert!(count >= 16, "Phase 1 malformed seed floor not met: {count}");
}

/// Fuzz smoke lives in fuzz/ (created with liminal-cst, see
/// docs/implementation-plan.md defer list); this class holds its minimized
/// regression cases per §113.
#[test]
fn fuzz_regressions_stay_fixed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("corpora/regression/phase1");
    let mut count = 0;
    for target in std::fs::read_dir(&root).expect("regression root") {
        let target = target.expect("target entry").path();
        if !target.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&target).expect("target corpus") {
            let path = entry.expect("corpus entry").path();
            if path.extension().is_some_and(|ext| ext == "case") {
                let bytes = std::fs::read(&path).expect("regression bytes");
                let metadata_path = path.with_extension("json");
                let metadata: serde_json::Value = serde_json::from_slice(
                    &std::fs::read(&metadata_path).expect("regression metadata bytes"),
                )
                .expect("regression metadata JSON");
                assert_eq!(metadata["schema_version"], 1);
                assert_eq!(
                    metadata["target"],
                    target.file_name().unwrap().to_string_lossy().as_ref()
                );
                let actual_hash = format!("{:x}", sha2::Sha256::digest(&bytes));
                assert_eq!(metadata["input_sha256"], actual_hash);
                let source = std::str::from_utf8(&bytes).expect("regression is UTF-8");
                let basis = liminal_source::SourceBasis {
                    source: liminal_id::SourceId::from_name(path.to_string_lossy().as_ref()),
                    content_hash: liminal_id::ContentHash::of(source.as_bytes()),
                };
                let view = liminal_source::Utf8HolderView::from_bytes(basis, source.as_bytes())
                    .expect("regression is UTF-8");
                let document = liminal_cst::parse(&view);
                assert_eq!(document.emit_lossless().as_bytes(), source.as_bytes());
                count += 1;
            }
        }
    }
    assert!(count > 0, "no minimized Phase 1 regression cases found");
}
