use vstd::prelude::*;

verus! {
fn admit_revision(previous: u64, proposed: u64) -> (ok: bool)
    ensures ok == (previous < u64::MAX && proposed as int == previous as int + 1),
{
    if previous == u64::MAX {
        false
    } else {
        proposed == previous + 1
    }
}
}

fn main() {
    assert!(admit_revision(0, 1));
    assert!(admit_revision(u64::MAX - 1, u64::MAX));
    assert!(!admit_revision(u64::MAX, 0));
    assert!(!admit_revision(1, 1));
    assert!(!admit_revision(1, 3));
}
