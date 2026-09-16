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

/// Existing graph-step classification; this introduces no semantic identity.
pub open spec fn graph_key(steps: Seq<(u128, u128, bool)>, identity: u128) -> bool {
    exists|i: int| 0 <= i < steps.len() && steps[i].0 == identity && steps[i].2
}

/// Every incoming prerequisite occurs in the already executed prefix.
pub open spec fn ready_at(
    dependencies: Seq<(u128, u128)>, schedule: Seq<u128>, identity: u128, prefix: int,
) -> bool {
    forall|edge: int| 0 <= edge < dependencies.len()
        && dependencies[edge].1 == identity
        ==> exists|position: int| 0 <= position < prefix
            && position < schedule.len() && schedule[position] == dependencies[edge].0
}

/// Exact existential membership used by the ordering contract.
pub open spec fn names_step(steps: Seq<(u128, u128, bool)>, identity: u128) -> bool {
    exists|j: int| 0 <= j < steps.len() && identity == steps[j].0
}

proof fn names_step_definition(steps: Seq<(u128, u128, bool)>, identity: u128)
    ensures names_step(steps, identity)
        == (exists|j: int| 0 <= j < steps.len() && identity == steps[j].0),
{
}

/// Full candidate ordering contract; qualification remains separate.
pub open spec fn ordering_valid(
    steps: Seq<(u128, u128, bool)>, dependencies: Seq<(u128, u128)>,
    schedule: Seq<u128>, applied: Seq<u128>,
) -> bool {
    &&& schedule.len() == steps.len()
    &&& (forall|i: int| 0 <= i < steps.len() ==> steps[i].0 == steps[i].1)
    &&& (forall|i: int, j: int| 0 <= i < j < steps.len() ==> steps[i].0 != steps[j].0)
    &&& (forall|i: int| 0 <= i < steps.len() ==> schedule.contains(steps[i].0))
    &&& (forall|i: int| #![trigger schedule[i]] 0 <= i < schedule.len()
        ==> names_step(steps, schedule[i]))
    &&& (forall|i: int, j: int| 0 <= i < j < schedule.len() ==> schedule[i] != schedule[j])
    &&& (forall|edge: int| 0 <= edge < dependencies.len() ==> (
        (exists|before: int, after: int| 0 <= before < after < schedule.len()
            && schedule[before] == dependencies[edge].0
            && schedule[after] == dependencies[edge].1)
        && !(graph_key(steps, dependencies[edge].0)
            && !graph_key(steps, dependencies[edge].1))
    ))
    &&& (forall|chosen: int, candidate: int| 0 <= chosen < candidate < schedule.len()
        && schedule[candidate] < schedule[chosen]
        ==> !ready_at(dependencies, schedule, schedule[candidate], chosen))
    &&& applied == schedule.filter(|identity: u128| !graph_key(steps, identity))
        + schedule.filter(|identity: u128| graph_key(steps, identity))
}

proof fn ordering_intro(
    steps: Seq<(u128, u128, bool)>, dependencies: Seq<(u128, u128)>,
    schedule: Seq<u128>, applied: Seq<u128>,
)
    requires
        schedule.len() == steps.len(),
        forall|i: int| 0 <= i < steps.len() ==> steps[i].0 == steps[i].1,
        forall|i: int, j: int| 0 <= i < j < steps.len() ==> steps[i].0 != steps[j].0,
        forall|i: int| 0 <= i < steps.len() ==> schedule.contains(steps[i].0),
        forall|i: int| #![trigger schedule[i]] 0 <= i < schedule.len()
            ==> names_step(steps, schedule[i]),
        forall|i: int, j: int| 0 <= i < j < schedule.len() ==> schedule[i] != schedule[j],
        forall|edge: int| 0 <= edge < dependencies.len() ==> (
            (exists|before: int, after: int| 0 <= before < after < schedule.len()
                && schedule[before] == dependencies[edge].0
                && schedule[after] == dependencies[edge].1)
            && !(graph_key(steps, dependencies[edge].0)
                && !graph_key(steps, dependencies[edge].1))),
        forall|chosen: int, candidate: int| 0 <= chosen < candidate < schedule.len()
            && schedule[candidate] < schedule[chosen]
            ==> !ready_at(dependencies, schedule, schedule[candidate], chosen),
        applied == schedule.filter(|identity: u128| !graph_key(steps, identity))
            + schedule.filter(|identity: u128| graph_key(steps, identity)),
    ensures ordering_valid(steps, dependencies, schedule, applied),
{
}

