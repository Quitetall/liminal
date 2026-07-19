//! M10 exit-gate born-passing tests: the D10.3 pandoc version pin and (at
//! M10.4) the declared-level recomputation.

use liminal_conformance::pandoc;

/// D10.3: the pinned toolchain is present and exact — `pandoc --version`
/// first line equals `pandoc 3.6.1`. A missing or mismatched pandoc PANICS
/// (no silent skip, no `#[ignore]`): a skipped loss report would fake Law 8.
#[test]
fn pandoc_version_pinned() {
    pandoc::assert_pandoc_pinned();
}
