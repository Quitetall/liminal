//! Direct leaf ordering seam control.

use liminal_safety::ordering_matches;

#[test]
fn dependency_precedence_overrides_numeric_step_order() {
    let steps = [(1_u128, 1_u128, false), (2_u128, 2_u128, false)];
    let dependencies = [(2_u128, 1_u128)];
    let schedule = [2_u128, 1_u128];
    let applied = [2_u128, 1_u128];
    assert!(ordering_matches(&steps, &dependencies, &schedule, &applied));

    let wrong_schedule = [1_u128, 2_u128];
    let wrong_applied = [1_u128, 2_u128];
    assert!(!ordering_matches(
        &steps,
        &dependencies,
        &wrong_schedule,
        &wrong_applied,
    ));
}

#[test]
fn smallest_ready_identity_breaks_ties() {
    let steps = [(1_u128, 1_u128, false), (2_u128, 2_u128, false)];
    let dependencies: [(u128, u128); 0] = [];
    let good = [1_u128, 2_u128];
    assert!(ordering_matches(&steps, &dependencies, &good, &good));

    let bad = [2_u128, 1_u128];
    assert!(!ordering_matches(&steps, &dependencies, &bad, &bad));
}

#[test]
fn stable_external_first_partition() {
    let steps = [(1_u128, 1_u128, true), (2_u128, 2_u128, false)];
    let dependencies: [(u128, u128); 0] = [];
    let schedule = [1_u128, 2_u128];
    let applied = [2_u128, 1_u128];
    assert!(ordering_matches(&steps, &dependencies, &schedule, &applied));

    let wrong_applied = [1_u128, 2_u128];
    assert!(!ordering_matches(
        &steps,
        &dependencies,
        &schedule,
        &wrong_applied,
    ));
}

#[test]
fn graph_before_external_dependency_is_refused() {
    let steps = [(1_u128, 1_u128, true), (2_u128, 2_u128, false)];
    let original = [1_u128, 2_u128];
    let applied = [2_u128, 1_u128];
    let graph_before_external = [(1_u128, 2_u128)];
    assert!(!ordering_matches(
        &steps,
        &graph_before_external,
        &original,
        &applied,
    ));

    let external_before_graph = [(2_u128, 1_u128)];
    let valid = [2_u128, 1_u128];
    assert!(ordering_matches(
        &steps,
        &external_before_graph,
        &valid,
        &valid,
    ));
}

#[test]
fn empty_and_full_width_inputs_are_boundary_valid() {
    let empty: [(u128, u128, bool); 0] = [];
    let no_dependencies: [(u128, u128); 0] = [];
    let no_ids: [u128; 0] = [];
    assert!(ordering_matches(&empty, &no_dependencies, &no_ids, &no_ids));

    let full_width = [(0_u128, 0_u128, false), (u128::MAX, u128::MAX, false)];
    let dependency = [(u128::MAX, 0_u128)];
    let order = [u128::MAX, 0_u128];
    assert!(ordering_matches(&full_width, &dependency, &order, &order));
}

#[test]
fn malformed_raw_inputs_are_refused() {
    let steps = [(1_u128, 1_u128, false), (2_u128, 2_u128, false)];
    let no_dependencies: [(u128, u128); 0] = [];
    let valid = [1_u128, 2_u128];

    let mismatched_declared_id = [(1_u128, 9_u128, false), (2_u128, 2_u128, false)];
    assert!(!ordering_matches(
        &mismatched_declared_id,
        &no_dependencies,
        &valid,
        &valid,
    ));

    let duplicate_map_keys = [(1_u128, 1_u128, false), (1_u128, 1_u128, false)];
    let duplicate_ids = [1_u128, 1_u128];
    assert!(!ordering_matches(
        &duplicate_map_keys,
        &no_dependencies,
        &duplicate_ids,
        &duplicate_ids,
    ));

    let missing_schedule = [1_u128];
    assert!(!ordering_matches(
        &steps,
        &no_dependencies,
        &missing_schedule,
        &missing_schedule,
    ));
    let extra_schedule = [1_u128, 2_u128, 3_u128];
    assert!(!ordering_matches(
        &steps,
        &no_dependencies,
        &extra_schedule,
        &extra_schedule,
    ));
    let repeated_schedule = [1_u128, 1_u128];
    assert!(!ordering_matches(
        &steps,
        &no_dependencies,
        &repeated_schedule,
        &repeated_schedule,
    ));

    let unknown_dependency = [(1_u128, 3_u128)];
    assert!(!ordering_matches(
        &steps,
        &unknown_dependency,
        &valid,
        &valid,
    ));
    let self_edge = [(1_u128, 1_u128)];
    assert!(!ordering_matches(&steps, &self_edge, &valid, &valid));
    let two_node_cycle = [(1_u128, 2_u128), (2_u128, 1_u128)];
    assert!(!ordering_matches(&steps, &two_node_cycle, &valid, &valid,));

    let repeated_valid_dependency = [(2_u128, 1_u128), (2_u128, 1_u128)];
    let reverse = [2_u128, 1_u128];
    assert!(ordering_matches(
        &steps,
        &repeated_valid_dependency,
        &reverse,
        &reverse,
    ));
}

#[test]
fn three_step_stable_partition_preserves_each_subgroup() {
    let steps = [
        (1_u128, 1_u128, false),
        (2_u128, 2_u128, true),
        (3_u128, 3_u128, false),
    ];
    let dependencies: [(u128, u128); 0] = [];
    let schedule = [1_u128, 2_u128, 3_u128];
    let partitioned = [1_u128, 3_u128, 2_u128];
    assert!(ordering_matches(
        &steps,
        &dependencies,
        &schedule,
        &partitioned,
    ));

    let reordered_external = [3_u128, 1_u128, 2_u128];
    assert!(!ordering_matches(
        &steps,
        &dependencies,
        &schedule,
        &reordered_external,
    ));

    let graph_steps = [
        (1_u128, 1_u128, true),
        (2_u128, 2_u128, false),
        (3_u128, 3_u128, true),
    ];
    let graph_partitioned = [2_u128, 1_u128, 3_u128];
    assert!(ordering_matches(
        &graph_steps,
        &dependencies,
        &schedule,
        &graph_partitioned,
    ));

    let reordered_graph = [2_u128, 3_u128, 1_u128];
    assert!(!ordering_matches(
        &graph_steps,
        &dependencies,
        &schedule,
        &reordered_graph,
    ));
}
