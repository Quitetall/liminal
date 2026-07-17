//! Subcommand implementations. Every command that prints must respect the
//! silence law: sound state → zero bytes (Law 3E).

pub(crate) mod check;
pub(crate) mod jurisdiction;
pub(crate) mod overlays;
pub(crate) mod repair_undo;
pub(crate) mod repairs;
