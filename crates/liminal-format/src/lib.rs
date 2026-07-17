//! Deterministic source formatting — the engine behind `lim fmt`.
//!
//! Liminal ships a built-in Ruff-like toolchain; `lim fmt` is its
//! deterministic source formatter (v4 §20). Two formatter laws are
//! constitutional (v4 §20, restated as correctness properties in §112):
//!
//! 1. **Formatting is idempotent** — formatting already-formatted source
//!    changes nothing; as a §112 law, `parse(format(parse(x))) ≡ parse(x)`.
//! 2. **Parsing canonical formatting recovers the same semantic graph** —
//!    the formatter may normalize spelling, never meaning.
//!
//! **RESERVED — implementation begins Phase 1** (v4 Part XXII "formatter,
//! `lim fmt`, `lim check`"). Law 14 forbids a production formatter (and the
//! parser it needs) until the Phase -1 falsification gates pass; formatter
//! fuzzing (v4 §113) arrives with it.
//!
//! This crate exists now so the constitutional vocabulary has a compiled,
//! greppable home: the [`Formatter`] seam lets the §112 idempotence law
//! compile as a generic conformance test today, before any implementation.

/// The formatter seam over an opaque parsed document (v4 §20).
///
/// SHAPE PROVISIONAL — exists only so the §112 formatter laws compile as
/// generic conformance tests now; Phase 1 may reshape it freely (in
/// particular, `parse` here is a formatting-front-end convenience and will be
/// reconciled with the real CST pipeline in `liminal-cst`).
///
/// Laws (v4 §20, §112):
/// - `format` is idempotent: `format(format(s)?)? == format(s)?`.
/// - Parsing canonical formatting recovers the same semantic graph:
///   `parse(format(parse(x))) ≡ parse(x)`. `Doc` intentionally carries no
///   `PartialEq` bound yet — Phase 1 decides whether document equality is
///   structural or graph-semantic (v4 §119 grammar ADR).
pub trait Formatter {
    /// Opaque parsed-document handle.
    type Doc;
    /// Formatter failure; malformed source must degrade, never panic
    /// (v4 §113 "malformed source recovery tests").
    type Error;

    /// Parse source text into a document.
    fn parse(&self, source: &str) -> Result<Self::Doc, Self::Error>;

    /// Emit canonical text for a document.
    fn emit(&self, doc: &Self::Doc) -> Result<String, Self::Error>;

    /// Format source text: canonically re-emit it (`emit(parse(source))`).
    ///
    /// Must be idempotent (v4 §20).
    fn format(&self, source: &str) -> Result<String, Self::Error>;
}
