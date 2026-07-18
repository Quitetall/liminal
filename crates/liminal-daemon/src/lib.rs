//! The Phase -1 toy harness (R4 §10).
//!
//! NOT a real daemon: no sockets, no watchers, no warm state — the `liminald`
//! binary runs one scripted scenario (or recovery) and exits. Its whole
//! purpose is to be KILLED at ILRP durable boundaries and prove recovery
//! (R4 §10: "the daemon is terminated after every ILRP durable boundary").
//! The persistent socket daemon is Phase 2 (v4 §56), which will reuse the same
//! boundary names for a SIGKILL-handshake harness.

pub mod crash;
pub mod digest;
pub mod executor;
pub mod render;
pub mod runner;
pub mod scenario;
pub mod session;
pub mod workspace;

pub use crash::EnvCrashInjector;
pub use executor::FsExecutor;
pub use session::ClientSession;
pub use workspace::{SaveError, ToyWorkspace, WorkspaceError};
