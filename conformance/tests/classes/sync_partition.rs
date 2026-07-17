//! §113 sync-simulation class: partition/reconnect over the DECLARED merge
//! model (v4 §88; Phase 6 exit gate).

/// Two offline replicas edit supported profiles, reconnect, converge, and
/// retain auditable history (Phase 6 exit gate).
#[test]
#[ignore = "Phase 6: replica sync over an evaluated merge engine"]
fn partitioned_replicas_converge_with_auditable_history() {
    unimplemented!("simulate partition; reconnect; assert convergence + history")
}

/// Unsupported cross-profile merges fail VISIBLY rather than pretending to be
/// safe (Phase 6 exit gate).
#[test]
#[ignore = "Phase 6: cross-profile merge refusal"]
fn unsupported_cross_profile_merge_fails_visibly() {
    unimplemented!("attempt unsupported merge; assert loud refusal, zero silent damage")
}
