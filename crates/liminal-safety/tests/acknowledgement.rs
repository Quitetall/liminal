//! Literal acknowledgement identity/dependency controls (v4 §7.8).
use liminal_safety::acknowledgement_matches;

#[test]
fn accepts_matching_step_identities_with_all_dependencies_acknowledged() {
    let dependencies = vec![3, 5];
    let acknowledged = vec![5, 3];

    assert!(acknowledgement_matches(
        17,
        17,
        17,
        &dependencies,
        &acknowledged,
    ));
}

#[test]
fn acknowledgement_matching_controls() {
    let cases = [
        (17, 17, 17, vec![], vec![], true),
        (17, 23, 17, vec![], vec![], false),
        (17, 17, 23, vec![], vec![], false),
        (17, 17, 17, vec![3, 5], vec![3], false),
        (17, 17, 17, vec![3], vec![3, 99], true),
        (17, 17, 17, vec![3, 3], vec![3], true),
        (17, 17, 17, vec![u128::MAX, 0], vec![0, u128::MAX], true),
        (0, u128::MAX, 0, vec![], vec![], false),
    ];

    for (step_key, planned_step, acknowledged_step, dependencies, acknowledged, expected) in cases {
        assert_eq!(
            acknowledgement_matches(
                step_key,
                planned_step,
                acknowledged_step,
                &dependencies,
                &acknowledged,
            ),
            expected,
            "unexpected result for step_key={step_key}, planned_step={planned_step}, acknowledged_step={acknowledged_step}, dependencies={dependencies:?}, acknowledged={acknowledged:?}",
        );
    }
}
