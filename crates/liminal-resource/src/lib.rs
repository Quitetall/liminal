//! Content-addressed object store for large binary payloads (v4 §47).
//!
//! RESERVED — implementation begins Phase 4 (v4 Part XXII). The eventual
//! shape is `Node → content hash → chunked object`: large payloads
//! (images, PDFs, audio, video) live outside the graph store, addressed by
//! `ContentHash` (blake3, v4 §19.3/§47), giving deduplication, integrity
//! verification, incremental synchronization, immutable caching,
//! reproducible outputs, and shared thumbnails/derivatives (v4 §47).
//!
//! What this crate deliberately does NOT do yet: Phase -1 (Law 14, v4
//! Part XXII) needs no chunking, no dedup, and no derivative pipeline —
//! the toy store in `liminal-graph` keeps everything inline, and
//! `PayloadRef::Object` / `ObjectId` exist only as vocabulary. The one
//! §47 property Phase -1 does exercise lives in the conformance suite:
//! corruption of stored bytes must be DETECTED, never silently served
//! (v4 §92), which the toy store's checksummed log already demonstrates.
//!
//! Related crates: `liminal-id` (`ContentHash`, `ObjectId`),
//! `liminal-graph` (`PayloadRef::Object`), `liminal-sync` (incremental
//! object transfer, v4 §88).
