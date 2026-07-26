//! The §113/§114 test classes, one module per class. Real where the API
//! exists today; phase-tagged `#[ignore]` where it does not. The class list
//! is the spec's, not ours — do not remove a module because it is far away.

mod ai_operations;
mod conversion_loss;
mod fuzz_regressions;
mod golden_render;
mod large_workspace;
mod malformed_source;
mod migration;
mod object_corruption;
mod plugin_capability;
mod sync_partition;
mod trace_replay;
