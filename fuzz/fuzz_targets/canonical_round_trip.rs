#![no_main]

use libfuzzer_sys::fuzz_target;
use liminal_format::{Formatter, MarkdownFormatter};

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let formatter = MarkdownFormatter::default();
    let document = formatter.parse(&source).expect("parser is total");
    let emitted = formatter.emit(&document).expect("emitter is total");
    let reparsed = formatter.parse(&emitted).expect("parser is total");
    assert_eq!(reparsed, document);
    assert_eq!(
        formatter.emit(&reparsed).expect("emitter is total"),
        emitted
    );
});
