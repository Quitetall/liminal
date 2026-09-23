# Third-party notices

Liminal's own source code and documentation are licensed under the Apache
License 2.0 ([`LICENSE`](LICENSE), ADR-0001).

**The exception is eight corpus files.** They contain text taken from other
projects and remain under those projects' licenses. This repository does not
relicense them under Apache-2.0.

```
conformance/corpora/heldout/v1/import-*.external-file.trace.ndjson.zst
```

None of these files is compiled into any Liminal crate. No crate in this
workspace is published (`publish = false`).

## What was changed

The Creative Commons licenses below require a statement of changes. Every file
was produced the same way, by `tracegen import` (M11, AM-11.5):

- It walks the upstream repository's first-parent git history, oldest first,
  optionally restricted to selected file extensions.
- It records the base commit's files as the trace's starting files, and each
  later commit's changed files as events in Liminal's trace NDJSON format.
- It omits files that are not valid UTF-8 and records how many it omitted.
- It records deleted files by path only.
- The result is zstd-compressed.

Provenance for each file is recorded in the `source` field of its trace
header. The text content is otherwise unmodified.

## Files under Creative Commons licenses

### `import-hott-book.external-file.trace.ndjson.zst`

- **Work:** *Homotopy Type Theory: Univalent Foundations of Mathematics*, by
  The Univalent Foundations Program, Institute for Advanced Study.
- **Source:** <https://github.com/HoTT/book>
- **License:** [Creative Commons Attribution-ShareAlike 3.0 Unported](https://creativecommons.org/licenses/by-sa/3.0/)
  (CC BY-SA 3.0).
- **Changes:** converted as described above.

**This file is itself licensed under CC BY-SA 3.0.** The ShareAlike term
requires it. The rest of the repository is unaffected.

### `import-tldr-pages.external-file.trace.ndjson.zst`

- **Work:** tldr-pages. Copyright © 2014–present the
  [tldr-pages team](https://github.com/orgs/tldr-pages/people) and
  [contributors](https://github.com/tldr-pages/tldr/graphs/contributors).
- **Source:** <https://github.com/tldr-pages/tldr>
- **License:** [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/)
  (CC BY 4.0). Upstream's `scripts/` directory is under the MIT License.
  See [`LICENSES/tldr-pages-LICENSE.md`](LICENSES/tldr-pages-LICENSE.md).
- **Changes:** converted as described above.

**This file is licensed under CC BY 4.0.** Any upstream `scripts/` content it
contains stays under the MIT License.

## Files under permissive software licenses

Each upstream offers a choice of licenses. For each one, Liminal chooses the
license named below and includes its notice, as that license requires.

| File | Source | License chosen | Notice |
|---|---|---|---|
| `import-rust-book…` | <https://github.com/rust-lang/book> | MIT (upstream: MIT OR Apache-2.0) | [`LICENSES/MIT-rust-lang-book.txt`](LICENSES/MIT-rust-lang-book.txt) |
| `import-rust-rfcs…` | <https://github.com/rust-lang/rfcs> | MIT (upstream: MIT OR Apache-2.0) | [`LICENSES/MIT-rust-lang-rfcs.txt`](LICENSES/MIT-rust-lang-rfcs.txt) |
| `import-serde…` | <https://github.com/serde-rs/serde> | MIT (upstream: MIT OR Apache-2.0) | [`LICENSES/MIT-serde.txt`](LICENSES/MIT-serde.txt) |
| `import-ripgrep…` | <https://github.com/BurntSushi/ripgrep> | MIT (upstream: Unlicense OR MIT) | [`LICENSES/MIT-ripgrep.txt`](LICENSES/MIT-ripgrep.txt); Unlicense in [`LICENSES/UNLICENSE-ripgrep.txt`](LICENSES/UNLICENSE-ripgrep.txt) |

Copyright holders, as stated upstream:

- rust-lang/book: Copyright (c) 2010 The Rust Project Developers.
- ripgrep: Copyright (c) 2015 Andrew Gallant.
- rust-lang/rfcs and serde: the upstream MIT files carry no copyright line.
  The notices are included as published.

## Public-domain files

| File | Work | Source |
|---|---|---|
| `import-shakespeare-hamlet…` | *Hamlet*, William Shakespeare. Standard Ebooks edition | <https://github.com/standardebooks/william-shakespeare_hamlet> |
| `import-frankenstein…` | *Frankenstein*, Mary Shelley. Standard Ebooks edition | <https://github.com/standardebooks/mary-shelley_frankenstein> |

Standard Ebooks states that the source texts are believed to be in the United
States public domain. It dedicates its own contributions to the public domain
under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/). It also
notes that the texts may still be copyrighted in other countries. Its statement
is reproduced in [`LICENSES/standard-ebooks-LICENSE.md`](LICENSES/standard-ebooks-LICENSE.md).
No notice is required.

## How this list was verified

Each license above was read from the upstream repository on 2026-09-23. The
corpus files were not opened: the held-out corpus is locked by M11's
corpus-lock ceremony. The upstream of each file is named in the commit that
added it and in the file's own header.
