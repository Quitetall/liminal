//! The two semantic primitives — Node and Relation — plus the transaction
//! vocabulary and the Phase -1 toy durable store.
//!
//! Law 1: only Nodes and Relations are semantically fundamental. Everything
//! else in the workspace is a derived abstraction (v4 §6).
//!
//! The `store` module is the hand-rolled §92 checksummed-log store (ADR-0007):
//! explicitly throwaway, maximally observable, and the transactional
//! coordinator for the Intent-Logged Repair Protocol (v4 §92: "The graph store
//! acts as coordinator because it can transactionally retain intent and
//! progress").
//!
//! Spec: v4 §4 (Node), §5 (Relation), §43–45 (physical layout, RESERVED),
//! §86 (transactions and operations), §92 (crash consistency).

mod node;
mod op;
pub mod physical;
mod relation;
pub mod store;

pub use node::{Node, NodeFlags, PayloadRef};
pub use op::{Operation, Origin, Transaction, TxnMeta};
pub use relation::{AnchorRef, IdentityRequirement, Relation, RelationFlags, Target};
pub use store::ns;
pub use store::{AuxWrite, GraphStore, GraphTxn, StoreError};