proof fn partition_length(steps: Seq<(u128, u128, bool)>, values: Seq<u128>)
    ensures
        values.filter(|identity: u128| !graph_key(steps, identity)).len()
            + values.filter(|identity: u128| graph_key(steps, identity)).len()
            == values.len(),
    decreases values.len(),
{
    reveal(Seq::filter);
    if values.len() > 0 {
        partition_length(steps, values.drop_last());
    }
}

proof fn equal_size_membership(keys: Seq<u128>, schedule: Seq<u128>)
    requires
        keys.len() == schedule.len(),
        keys.no_duplicates(),
        forall|i: int| 0 <= i < keys.len() ==> schedule.contains(keys[i]),
    ensures
        schedule.no_duplicates(),
        forall|i: int| 0 <= i < schedule.len() ==> keys.contains(schedule[i]),
{
    keys.to_set_ensures();
    schedule.to_set_ensures();
    keys.unique_seq_to_set();
    schedule.lemma_cardinality_of_set();
    assert(keys.to_set().subset_of(schedule.to_set()));
    vstd::set_lib::lemma_len_subset(keys.to_set(), schedule.to_set());
    schedule.lemma_no_dup_set_cardinality();
    assert forall|value: u128| schedule.to_set().contains(value)
        implies keys.to_set().contains(value) by {
        if !keys.to_set().contains(value) {
            keys.to_set().lemma_subset_not_in_lt(schedule.to_set(), value);
        }
    }
    assert(keys.to_set() =~= schedule.to_set());
    assert forall|i: int| 0 <= i < schedule.len()
        implies keys.contains(schedule[i]) by {
        assert(schedule.contains(schedule[i]));
        assert(schedule.to_set().contains(schedule[i]));
        assert(keys.to_set().contains(schedule[i]));
    }
}

proof fn step_reverse_membership(steps: Seq<(u128, u128, bool)>, schedule: Seq<u128>)
    requires
        steps.len() == schedule.len(),
        forall|i: int, j: int| 0 <= i < j < steps.len() ==> steps[i].0 != steps[j].0,
        forall|i: int| 0 <= i < steps.len() ==> schedule.contains(steps[i].0),
    ensures
        schedule.no_duplicates(),
        forall|i: int| #![trigger schedule[i]] 0 <= i < schedule.len()
            ==> names_step(steps, schedule[i]),
{
    let keys = steps.map_values(|step: (u128, u128, bool)| step.0);
    assert(keys.no_duplicates());
    assert forall|i: int| 0 <= i < keys.len() implies schedule.contains(keys[i]) by {
        assert(keys[i] == steps[i].0);
    }
    equal_size_membership(keys, schedule);
    assert forall|i: int| #![trigger schedule[i]] 0 <= i < schedule.len()
        implies names_step(steps, schedule[i]) by {
        assert(keys.contains(schedule[i]));
        let j = choose|j: int| 0 <= j < keys.len() && keys[j] == schedule[i];
        assert(keys[j] == steps[j].0);
    }
}

proof fn filter_scan_step(values: Seq<u128>, predicate: spec_fn(u128) -> bool, index: int)
    requires 0 <= index < values.len(),
    ensures
        values.take(index + 1).filter(predicate)
            == if predicate(values[index]) {
                values.take(index).filter(predicate).push(values[index])
            } else {
                values.take(index).filter(predicate)
            },
        predicate(values[index]) ==> (
            values.take(index).filter(predicate).len() < values.filter(predicate).len()
            && values.filter(predicate)[values.take(index).filter(predicate).len() as int]
                == values[index]
        ),
{
    values.lemma_take_succ_push(index);
    values.take(index).lemma_filter_push(values[index], predicate);
    assert(values == values.take(index + 1) + values.skip(index + 1));
    Seq::filter_distributes_over_add(
        values.take(index + 1), values.skip(index + 1), predicate);
}

