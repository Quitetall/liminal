//! HAQP-1 fuzz target for the **graph/interchange codecs** family
//! (ADR-0020 §4; M17.5 F-29).
//!
//! F-29 found this family declared a 30-minute campaign in the packet with no
//! fuzz target behind it. The interchange codec is where hostile or corrupt
//! bytes first meet the graph vocabulary, so a decoder that panics — or that
//! decodes to something it will not re-encode — is a defect the family's
//! budget was supposed to be looking for.
//!
//! The property is **decode/encode round-trip stability**, not merely totality.
//! Totality alone is satisfied by a decoder that rejects everything; a decoder
//! that accepts a value it cannot faithfully re-emit is the interesting bug,
//! because it silently rewrites a document on save.

#![no_main]

use libfuzzer_sys::fuzz_target;
use liminal_graph::Transaction;

fuzz_target!(|data: &[u8]| {
    // Decoding must be TOTAL: hostile bytes produce Err, never a panic.
    let Ok(decoded) = serde_json::from_slice::<Transaction>(data) else {
        return;
    };

    // Anything accepted must survive a round trip byte-for-byte. An encoder
    // that cannot reproduce what it just decoded has changed the caller's data.
    let encoded = serde_json::to_vec(&decoded).expect("a decoded transaction must re-encode");
    let again: Transaction =
        serde_json::from_slice(&encoded).expect("re-encoded bytes must decode again");
    assert_eq!(
        decoded, again,
        "decode(encode(decode(x))) diverged from decode(x)"
    );

    // Encoding must be deterministic, or no golden over this codec can hold
    // and no digest of it means anything.
    let re_encoded = serde_json::to_vec(&again).expect("re-encode must be total");
    assert_eq!(encoded, re_encoded, "encoding the same value twice differed");
});
