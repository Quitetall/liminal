//! §23/§113 large-workspace class: demand-driven elaboration — big
//! workspaces must not be eagerly parsed in full.

/// Opening one document in a 10k-document workspace elaborates only demanded
/// regions; untouched documents remain `OpaqueBlock` stubs (v4 §22–23).
#[test]
#[ignore = "Phase 4: workspace graph + demand-driven parsing"]
fn opening_one_doc_elaborates_only_demanded_regions() {
    unimplemented!("generate 10k-doc workspace; open one; count elaborated nodes")
}