fn identity_position(values: &[u128], identity: u128) -> (position: usize)
    ensures
        position <= values.len(),
        forall|i: int| 0 <= i < position ==> values@[i] != identity,
        position < values.len() ==> values@[position as int] == identity,
{
    let mut index: usize = 0;
    while index < values.len()
        invariant
            index <= values.len(),
            forall|i: int| 0 <= i < index ==> values@[i] != identity,
        decreases values.len() - index,
    {
        if values[index] == identity {
            return index;
        }
        index += 1;
    }
    index
}

fn graph_identity(steps: &[(u128, u128, bool)], identity: u128) -> (found: bool)
    ensures found == (exists|i: int| 0 <= i < steps.len()
        && steps@[i].0 == identity && steps@[i].2),
{
    let mut index: usize = 0;
    while index < steps.len()
        invariant
            index <= steps.len(),
            forall|i: int| 0 <= i < index
                ==> !(steps@[i].0 == identity && steps@[i].2),
        decreases steps.len() - index,
    {
        if steps[index].0 == identity && steps[index].2 {
            return true;
        }
        index += 1;
    }
    false
}

fn smallest_ready_matches(dependencies: &[(u128, u128)], schedule: &[u128]) -> (accepted: bool)
    ensures accepted == (forall|chosen: int, candidate: int| 0 <= chosen < candidate < schedule.len()
        && schedule@[candidate] < schedule@[chosen]
        ==> !ready_at(dependencies@, schedule@, schedule@[candidate], chosen)),
{
    let mut selected: usize = 0;
    while selected < schedule.len()
        invariant
            selected <= schedule.len(),
            forall|chosen: int, candidate: int| 0 <= chosen < selected
                && chosen < candidate < schedule.len()
                && schedule@[candidate] < schedule@[chosen]
                ==> !ready_at(dependencies@, schedule@, schedule@[candidate], chosen),
        decreases schedule.len() - selected,
    {
        let mut candidate = selected + 1;
        while candidate < schedule.len()
            invariant
                selected < candidate <= schedule.len(),
                selected < schedule.len(),
                forall|next: int| selected < next < candidate
                    && schedule@[next] < schedule@[selected as int]
                    ==> !ready_at(dependencies@, schedule@, schedule@[next], selected as int),
            decreases schedule.len() - candidate,
        {
            if schedule[candidate] < schedule[selected] {
                let mut blocked = false;
                let mut incoming: usize = 0;
                while incoming < dependencies.len()
                    invariant
                        incoming <= dependencies.len(),
                        candidate < schedule.len(),
                        selected < candidate,
                        schedule@[candidate as int] < schedule@[selected as int],
                        blocked ==> !ready_at(dependencies@, schedule@,
                            schedule@[candidate as int], selected as int),
                        !blocked ==> (forall|edge: int| 0 <= edge < incoming
                            && dependencies@[edge].1 == schedule@[candidate as int]
                            ==> exists|position: int| 0 <= position < selected
                                && position < schedule.len()
                                && schedule@[position] == dependencies@[edge].0),
                    decreases dependencies.len() - incoming,
                {
                    if dependencies[incoming].1 == schedule[candidate]
                        && identity_position(schedule, dependencies[incoming].0) >= selected
                    {
                        blocked = true;
                    }
                    incoming += 1;
                }
                if !blocked {
                    proof {
                        assert(ready_at(dependencies@, schedule@,
                            schedule@[candidate as int], selected as int));
                    }
                    return false;
                }
            }
            candidate += 1;
        }
        selected += 1;
    }
    true
}

