#![no_main]

use libfuzzer_sys::fuzz_target;
use liminal_format::{Formatter, MarkdownFormatter};

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let formatter = MarkdownFormatter::default();
    let once = formatter.format(&source).expect("formatter is total");
    let twice = formatter.format(&once).expect("formatter is total");
    assert_eq!(twice, once);
});
