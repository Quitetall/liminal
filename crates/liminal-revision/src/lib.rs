//! Workspace Basis capture, Basis Perspectives, and component-granular
//! invalidation.
//!
//! Law 3D: one immutable Workspace Basis per computation. Law 3J: independent
//! dirty buffers never form a chimeric Basis.
//!
//! Spec: v4 §7.5 (WorkspaceBasis, BasisComponent, BasisPerspective, resolution
//! rules), §24.1 (durability classes); R4 §8 (Basis Perspectives).

mod basis;
mod deps;
mod inputs;
mod perspective;

pub use basis::{BasisComponent, CausalFrontier, WorkspaceBasis};
pub use deps::ComponentDeps;
pub use inputs::{AvailableInputs, PerspectiveError, resolve};
pub use perspective::BasisPerspective;
