#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use liminal_id::TransactionId;
use liminal_query::{IncrementalCompiler, ParagraphCompiler, SourceEdit};
use liminal_revision::{BasisPerspective, WorkspaceBasis};

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data).into_owned();
    let midpoint = source.floor_char_boundary(source.len() / 2);
    let edit = SourceEdit {
        start: midpoint,
        end: midpoint,
        replacement: "\n".to_owned(),
    };
    let basis = WorkspaceBasis {
        transaction: TransactionId::new(),
        perspective: BasisPerspective::DurableOnly,
        components: BTreeMap::new(),
    };
    let compiler = ParagraphCompiler;
    let incremental = compiler.incremental(&source, &[edit.clone()], &basis);
    let edited = compiler.apply(&source, &[edit]);
    assert_eq!(incremental, compiler.full(&edited, &basis));
});
