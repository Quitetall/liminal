//! Executable acknowledgement identity/dependency checks (v4 §7.8).
//! Integers encode existing repair-step identities, not new semantic primitives.
//! These checks do not establish poststate truth, durable history or authority.
//! No allocator or standard-library thread adapter is enabled in this leaf.
//!
//! ```compile_fail
//! use vstd::thread;
//! ```

#![no_std]

use vstd::prelude::*;

verus! {

fn contains_identity(values: &[u128], identity: u128) -> (found: bool)
    ensures found == values@.contains(identity),
{
    let mut index: usize = 0;
    while index < values.len()
        invariant
            index <= values.len(),
            forall|i: int| 0 <= i < index ==> values@[i] != identity,
        decreases values.len() - index,
    {
        if values[index] == identity {
            return true;
        }
        index += 1;
    }
    false
}

/// Check exact step identity and every dependency acknowledgement (v4 §7.8).
/// Input order and duplicate dependencies do not confer extra authority.
pub fn acknowledgement_matches(
    step_key: u128,
    planned_step: u128,
    acknowledged_step: u128,
    dependencies: &[u128],
    acknowledged: &[u128],
) -> (accepted: bool)
    ensures accepted == (
        step_key == planned_step && step_key == acknowledged_step
        && forall|i: int| 0 <= i < dependencies.len()
            ==> acknowledged@.contains(dependencies@[i])
    ),
{
    if step_key != planned_step || step_key != acknowledged_step {
        return false;
    }
    let mut index: usize = 0;
    while index < dependencies.len()
        invariant
            index <= dependencies.len(),
            forall|i: int| 0 <= i < index
                ==> acknowledged@.contains(dependencies@[i]),
        decreases dependencies.len() - index,
    {
        if !contains_identity(acknowledged, dependencies[index]) {
            return false;
        }
        index += 1;
    }
    true
}

}
