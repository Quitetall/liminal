#![no_main]

use libfuzzer_sys::fuzz_target;
use liminal_format::MarkdownRenderer;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let renderer = MarkdownRenderer;
    let first = renderer.render(&source).expect("renderer is total");
    let second = renderer.render(&source).expect("renderer is total");
    assert_eq!(first, second);
});