fn stable_partition_matches(
    steps: &[(u128, u128, bool)], schedule: &[u128], applied: &[u128],
) -> (accepted: bool)
    ensures accepted == (applied@ == schedule@.filter(|identity: u128| !graph_key(steps@, identity))
        + schedule@.filter(|identity: u128| graph_key(steps@, identity))),
{
    proof { partition_length(steps@, schedule@); }
    if applied.len() != schedule.len() { return false; }
    let mut group: usize = 0;
    let mut applied_index: usize = 0;
    while group < 2
        invariant
            group <= 2, applied_index <= applied.len(),
            applied.len() == schedule@.filter(|identity: u128| !graph_key(steps@, identity)).len()
                + schedule@.filter(|identity: u128| graph_key(steps@, identity)).len(),
            forall|i: int| 0 <= i < applied_index ==> applied@[i] ==
                (schedule@.filter(|identity: u128| !graph_key(steps@, identity))
                    + schedule@.filter(|identity: u128| graph_key(steps@, identity)))[i],
            group == 0 ==> applied_index == 0,
            group == 1 ==> applied_index == schedule@.filter(
                |identity: u128| !graph_key(steps@, identity)).len(),
            group == 2 ==> applied_index == schedule@.filter(
                |identity: u128| !graph_key(steps@, identity)).len()
                + schedule@.filter(|identity: u128| graph_key(steps@, identity)).len(),
        decreases 2 - group,
    {
        let mut original: usize = 0;
        while original < schedule.len()
            invariant
                group < 2,
                original <= schedule.len(), applied_index <= applied.len(),
                applied.len() == schedule@.filter(|identity: u128| !graph_key(steps@, identity)).len()
                    + schedule@.filter(|identity: u128| graph_key(steps@, identity)).len(),
                forall|i: int| 0 <= i < applied_index ==> applied@[i] ==
                    (schedule@.filter(|identity: u128| !graph_key(steps@, identity))
                        + schedule@.filter(|identity: u128| graph_key(steps@, identity)))[i],
                applied_index == (if group == 0 { 0 } else {
                    schedule@.filter(|identity: u128| !graph_key(steps@, identity)).len()
                }) + schedule@.take(original as int).filter(
                    |identity: u128| graph_key(steps@, identity) == (group == 1)).len(),
            decreases schedule.len() - original,
        {
            proof {
                filter_scan_step(schedule@,
                    |identity: u128| graph_key(steps@, identity) == (group == 1),
                    original as int);
            }
            if graph_identity(steps, schedule[original]) == (group == 1) {
                proof {
                    if group == 0 {
                        assert((|identity: u128| graph_key(steps@, identity) == (group == 1))
                            =~= (|identity: u128| !graph_key(steps@, identity)));
                    } else {
                        assert((|identity: u128| graph_key(steps@, identity) == (group == 1))
                            =~= (|identity: u128| graph_key(steps@, identity)));
                    }
                    let partition = schedule@.filter(|identity: u128| !graph_key(steps@, identity))
                        + schedule@.filter(|identity: u128| graph_key(steps@, identity));
                    assert(applied_index < partition.len());
                    assert(partition[applied_index as int] == schedule@[original as int]);
                }
                if applied_index >= applied.len() {
                    return false;
                }
                if applied[applied_index] != schedule[original] {
                    return false;
                }
                applied_index += 1;
            }
            original += 1;
        }
        proof {
            assert(schedule@.take(original as int) == schedule@);
            if group == 0 {
                assert((|identity: u128| graph_key(steps@, identity) == (group == 1))
                    =~= (|identity: u128| !graph_key(steps@, identity)));
            } else {
                assert((|identity: u128| graph_key(steps@, identity) == (group == 1))
                    =~= (|identity: u128| graph_key(steps@, identity)));
            }
        }
        group += 1;
    }
    proof {
        partition_length(steps@, schedule@);
        assert(applied@ =~= schedule@.filter(|identity: u128| !graph_key(steps@, identity))
            + schedule@.filter(|identity: u128| graph_key(steps@, identity)));
    }
    applied_index == applied.len()
}

