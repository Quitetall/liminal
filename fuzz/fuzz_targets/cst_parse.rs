#![no_main]

use libfuzzer_sys::fuzz_target;
use liminal_id::{ContentHash, SourceId};
use liminal_source::{SourceBasis, Utf8HolderView};

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let basis = SourceBasis {
        source: SourceId::new(),
        content_hash: ContentHash::of(data),
    };
    let holder = Utf8HolderView::from_bytes(basis, data).expect("validated UTF-8 holder");
    let document = liminal_cst::parse(&holder);
    assert_eq!(document.emit_lossless(), source);
});
