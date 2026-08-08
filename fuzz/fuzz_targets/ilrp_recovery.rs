//! HAQP-1 fuzz target for the **repair/ILRP/recovery** family
//! (ADR-0020 §4; M17.5 F-29).
//!
//! F-29 found this family declared a 30-minute campaign in the packet with no
//! fuzz target behind it.
//!
//! The invariant under test is R4 §10 / v4 §7.8: an intent driven only through
//! LEGAL transitions must always be able to reach a terminal state
//! (`Committed`, `NeedsReview`, `Aborted`), and a terminal state must never
//! transition onward. A state machine that can strand an intent in a
//! non-terminal state has lost a repair in progress, which is the specific
//! failure ILRP exists to make impossible — the world was touched and nothing
//! records how far.

#![no_main]

use libfuzzer_sys::fuzz_target;
use liminal_jurisdiction::ilrp::IntentState;

const STATES: [IntentState; 7] = [
    IntentState::Prepared,
    IntentState::Applying,
    IntentState::ExternalApplied,
    IntentState::Finalizing,
    IntentState::Committed,
    IntentState::NeedsReview,
    IntentState::Aborted,
];

/// Can `from` reach ANY terminal state by legal transitions? Breadth-first over
/// a 7-state machine, computed here rather than asked of the implementation —
/// ADR-0020 §5 forbids proving a relation with the subject's own accounting.
fn can_reach_terminal(from: IntentState) -> bool {
    let mut seen = vec![from];
    let mut frontier = vec![from];
    while let Some(state) = frontier.pop() {
        if state.is_terminal() {
            return true;
        }
        for next in STATES {
            if state.may_transition_to(next) && !seen.contains(&next) {
                seen.push(next);
                frontier.push(next);
            }
        }
    }
    false
}

fuzz_target!(|data: &[u8]| {
    // Walk the machine under fuzzer control, taking only legal steps.
    let mut state = IntentState::Prepared;
    for byte in data {
        let candidate = STATES[usize::from(byte % 7)];
        if state.may_transition_to(candidate) {
            state = candidate;
        }

        // A terminal state is terminal: nothing may lead out of it. Otherwise a
        // committed repair could be re-driven and its external effects repeated.
        if state.is_terminal() {
            for next in STATES {
                assert!(
                    !state.may_transition_to(next),
                    "{state:?} is terminal but may transition to {next:?}"
                );
            }
            break;
        }

        // ...and from anywhere reachable, a terminal state must remain
        // reachable. An intent that cannot finish has stranded a repair whose
        // external effects already happened.
        assert!(
            can_reach_terminal(state),
            "{state:?} is reachable but cannot reach any terminal state"
        );
    }
});
