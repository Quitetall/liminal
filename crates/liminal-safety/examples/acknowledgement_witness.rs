//! Executable witness for the safety acknowledgement rules in spec v4 §7.8.

use liminal_safety::acknowledgement_matches;

fn main() {
    assert!(acknowledgement_matches(17, 17, 17, &[3, 5], &[5, 3],));
    assert!(!acknowledgement_matches(17, 17, 17, &[3, 5], &[3],));
    assert!(!acknowledgement_matches(17, 17, 23, &[], &[],));
    assert!(acknowledgement_matches(17, 17, 17, &[], &[],));
    assert!(acknowledgement_matches(
        u128::MAX,
        u128::MAX,
        u128::MAX,
        &[0],
        &[0],
    ));

    println!("acknowledgement-witness:5");
}