/// Development ordering predicate; not yet integrated or qualified.
/// Checks membership, strict DAG edges, smallest-ready selection, and partition.
/// Its equivalence proof is development evidence; host integration remains pending.
/// It confers no authorization, durability, or protocol capability.
pub fn ordering_matches(
    steps: &[(u128, u128, bool)],
    dependencies: &[(u128, u128)],
    schedule: &[u128],
    applied: &[u128],
) -> (accepted: bool)
    ensures accepted == ordering_valid(steps@, dependencies@, schedule@, applied@),
{
    proof { partition_length(steps@, schedule@); }
    if schedule.len() != steps.len() || applied.len() != schedule.len() {
        return false;
    }
    let mut index: usize = 0;
    while index < steps.len()
        invariant
            index <= steps.len(),
            forall|i: int| 0 <= i < index ==> steps@[i].0 == steps@[i].1,
            forall|i: int, j: int| 0 <= i < j < index ==> steps@[i].0 != steps@[j].0,
            forall|i: int| 0 <= i < index ==> schedule@.contains(steps@[i].0),
        decreases steps.len() - index,
    {
        if steps[index].0 != steps[index].1 {
            return false;
        }
        let mut earlier: usize = 0;
        while earlier < index
            invariant
                earlier <= index, index < steps.len(),
                forall|i: int| 0 <= i < earlier ==> steps@[i].0 != steps@[index as int].0,
            decreases index - earlier,
        {
            if steps[earlier].0 == steps[index].0 {
                return false;
            }
            earlier += 1;
        }
        if identity_position(schedule, steps[index].0) == schedule.len() {
            return false;
        }
        index += 1;
    }
    // Equal lengths and occurrence of every distinct key establish a permutation.
    proof {
        assert(forall|i: int| 0 <= i < steps.len() ==> steps@[i].0 == steps@[i].1);
        assert(forall|i: int, j: int| 0 <= i < j < steps.len() ==> steps@[i].0 != steps@[j].0);
        assert(forall|i: int| 0 <= i < steps.len() ==> schedule@.contains(steps@[i].0));
        let keys = steps@.map_values(|step: (u128, u128, bool)| step.0);
        assert(keys.no_duplicates());
        assert forall|i: int| 0 <= i < keys.len()
            implies schedule@.contains(keys[i]) by {
            assert(keys[i] == steps@[i].0);
        }
        equal_size_membership(keys, schedule@);
        assert(schedule@.no_duplicates());
        assert forall|i: int| #![trigger schedule@[i]] 0 <= i < schedule.len()
            implies exists|j: int| 0 <= j < steps.len()
                && schedule@[i] == steps@[j].0 by {
            assert(keys.contains(schedule@[i]));
            let j = choose|j: int| 0 <= j < keys.len() && keys[j] == schedule@[i];
            assert(keys[j] == steps@[j].0);
        }
    }
    let mut edge: usize = 0;
    while edge < dependencies.len()
        invariant
            edge <= dependencies.len(),
            forall|e: int| 0 <= e < edge ==> (
                (exists|before: int, after: int| 0 <= before < after < schedule.len()
                    && schedule@[before] == dependencies@[e].0
                    && schedule@[after] == dependencies@[e].1)
                && !(graph_key(steps@, dependencies@[e].0)
                    && !graph_key(steps@, dependencies@[e].1))),
        decreases dependencies.len() - edge,
    {
        let before = identity_position(schedule, dependencies[edge].0);
        let after = identity_position(schedule, dependencies[edge].1);
        if before >= after || after == schedule.len() {
            return false;
        }
        if graph_identity(steps, dependencies[edge].0)
            && !graph_identity(steps, dependencies[edge].1)
        {
            return false;
        }
        edge += 1;
    }
    proof {
        assert(forall|e: int| 0 <= e < dependencies.len() ==> (
            (exists|before: int, after: int| 0 <= before < after < schedule.len()
                && schedule@[before] == dependencies@[e].0
                && schedule@[after] == dependencies@[e].1)
            && !(graph_key(steps@, dependencies@[e].0)
                && !graph_key(steps@, dependencies@[e].1))));
    }
    if !smallest_ready_matches(dependencies, schedule) {
        return false;
    }
    if !stable_partition_matches(steps, schedule, applied) {
        return false;
    }
    proof {
        step_reverse_membership(steps@, schedule@);
        ordering_intro(steps@, dependencies@, schedule@, applied@);
    }
    true
}

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
