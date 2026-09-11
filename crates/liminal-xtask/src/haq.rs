//! HAQP-1 packet verification.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use liminal_format::Formatter;
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Re-attest concurrency evidence at the current base (ADR-0020 §4).
///
/// Nothing produced this artifact. `haqp_qualify.sh` had no concurrency stage
/// and the justfile no recipe, so `concurrency.json` was hand-written once in
/// August and every later lane inherited its stale `source_commit` -- the flip
/// refused the 2026-09-01 campaign partly on this file. A required artifact
/// with no producer cannot survive a rule that says evidence must describe ONE
/// fixed tree.
///
/// The record is built from the packet and the closed constants
/// `verify_concurrency_evidence_record` checks against, not by re-stamping the
/// previous output: regenerating a file from itself would carry a wrong
/// declaration forward untouched, which is the failure mode this replaces.
pub fn run_concurrency_repo(root: &Utf8Path) -> Result<()> {
    let packet = read_packet(root)?;
    anyhow::ensure!(
        packet.concurrent_code == "not_applicable",
        "concurrency generation covers the not-applicable declaration only; \
         packet declares {:?} and an executed schedule campaign must record its \
         own runs",
        packet.concurrent_code
    );
    let commit = git_text(root, &["rev-parse", "HEAD"])?;
    let tree = git_text(root, &["rev-parse", "HEAD^{tree}"])?;
    let evidence = ConcurrentEvidence {
        schema_version: "haqp-concurrency-v1".to_owned(),
        implementation: packet.concurrent_code.clone(),
        mode: "not_applicable".to_owned(),
        reason: "no concurrent implementation exists; deterministic schedule \
                 exploration is reserved for Phase 6"
            .to_owned(),
        source_anchor: "docs/execution/M21.md:13".to_owned(),
        runner: String::new(),
        source_commit: commit,
        source_tree: tree,
        oracle_id: "not_applicable".to_owned(),
        schedules: Vec::new(),
        sync_primitives: scan_concurrency_primitives(root)?.sync,
    };
    // Fail here rather than at the gate: the record is checked by the same code
    // that will judge it, so a generator that drifts from the verifier is
    // caught when it is written, not after a four-hour lane.
    verify_concurrency_evidence_record(root, &packet, &evidence)?;
    let path = root.join("conformance/haqp/evidence/concurrency.json");
    fs::write(&path, serde_json::to_vec_pretty(&evidence)?)?;
    println!("concurrency evidence written to {path}");
    Ok(())
}

/// Digests the lane records for a captured scope trace, computed by the same
/// code the gate later judges them with. `kind` is `paths` (the observed
/// open-path set) or `resolved` (traced corpus paths resolved inside the tree).
pub fn scope_trace_digest_repo(
    root: &Utf8Path,
    kind: &str,
    traces: &[Utf8PathBuf],
    scope: &str,
) -> Result<String> {
    anyhow::ensure!(!traces.is_empty(), "scope digest needs at least one trace");
    let label = format!("{scope} corpus scope");
    // The union of open paths: sets, so carving a fuzz binary's lines out of
    // the stage trace into its own part changes nothing the gate judges (F-43).
    let mut open = BTreeSet::new();
    for trace in traces {
        open.extend(scan_scope_trace_file(trace, &label)?.open_paths);
    }
    match kind {
        "paths" => Ok(scope_trace_paths_digest_from(&open)),
        "resolved" => {
            let access = if scope == "fuzz" {
                CorpusAccess::Required
            } else {
                CorpusAccess::IfPresent
            };
            resolved_corpus_paths_digest_from(
                root,
                root.as_str(),
                &open,
                &label,
                access,
                LockedCorpus::WritesOnly,
            )
        }
        other => anyhow::bail!("unknown scope digest kind {other:?}; expected paths or resolved"),
    }
}

/// The digest `verify_markdown_surface` requires the review markdown to carry.
pub fn packet_digest_repo(root: &Utf8Path) -> Result<String> {
    packet_digest(&read_packet(root)?)
}

/// Verify committed HAQP inventories, packet status, and crash-boundary registry.
/// Generated-case oracles that must not call the production path they judge
/// (ADR-0020 §5). Closed list: (file, function, forbidden symbols). Blind pass 1
/// at 78c8f9b (A02): the comment on `incremental_paragraph_oracle` said it does
/// not call `paragraph::parse`, and nothing enforced it.
/// Blind pass 1 at ab5e109a (A02): this held the query oracle alone while the
/// packet declared five generated oracles besides it, so four of the six ran
/// with no independence scan at all. Each forbidden list names the production
/// entry points its own case builder exercises — the paths the oracle judges
/// and must not re-run.
const INDEPENDENT_ORACLES: [(&str, &str, &[&str]); 6] = [
    (
        "crates/liminal-xtask/src/haq.rs",
        "independent_oracle_source_cst",
        &[
            "liminal_cst::parse",
            "liminal_cst::coarse_parse",
            "MarkdownFormatter",
            ".format(",
        ],
    ),
    (
        "crates/liminal-xtask/src/haq.rs",
        // The codec under test IS a serde round trip, so decoding is how this
        // oracle reads anything at all; forbidding that would assert something
        // untrue about the design. What it must not do is rebuild the expected
        // value from the builder that produced the case.
        "independent_oracle_interchange",
        &["interchange_transaction("],
    ),
    (
        "crates/liminal-xtask/src/haq.rs",
        "independent_oracle_source_transform",
        &["merge::three_way", "three_way("],
    ),
    (
        "crates/liminal-xtask/src/haq.rs",
        "independent_oracle_source_repair",
        &["liminal_jurisdiction::"],
    ),
    (
        "crates/liminal-xtask/src/haq.rs",
        // `invalidated_by` is what this oracle observes, not what couples it:
        // a metamorphic oracle for an invalidation property has to ask the
        // query. The coupling would be deriving the EXPECTED set from
        // production, and that set arrives as a parameter.
        "independent_oracle_source_invalidation",
        &["graph_key("],
    ),
    (
        "crates/liminal-query/src/lib.rs",
        "incremental_paragraph_oracle",
        &[
            "paragraph::parse",
            "ParagraphCompiler",
            "liminal_source::paragraph",
            "IncrementalCompiler",
        ],
    ),
];

/// The body of `name` in `text`: from its `fn` line to the first line that is a
/// bare `}` at column zero — under rustfmt only a top-level item closes there,
/// so nested blocks cannot end the scan early.
fn item_body<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let start = text.find(&format!("fn {name}("))?;
    let rest = &text[start..];
    let end = rest.find("\n}\n").map_or(rest.len(), |index| index + 2);
    Some(&rest[..end])
}

/// Every definition of `name` in `text`. All of them, not the first: a crate's
/// modules concatenate here, and two modules may name a function alike.
fn item_bodies<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    // Blind pass 1 at `0a0b4b4b` (A02): a macro is an item the oracle can
    // reach, and its body is where the production call would sit — following
    // functions alone left `production!()` outside the scanned body.
    let needles = [format!("fn {name}("), format!("macro_rules! {name}")];
    let mut out = Vec::new();
    for needle in &needles {
        let mut from = 0;
        while let Some(offset) = text[from..].find(needle.as_str()) {
            let start = from + offset;
            let rest = &text[start..];
            let end = rest.find("\n}\n").map_or(rest.len(), |index| index + 2);
            out.push(&rest[..end]);
            from = start + needle.len();
        }
    }
    out
}

/// The crate's own sources, concatenated, for the crate owning `file`. Blind
/// pass 1 at c29bc0ea (A02): the closure followed same-file functions only, so
/// a helper one module away carried the oracle to the production path it
/// judges and named nothing.
fn crate_sources(root: &Utf8Path, file: &str) -> Result<String> {
    let path = root.join(file);
    let Some(src) = path.parent() else {
        return fs::read_to_string(&path).with_context(|| format!("read {path}"));
    };
    let mut stack = vec![src.to_owned()];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        let mut entries = fs::read_dir(dir.as_std_path())
            .with_context(|| format!("read {dir}"))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        for entry in entries {
            let entry = Utf8PathBuf::from_path_buf(entry)
                .map_err(|p| anyhow::anyhow!("non-UTF-8 path {}", p.display()))?;
            if entry.is_dir() {
                stack.push(entry);
            } else if entry.extension() == Some("rs") {
                files.push(entry);
            }
        }
    }
    files.sort();
    let mut combined = String::new();
    for file in files {
        combined.push_str(&fs::read_to_string(&file).with_context(|| format!("read {file}"))?);
        combined.push('\n');
    }
    Ok(combined)
}

/// Every function in `text` reachable from `name`, concatenated. A helper the
/// oracle calls is part of the oracle for ADR-0020 §5: independence that one
/// hop defeats is not independence.
fn oracle_reachable_body(text: &str, name: &str) -> String {
    let defined = text
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            // Review of f4d42d3: `pub(crate) fn helper` named no prefix, so a
            // helper declared that way was missing from the closure and the
            // oracle could reach production through it. Review of 4bc26b6d:
            // the first fix tested `pub(...)` as a whole-prefix alternative,
            // which `pub(crate) async fn` fails on both arms. Review of
            // `39de4daa`: a per-word check still missed `pub(in crate::x) fn`,
            // which splits into two words. The restriction is removed as a
            // span, then the remaining qualifiers are checked word by word.
            // `unsafe` is among them because adding a helper to the closure
            // can only add refusals; dropping one is the direction that lets
            // an oracle reach production unnoticed.
            if let Some(rest) = trimmed.strip_prefix("macro_rules! ") {
                let end = rest.find(['{', '(', '[']).unwrap_or(rest.len());
                return Some(rest[..end].trim().to_owned());
            }
            let rest = trimmed
                .split_once("fn ")
                .filter(|(before, _)| {
                    let mut qualifiers = (*before).to_owned();
                    while let Some(start) = qualifiers.find("pub(") {
                        let Some(offset) = qualifiers[start..].find(')') else {
                            break;
                        };
                        qualifiers.replace_range(start..=start + offset, "");
                    }
                    qualifiers
                        .split_whitespace()
                        .all(|word| matches!(word, "pub" | "async" | "const" | "unsafe"))
                })
                .map(|(_, rest)| rest)?;
            let end = rest.find(['(', '<'])?;
            Some(rest[..end].to_owned())
        })
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut queue = vec![name.to_owned()];
    let mut combined = String::new();
    while let Some(current) = queue.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }
        for body in item_bodies(text, &current) {
            combined.push_str(body);
            combined.push('\n');
            for callee in &defined {
                // Blind pass 1 at `6b36bbb9` (A01): only the CALL form was
                // followed, so `let h = helper; h(input)` carried the oracle
                // to a body the closure never entered — the same value
                // indirection that defeated the alias scan one lane earlier.
                // Naming a function at all pulls its body in.
                if !seen.contains(callee) && mentions_word(body, callee) {
                    queue.push(callee.clone());
                }
            }
        }
    }
    combined
}

/// Whether `text` uses `word` as a whole identifier. `contains` would match a
/// one-letter alias inside every other identifier in the file.
fn mentions_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        let boundary = |ch: Option<char>| !ch.is_some_and(|c| c.is_alphanumeric() || c == '_');
        boundary(before) && boundary(after)
    })
}

fn verify_oracle_independence(root: &Utf8Path) -> Result<()> {
    for (file, function, forbidden) in INDEPENDENT_ORACLES {
        let path = root.join(file);
        anyhow::ensure!(
            path.is_file(),
            "{file}: independent oracle {function} has no source file"
        );
        // The scanned body spans the crate, so the aliases must too: a `use`
        // that renames a forbidden symbol in another module renames it for
        // code that is now inside the body being judged.
        let text = crate_sources(root, file)?;
        // Blind pass 1 at aecb2ec7 (A10): only the oracle's own body was
        // scanned, so `fn helper(x) { paragraph::parse(x) }` called from the
        // oracle reached the production path through one hop and named
        // nothing. The body is the transitive closure now: the oracle plus
        // every function defined in the same file that it reaches.
        anyhow::ensure!(
            item_body(&text, function).is_some(),
            "{file}: independent oracle {function} is not defined"
        );
        let body = oracle_reachable_body(&text, function);
        // Blind pass 1 at aa00d41 (A02): the scan matched literal paths, so
        // `use liminal_source::paragraph::parse as p; p(input)` named none of
        // them. A `use` that renames a forbidden symbol makes its alias
        // forbidden too, in this file.
        let mut watched_words: Vec<String> = Vec::new();
        let mut watched = forbidden
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>();
        for line in text.lines() {
            for (symbol, alias) in use_aliases(line) {
                if forbidden.iter().any(|watch| {
                    watch.contains(&symbol) || symbol.contains(watch.trim_end_matches('('))
                }) {
                    // Blind pass 1 at 5fb1b57 (A02): only the call form was
                    // watched, so `use ...::paragraph as prod;` followed by
                    // `prod::parse(input)` named nothing. A module alias is
                    // used with `::`, a function alias with `(`.
                    watched.push(format!("{alias}("));
                    watched.push(format!("{alias}::"));
                    // Blind pass 1 at `aeed70f9` (A02): only the call and
                    // module forms were watched, so `let q = p; q(input)`
                    // reached the production path under a name the scan had
                    // never heard of. The bare alias is watched as a WORD, so
                    // handing it to anything at all is the signal.
                    watched_words.push(alias);
                }
            }
        }
        for symbol in &watched {
            anyhow::ensure!(
                !body.contains(symbol.as_str()),
                "{file}: oracle {function} reaches the production path it judges ({symbol}); \
                 ADR-0020 §5 requires an independent oracle"
            );
        }
        for word in &watched_words {
            anyhow::ensure!(
                !mentions_word(&body, word),
                "{file}: oracle {function} names the aliased production path it judges \
                 ({word}); ADR-0020 §5 requires an independent oracle"
            );
        }
    }
    Ok(())
}

/// Blind pass 1 at 78c8f9b (A09): the markdown verifier checked headers, the
/// status tuple and the packet digest, never the table cells the flip renders
/// from artifacts. Each pass block's rows are re-derived from the committed
/// record and compared; an unqualified packet's rows must all be NOT_RUN.
/// The eleven cells the flip renders for one pass block, derived from its
/// record exactly as `haqp_flip_packet.py` derives them.
fn expected_review_rows(record: &ReviewRecord) -> [(&'static str, String); 11] {
    let caught = record
        .attempts
        .iter()
        .filter(|a| a.classification == "caught_violation")
        .count();
    [
        (
            "reviewer identity hash",
            record.reviewer.identity_hash.clone(),
        ),
        (
            "reviewer kind/model family",
            format!(
                "{} via {}",
                record.reviewer.model_family, record.reviewer.backend
            ),
        ),
        (
            "isolated session hash",
            record.isolated_session_hash.clone(),
        ),
        (
            "sanitized prompt hash",
            record.sanitized_prompt_hash.clone(),
        ),
        ("fixed-base hash", record.fixed_base.commit.clone()),
        (
            "blindness proof",
            format!(
                "{}; prior_pass_artifact_supplied={}",
                record.blindness_proof.session_state,
                record.blindness_proof.prior_pass_artifact_supplied
            ),
        ),
        (
            "concrete falsification attempts (minimum 12)",
            record.attempts.len().to_string(),
        ),
        ("attempted caught violations", caught.to_string()),
        (
            "findings artifact hash",
            record.integrity_binding_sha256.clone(),
        ),
        (
            "unresolved verified findings",
            record.unresolved_verified_findings.to_string(),
        ),
        ("result", record.result.clone()),
    ]
}

/// Every content table the flip renders from an artifact, by its header's
/// leading columns. Blind pass 1 at e09ae5e (A09): the gate checked headers,
/// the packet digest, the status tuple, the canary table and the review
/// blocks, and nothing else — so a human-facing cell in any of these could
/// claim a result no artifact supports while every checked thing stayed valid.
/// The activation values the review markdown's test table may use. The packet
/// carries no activation field, so this column is editorial and its vocabulary
/// is closed (A09).
/// Words the flip writes only when an artifact says so. Matched against the
/// WHOLE cell, never as a substring, so a descriptive cell that happens to
/// contain "executed" is not a claim; the tables these guard hold identifiers
/// and results, not prose. While the packet is unqualified no cell anywhere in
/// the review markdown may BE one of them —
/// the content tables are checked cell by cell above, and this catches the
/// `Field | Value` blocks the flip also fills (A09).
/// Words that claim a result. Blind pass 1 at `0a0b4b4b` (A09): `approved`,
/// `accepted`, `granted` and `signed` all assert a decision the flip has not
/// made, and none of them were here — an unqualified packet's report could
/// have said its ratification was approved.
const VERDICT_VOCABULARY: [&str; 16] = [
    "pass",
    "passed",
    "fail",
    "failed",
    "caught",
    "missed",
    "killed",
    "survived",
    "executed",
    "complete",
    "ratified",
    "authorized",
    "approved",
    "accepted",
    "granted",
    "signed",
];

const ACTIVATION_VOCABULARY: [&str; 2] = ["Phase 1", "Conditional: first persisted-format ADR"];

/// The rendered tables whose cells change when the lane runs. Blind pass 1 at
/// ab5e109a (A09): relaxing the cell rule for every table on qualification was
/// still far too much — ten of the twelve are derived from the packet and read
/// the same before and after, so only these two may hold a value the packet
/// does not declare.
const RESULT_BEARING_TABLES: [&str; 2] = [
    "| Family | Planned | Executed |",
    "| Family | Relations and results |",
];

const RENDERED_TABLES: [&str; 12] = [
    "| Requirement ID | Kind |",
    "| Test ID | Exact test |",
    "| Requirement ID | Positive tests |",
    "| Class/boundary ID | Source coordinate |",
    "| Abuse ID | Trigger/input |",
    "| Fault ID | Registered boundary |",
    "| Family | Planned | Executed |",
    "| Mutant ID | Family | Operator |",
    "| HAQP family | Accepted target |",
    "| Family | Relations and results |",
    "| Surface | Primary implementation |",
    "| Boundary ID | Registration coordinate |",
];

/// A cell that asserts nothing: the lane has not run, or the surface is
/// quarantined behind Phase 1 authorization (AM-17.2).
fn cell_claims_nothing(cell: &str) -> bool {
    let cell = cell.trim();
    // `NOT_RUN — prohibited` and `QUARANTINED (AM-17.2)` carry a reason after
    // the marker, so the marker is a prefix. A reason may not smuggle a
    // verdict past the whole-cell check below (review of `a5c27fc`).
    let annotated = (cell.starts_with("NOT_RUN ") || cell.starts_with("QUARANTINED"))
        && !cell
            .split(|ch: char| !ch.is_ascii_alphabetic())
            .any(|word| VERDICT_VOCABULARY.contains(&word.to_ascii_lowercase().as_str()));
    cell.is_empty()
        || cell == "NOT_RUN"
        || annotated
        || cell.chars().all(|ch| ch == '-' || ch == ':')
}

/// The rows of the table whose header starts with `header`, each split into
/// trimmed cells. The separator row is dropped.
/// Every table under `header`, with the width its own header line declares.
/// Blind pass 1 at `6b36bbb9` (A03): this found the FIRST header and stopped,
/// so a second table spelling the same header was never read — its rows could
/// claim anything. A document may render a header more than once, so all of
/// them are returned and each is judged against its own header's width.
fn markdown_tables<'a>(text: &'a str, header: &str) -> Vec<(usize, Vec<Vec<&'a str>>)> {
    let mut tables = Vec::new();
    for (offset, _) in text.match_indices(header) {
        let is_line_start = text[..offset]
            .chars()
            .next_back()
            .is_none_or(|previous| previous == '\n');
        if !is_line_start {
            continue;
        }
        let mut lines = text[offset..].lines();
        let width = lines
            .next()
            .map(|line| line.trim().trim_matches('|').split('|').count())
            .unwrap_or_default();
        let mut rows = Vec::new();
        for line in lines {
            if !line.starts_with('|') {
                break;
            }
            let cells = line
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            if cells
                .iter()
                .all(|cell| cell.chars().all(|ch| ch == '-' || ch == ':'))
            {
                continue;
            }
            rows.push(cells);
        }
        tables.push((width, rows));
    }
    tables
}

/// While the packet is unqualified, a rendered table may carry only what the
/// PACKET already declares — family names, predeclared counts, the declared
/// fuzz budget — and otherwise must claim nothing. The flip is the only thing
/// that may write a result, and it runs only on a qualified packet, so any
/// other value in these tables is drift (A09).
fn verify_markdown_tables(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("docs/execution/phase1-suite-review.md");
    let text = fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
    // Blind pass 1 at ee646f8c (A09): this returned here for any qualified
    // packet, on the reasoning that "the rest are bound by the packet digest
    // this markdown carries". They are not. The digest binds the packet;
    // editing a cell of this markdown leaves it untouched, so every table
    // outside the review blocks went unchecked the moment provenance existed.
    // Qualification changes exactly one thing: the flip may write results. So
    // the shapes are checked either way, and only the value rule relaxes.
    let qualified = packet.provenance.is_some();
    let mut declared = BTreeSet::new();
    for mutant in &packet.mutants {
        declared.insert(mutant.family.clone());
    }
    for row in &packet.generated {
        declared.insert(row.family.clone());
        declared.insert(row.accepted.to_string());
    }
    let mut by_family = BTreeMap::<&str, usize>::new();
    for mutant in &packet.mutants {
        *by_family.entry(mutant.family.as_str()).or_default() += 1;
    }
    for count in by_family.values() {
        declared.insert(count.to_string());
    }
    // The declared per-family fuzz budget, as the markdown spells it. ADR-0020
    // §4 fixes 30 minutes per family; the packet names the targets, and the
    // minutes are derived from the campaign, so the constant is the ADR's.
    declared.insert("30 minutes".to_owned());
    // The packet carries no `activation` field, so the markdown's activation
    // column is editorial. It is a CLOSED vocabulary rather than free text: a
    // new value is a visible decision, not drift.
    for activation in ACTIVATION_VOCABULARY {
        declared.insert((*activation).to_owned());
    }
    for requirement in &packet.requirements {
        declared.insert(requirement.id.clone());
    }
    for test in &packet.tests {
        declared.insert(test.id.clone());
        declared.insert(test.name.clone());
    }
    // No cell in ANY table may be a verdict while the packet is unqualified.
    let lines = text.lines().collect::<Vec<_>>();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        // A header row names its columns — "Executed", "Killed", "Survived"
        // are column NAMES there, not claims. A header is the row a separator
        // follows.
        let is_header = lines.get(index + 1).is_some_and(|next| {
            let next = next.trim();
            next.starts_with('|')
                && next
                    .trim_matches('|')
                    .chars()
                    .all(|ch| matches!(ch, '-' | ':' | '|' | ' '))
        });
        if is_header {
            continue;
        }
        for cell in trimmed.trim_matches('|').split('|') {
            let value = cell.trim().trim_matches('`').trim().to_ascii_lowercase();
            // A cell IS a verdict, or annotates a marker with one: both
            // `| pass |` and `| NOT_RUN — passed |` claim a result the lane
            // has not recorded (review of `a5c27fc`).
            let smuggled = (value.starts_with("not_run") || value.starts_with("quarantined"))
                && value
                    .split(|ch: char| !ch.is_ascii_alphabetic())
                    .any(|word| VERDICT_VOCABULARY.contains(&word));
            anyhow::ensure!(
                qualified || (!VERDICT_VOCABULARY.contains(&value.as_str()) && !smuggled),
                "review markdown claims {value:?} while the packet is unqualified; only the \
                 flip may write a verdict, and it runs on a qualified packet"
            );
        }
    }
    for header in RENDERED_TABLES {
        let tables = markdown_tables(&text, header);
        anyhow::ensure!(
            !tables.is_empty(),
            "review markdown has no table {header:?}"
        );
        // The HEADER's width, not the first row's: the finding was a row with
        // surplus cells beneath a narrower header, which comparing rows to
        // each other would not see. Each table brings its own.
        for (width, rows) in tables {
            for row in rows {
                // Blind pass 1 at 9eca4f0 (A09): cell VALUES were checked but not
                // how many there were, so a row could carry surplus cells beneath
                // a narrower header and read as a different claim.
                anyhow::ensure!(
                    row.len() == width,
                    "review markdown table {header:?} has a row of {} cells under a {width}-cell \
                 header",
                    row.len()
                );
                for cell in row {
                    // Cells are markdown: an identifier is spelled in backticks.
                    let value = cell.trim().trim_matches('`').trim();
                    let renders_results = qualified && RESULT_BEARING_TABLES.contains(&header);
                    anyhow::ensure!(
                        renders_results || cell_claims_nothing(value) || declared.contains(value),
                        "review markdown table {header:?} claims {value:?}, which the unqualified \
                     packet does not declare; only the flip may write a result, and it runs \
                     on a qualified packet"
                    );
                }
            }
        }
    }
    Ok(())
}

fn verify_markdown_review_blocks(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let text = fs::read_to_string(root.join("docs/execution/phase1-suite-review.md"))
        .context("read the review markdown")?;
    let fields = [
        "reviewer identity hash",
        "reviewer kind/model family",
        "isolated session hash",
        "sanitized prompt hash",
        "fixed-base hash",
        "blindness proof",
        "concrete falsification attempts (minimum 12)",
        "attempted caught violations",
        "findings artifact hash",
        "unresolved verified findings",
        "result",
    ];
    let cell = |value: &str| value.replace('|', "\\|").replace('\n', " ");
    for (index, review) in packet.reviews.iter().enumerate() {
        let marker = format!("### Pass {}", index + 1);
        let block_at = text
            .find(&marker)
            .with_context(|| format!("review markdown has no {marker:?} block"))?;
        let block = &text[block_at..];
        let block = &block[..block[1..].find("\n### ").map_or(block.len(), |i| i + 1)];
        let row_value = |field: &str| -> Result<String> {
            let prefix = format!("| {field} | ");
            let line = block
                .lines()
                .find(|line| line.starts_with(&prefix))
                .with_context(|| format!("{marker}: no row for {field:?}"))?;
            Ok(line[prefix.len()..]
                .trim_end()
                .trim_end_matches('|')
                .trim()
                .to_owned())
        };
        if packet.provenance.is_none() {
            for field in fields {
                require_eq(&format!("{marker} {field}"), &row_value(field)?, "NOT_RUN")?;
            }
            continue;
        }
        let path = review
            .evidence
            .as_deref()
            .with_context(|| format!("{marker}: qualified review row names no record"))?;
        let record_path = safe_repo_path(root, path, "review record")?;
        let record: ReviewRecord = serde_json::from_slice(&fs::read(&record_path)?)
            .with_context(|| format!("parse {record_path}"))?;
        let expected = expected_review_rows(&record);
        for (field, value) in expected {
            require_eq(
                &format!("{marker} {field}"),
                &row_value(field)?,
                &cell(&value),
            )?;
        }
    }
    Ok(())
}

/// Verify this repository's HAQP state for the named subcommand.
pub fn verify_inventory_repo(root: &Utf8Path) -> Result<()> {
    let packet = read_packet(root)?;
    verify_packet_shape(&packet)?;
    verify_requirement_sources(root, &packet)?;
    verify_packet_statuses(&packet, inventory_statuses())?;
    verify_markdown_surface(root, &packet)?;
    verify_markdown_review_blocks(root, &packet)?;
    verify_markdown_tables(root, &packet)?;
    verify_oracle_independence(root)?;
    // M17.5 F-65: the mutation plan's gates lived only in the qualified path,
    // which runs after the flip. A plan defect therefore cost a whole lane to
    // discover, and `haq verify-inventory` — the command run by hand between
    // lanes — never read the plan at all. AM-17.10's derivation and F-64's
    // operator contract are checks on the PACKET, so they belong here too.
    verify_mutant_source_coordinates(root, &packet)?;
    verify_mutant_anchors_support_operators(root, &packet)?;
    verify_mutant_killing_tests(root, &packet)?;
    // Blind pass 2 at `ec045588` (A05, A09, A12): four checks were reachable
    // only through `verify_qualified_repo`, which refuses before it starts
    // unless the packet is already qualified — so at stage 1a they had never
    // run against anything, and replacing each with `Ok(())` changed no
    // verdict. F-65 moved the mutation plan's gates here; these are the rest
    // that need no provenance, which is to say everything whose evidence is
    // already in the tree.
    verify_qualification_stage(&packet)?;
    verify_locked_corpus_has_no_aliases(root)?;
    verify_crash_terminal_states(&read_crash_evidence(root)?)?;
    verify_review_receipts_inventory(root, &packet)?;
    Ok(())
}

/// The crash evidence as committed. Split out so the terminal-state oracle can
/// run before the flip: the file is in the tree whether or not a campaign has.
fn read_crash_evidence(root: &Utf8Path) -> Result<CrashEvidence> {
    let path = root.join("conformance/haqp/evidence/crash.json");
    let bytes =
        fs::read(&path).with_context(|| format!("{path}: committed crash evidence is required"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))
}

/// Every declared review record's digests, receipt and retained transcript,
/// checked before the flip. Blind pass 2 at `ec045588` (A09): these ran only
/// on the qualified path, so the committed records' digests bound nothing a
/// clone could check.
fn verify_review_receipts_inventory(root: &Utf8Path, packet: &Packet) -> Result<()> {
    for review in &packet.reviews {
        let Some(evidence) = review.evidence.as_deref() else {
            continue;
        };
        let path = safe_repo_path(root, evidence, "review receipt")?;
        let record: ReviewRecord =
            serde_json::from_slice(&fs::read(&path).with_context(|| format!("read {path}"))?)
                .with_context(|| format!("parse {path}"))?;
        verify_review_record_digests(root, &record, &path)
            .with_context(|| format!("{}: committed review record", review.reviewer))?;
    }
    Ok(())
}

/// Verify completed HAQP qualification evidence. Planned or NOT_RUN packet rows
/// fail here; this is the command used by the M17 packet gate.
pub fn verify_qualified_repo(root: &Utf8Path) -> Result<()> {
    let packet = read_packet(root)?;
    verify_packet_shape(&packet)?;
    verify_requirement_sources(root, &packet)?;
    verify_crash_boundary_scope(root, &packet)?;
    verify_markdown_surface(root, &packet)?;
    // Blind pass 1 at 5fb1b57 (A09): both of these ran on the inventory path
    // only, so the path that judges a FLIPPED packet — the one that matters —
    // accepted review cells disagreeing with their records and an oracle
    // coupled to the production path it judges.
    verify_markdown_review_blocks(root, &packet)?;
    verify_markdown_tables(root, &packet)?;
    verify_oracle_independence(root)?;
    require_eq(
        "qualification_state",
        &packet.qualification_state,
        "complete",
    )?;
    verify_mutant_source_coordinates(root, &packet)?;
    verify_packet_statuses(
        &packet,
        PacketStatusExpectations {
            // ADR-0021 stages the mutation requirement, so the expected mutant
            // disposition is stage-dependent. Hardcoding `killed` here made
            // HAQP-1a UNREACHABLE: leave mutants `predeclared` and this gate
            // failed; mark one `killed` and verify_qualification_stage failed.
            // The first version of ADR-0021 shipped with exactly that
            // contradiction — the deadlock it existed to break, reintroduced
            // one layer down.
            mutant: if packet.qualification_stage == "1b" {
                "resolved"
            } else {
                "predeclared"
            },
            canary: "caught",
            generated: "pass",
            crash: "pass",
            review: "pass",
        },
    )?;
    // M17.5 A6 / pass-2 #14,#15,#17,#19,#21: statuses are CLAIMS. Bind them to
    // artifacts a run actually produced, or the whole gate is satisfiable by
    // editing two files.
    verify_provenance(root, &packet)?;
    verify_concurrency_evidence(root, &packet)?;
    verify_fuzz_evidence(root, &packet)?;
    verify_locked_corpus_has_no_aliases(root)?;
    verify_corpus_access_audit(root, &packet)?;
    verify_crash_evidence(root, &packet)?;
    verify_crash_replay(root, &packet)?;
    let provenance = packet
        .provenance
        .as_ref()
        .context("qualified campaign clock requires packet provenance")?;
    verify_campaign_clock(root, &packet, Some(provenance))?;
    verify_residual_risks(root, &packet)?;
    verify_qualification_stage(&packet)?;
    if packet.qualification_stage == "1b" {
        // M17.5 F-33: 36 of the 65 declared mutants are anchored to
        // declarations and cannot be applied. ADR-0021 defers §3's mutation
        // clauses to 1b, so this refuses there rather than blocking 1a on work
        // that milestone already deferred — but it MUST refuse, or M24
        // rediscovers it after Phase 1 is built.
        verify_mutant_anchors_support_operators(root, &packet)?;
        verify_mutant_killing_tests(root, &packet)?;
        verify_mutant_evidence(root, &packet)?;
        verify_mutant_evidence_replay(root, &packet)?;
    }
    verify_canary_evidence(root, &packet)?;
    verify_generated_evidence(root, &packet)?;
    verify_review_evidence(root, &packet)?;
    verify_mutant_concurrence(root, &packet)?;
    // HAQP-1a qualifies packet machinery before Phase 1 authorization; its
    // inventory may still name future M18-M24 tests. HAQP-1b runs at M24 and
    // must bind every declared test to a real, runnable function.
    if packet.qualification_stage == "1b" {
        verify_test_names_exist(root, &packet)?;
    }
    // Runs at 1a as well: it only speaks when a leaf names more than one
    // EXISTING test, so an inventory naming future M18-M24 tests is unaffected,
    // and an ambiguity that would let a namesake certify a kill is caught now
    // rather than at M24 (review of c455cdf).
    verify_killing_test_leaves_are_unambiguous(root, &packet)?;
    Ok(())
}

/// Bind the packet's crash-boundary claims to the fault lane's committed
/// artifact (M17.5 pass-2 #19).
///
/// The lane wrote `target/haqp/crash.json`, which is gitignored, so nothing
/// could ever read it back: the artifact said `pass`, the packet said
/// `registered`, and `haq verify-inventory` exited 0 with no one comparing the
/// two. The packet's crash rows were pure assertion.
/// What ILRP recovery must reach from each registered crash boundary, read off
/// the protocol rather than off the run. Blind pass 1 at `6b36bbb9` (A06): the
/// evidence carried digests, which prove recovery is repeatable and leaves no
/// residue and cannot say WHICH state it repeatably reached, and the only
/// terminal expectation was one string declared by the scenario the harness
/// itself runs. A recovery resolving every boundary wrongly but consistently
/// satisfied all of it.
///
/// The protocol decides these, not the implementation: before the intent is
/// durable there is nothing to recover, so recovery must find no terminal at
/// all; once it is durable the protocol rolls forward, so every later boundary
/// must reach `Committed`. A boundary reaching anything else — `Aborted` after
/// the intent was committed, or `NeedsReview` where the world was intact — is
/// a recovery that lost or invented work.
const CRASH_TERMINAL_EXPECTATION: [(&str, &[&str]); 8] = [
    ("ilrp/before_intent_commit", &[]),
    ("ilrp/after_intent_commit", &["Committed"]),
    ("ilrp/before_external_apply", &["Committed"]),
    ("ilrp/after_external_apply", &["Committed"]),
    ("ilrp/before_ack", &["Committed"]),
    ("ilrp/after_ack", &["Committed"]),
    ("ilrp/before_finalize", &["Committed"]),
    ("ilrp/after_finalize_before_notify", &["Committed"]),
];

/// Judge each boundary's observed terminals against the protocol's table.
fn verify_crash_terminal_states(evidence: &CrashEvidence) -> Result<()> {
    for row in &evidence.boundaries {
        let expected = CRASH_TERMINAL_EXPECTATION
            .iter()
            .find(|(boundary, _)| *boundary == row.boundary)
            .map(|(_, states)| *states)
            .with_context(|| {
                format!(
                    "crash boundary {:?} has no declared terminal expectation; a boundary the \
                     protocol table does not name cannot be judged",
                    row.boundary
                )
            })?;
        for pair in &row.recovery_pairs {
            anyhow::ensure!(
                pair.first_terminals == expected,
                "crash boundary {} recovered to {:?} in scenario {} occurrence {}; the protocol \
                 requires {expected:?}",
                row.boundary,
                pair.first_terminals,
                pair.scenario,
                pair.occurrence
            );
        }
    }
    let named = CRASH_TERMINAL_EXPECTATION
        .iter()
        .map(|(boundary, _)| (*boundary).to_owned())
        .collect::<BTreeSet<_>>();
    let recorded = evidence
        .boundaries
        .iter()
        .map(|row| row.boundary.clone())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        named == recorded,
        "crash boundaries recorded {recorded:?} but the protocol table names {named:?}"
    );
    Ok(())
}

fn verify_crash_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/crash.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed crash evidence is required; run `just haq-crash`")
    })?;
    let recorded: CrashEvidence =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;

    let git = |args: &[&str]| -> Result<String> {
        let out = Command::new("git").current_dir(root).args(args).output()?;
        anyhow::ensure!(out.status.success(), "git {args:?} failed");
        Ok(String::from_utf8(out.stdout)?.trim().to_owned())
    };
    let (expected_commit, expected_tree) = if let Some(provenance) = &packet.provenance {
        (
            provenance.fixed_commit.clone(),
            provenance.fixed_tree.clone(),
        )
    } else {
        let commit = git(&["rev-parse", "HEAD"])?;
        let tree = git(&["rev-parse", &format!("{commit}^{{tree}}")])?;
        (commit, tree)
    };
    require_eq(
        "crash evidence source_commit",
        &recorded.source_commit,
        &expected_commit,
    )?;
    require_eq(
        "crash evidence source_tree",
        &recorded.source_tree,
        &expected_tree,
    )?;
    let lockfile_blake3 = hex_digest(&fs::read(root.join("Cargo.lock"))?);
    require_eq(
        "crash evidence lockfile_blake3",
        &recorded.lockfile_blake3,
        &lockfile_blake3,
    )?;

    let declared = packet
        .crash_boundaries
        .iter()
        .map(|row| row.boundary.clone())
        .collect::<BTreeSet<_>>();
    verify_recovery_proof_presence(&recorded)?;
    verify_crash_rows(&recorded, &declared)?;
    verify_crash_terminal_states(&recorded)?;
    verify_crash_injection_bindings(packet, &recorded)
}

/// Re-run the fault matrix during qualification and compare its measured
/// recovery rows with the committed artifact. Hex digests alone are claims;
/// this replay is the independent producer that makes fabricated terminal,
/// Basis, and effect proofs inadmissible (P1-A05).
fn verify_crash_replay(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let recorded_path = root.join("conformance/haqp/evidence/crash.json");
    let replay_path = root.join("target/haqp/crash-replay.json");
    if let Some(parent) = replay_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let status = Command::new("cargo")
        .current_dir(root)
        .args([
            "run",
            "-q",
            "-p",
            "liminal-conformance",
            "--bin",
            "crash-evidence",
        ])
        .env("HAQP_CRASH_EVIDENCE_OUT", &replay_path)
        .status()
        .context("run independent crash-evidence replay")?;
    anyhow::ensure!(status.success(), "independent crash-evidence replay failed");
    let recorded: CrashEvidence = serde_json::from_slice(&fs::read(&recorded_path)?)?;
    let replayed: CrashEvidence = serde_json::from_slice(&fs::read(&replay_path)?)?;
    require_eq(
        "crash replay lockfile_blake3",
        &replayed.lockfile_blake3,
        &recorded.lockfile_blake3,
    )?;
    anyhow::ensure!(
        replayed.registered == recorded.registered
            && replayed.exercised == recorded.exercised
            && replayed.scenarios == recorded.scenarios
            && replayed.boundaries == recorded.boundaries,
        "committed crash evidence differs from independent fault-matrix replay"
    );
    let declared = packet
        .crash_boundaries
        .iter()
        .map(|row| row.boundary.clone())
        .collect::<BTreeSet<_>>();
    verify_crash_rows(&replayed, &declared)?;
    verify_crash_injection_bindings(packet, &replayed)
}

fn verify_recovery_proof_presence(recorded: &CrashEvidence) -> Result<()> {
    for boundary in &recorded.boundaries {
        for pair in &boundary.recovery_pairs {
            anyhow::ensure!(
                !pair.first_terminal_digest.is_empty()
                    && !pair.second_terminal_digest.is_empty()
                    && !pair.first_basis_digest.is_empty()
                    && !pair.second_basis_digest.is_empty()
                    && !pair.first_effect_digest.is_empty()
                    && !pair.second_effect_digest.is_empty(),
                "crash boundary {} recovery case {}:{} lacks terminal/Basis/effect duplicate proof",
                boundary.boundary,
                pair.scenario,
                pair.occurrence
            );
        }
    }
    Ok(())
}

/// Bind the packet's canary claims to the run that produced them
/// (M17.5 F-27 / P1-A03).
///
/// The qualified path accepted `result: caught` on every canary row while never
/// invoking the runner and never reading its output. `run_canaries_repo` wrote
/// `target/haqp/canaries.json`, which was gitignored — the same shape as the
/// crash-evidence defect fixed at pass-2 #19, left in place for canaries.
///
/// Canaries are the mechanism the whole gate rests on: each one proves a
/// deliberate violation makes the expected gate fail. A canary table nobody
/// executed proves nothing at all.
fn verify_canary_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/canaries.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed canary evidence is required; run `just haq-canaries`")
    })?;
    let recorded: Vec<CanaryEvidence> =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    verify_canary_rows(&packet.canaries, &recorded)?;
    if let Some(provenance) = &packet.provenance {
        verify_canary_evidence_replay(root, provenance, &recorded)?;
    }
    Ok(())
}

/// Re-run canaries from the fixed inventory commit.  A qualified packet is a
/// metadata child and has resolved statuses, so replaying its in-memory shape
/// would no longer exercise the inventory verifier that the canary campaign
/// claims to attack.  The fixed parent is the authoritative proposed packet;
/// its packet and Markdown are loaded from Git, mutated, and checked again.
fn verify_canary_evidence_replay(
    root: &Utf8Path,
    provenance: &Provenance,
    recorded: &[CanaryEvidence],
) -> Result<()> {
    let git_show = |path: &str| -> Result<Vec<u8>> {
        let spec = format!("{}:{path}", provenance.fixed_commit);
        let output = Command::new("git")
            .current_dir(root)
            .args(["show", &spec])
            .output()
            .with_context(|| format!("git show {spec}"))?;
        anyhow::ensure!(
            output.status.success(),
            "git show {spec} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(output.stdout)
    };
    let baseline: Packet = serde_json::from_slice(&git_show("conformance/haqp/packet.json")?)
        .context("parse fixed canary packet")?;
    verify_packet_shape(&baseline)?;
    verify_packet_statuses(&baseline, inventory_statuses())?;
    let markdown = String::from_utf8(git_show("docs/execution/phase1-suite-review.md")?)
        .context("fixed canary Markdown is not UTF-8")?;
    let baseline_digest = packet_digest(&baseline)?;
    anyhow::ensure!(
        markdown.contains(&baseline_digest),
        "fixed canary Markdown is not bound to fixed packet digest"
    );
    let replayed = run_canary_suite(root, &baseline, &markdown)?;
    anyhow::ensure!(
        replayed == recorded,
        "committed canary evidence differs from replay from fixed inventory commit"
    );
    Ok(())
}

fn verify_canary_rows(declared: &[Canary], recorded: &[CanaryEvidence]) -> Result<()> {
    require_unique(
        recorded.iter().map(|row| row.id.as_str()),
        "canary evidence",
    )?;
    let by_id: BTreeMap<&str, &CanaryEvidence> =
        recorded.iter().map(|row| (row.id.as_str(), row)).collect();
    for canary in declared {
        if canary.result != "caught" {
            continue;
        }
        let evidence = by_id.get(canary.id.as_str()).with_context(|| {
            format!(
                "{} claims caught but the committed canary run has no such canary",
                canary.id
            )
        })?;
        if !evidence.caught {
            anyhow::bail!(
                "{} claims caught but the committed run recorded it as NOT caught",
                canary.id
            );
        }
        // The prose must describe the mutation that actually ran, or the table
        // a reader trusts describes a different experiment from the one
        // performed.
        if evidence.violation != canary.violation {
            anyhow::bail!(
                "{}: packet says the violation is {:?} but the run performed {:?}",
                canary.id,
                canary.violation,
                evidence.violation
            );
        }
        if evidence.expected_failure != canary.expected_failure {
            anyhow::bail!(
                "{}: packet expects failure {:?} but the run expected {:?}",
                canary.id,
                canary.expected_failure,
                evidence.expected_failure
            );
        }
        anyhow::ensure!(
            canary_failure_matches(&evidence.expected_failure, &evidence.observed_failure),
            "{}: observed failure {:?} does not match expected failure {:?}",
            canary.id,
            evidence.observed_failure,
            evidence.expected_failure
        );
        require_eq("canary evidence gate", &evidence.gate, &canary.gate)?;
        require_eq(
            "canary evidence mutation semantics",
            &evidence.mutation_semantics,
            canary_mutation_semantics(&canary.id)?,
        )?;
    }
    // Every canary the run exercised must be declared, or the packet is a
    // subset of the experiment and a reader cannot tell which rows are missing.
    let declared_ids: BTreeSet<&str> = declared.iter().map(|row| row.id.as_str()).collect();
    for row in recorded {
        if !declared_ids.contains(row.id.as_str()) {
            anyhow::bail!(
                "{} was exercised by the canary run but the packet does not declare it",
                row.id
            );
        }
    }
    Ok(())
}

/// Bind each generated family's claimed hash to the committed artifact
/// (M17.5 F-27 / P1-A04, and P2-F03 — both blind passes found it).
///
/// `evidence_hash` was checked for length and nothing else: never recomputed,
/// never compared against a run, and `target/haqp/generated.json` — the file the
/// generator actually writes — was gitignored and therefore unreadable.
fn verify_generated_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/generated.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed generated evidence is required; run `just haq-generated`")
    })?;
    let recorded: GeneratedEvidenceArtifact =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    require_eq(
        "generated evidence schema_version",
        &recorded.schema_version,
        "haqp-generated-v3-negative-categories",
    )?;
    require_eq(
        "generated evidence artifact_blake3",
        &recorded.artifact_blake3,
        &generated_artifact_digest(&recorded.rows)?,
    )?;
    require_exact_ids(
        recorded.rows.iter().map(|row| row.family.as_str()),
        GENERATED_FAMILIES.iter().map(|family| (*family).to_owned()),
        "generated evidence family",
    )?;
    verify_generated_rows(root, &packet.generated, &recorded.rows)?;
    // A committed packet and artifact are both attacker-controlled inputs to
    // this verifier.  Once qualification claims completion, replay the fixed
    // seeded generator and compare rows, rather than trusting a jointly edited
    // digest pair (P1-A09).
    if packet.qualification_state == "complete" {
        let (replayed, _) = generate_evidence(100_000)?;
        anyhow::ensure!(
            replayed == recorded.rows,
            "qualified generated evidence differs from deterministic 100000-case replay"
        );
    }
    Ok(())
}

fn verify_generated_rows(
    root: &Utf8Path,
    declared: &[Generated],
    recorded: &[GeneratedEvidence],
) -> Result<()> {
    let mut by_family = BTreeMap::new();
    for row in recorded {
        if by_family.insert(row.family.as_str(), row).is_some() {
            anyhow::bail!("generated evidence repeats family {:?}", row.family);
        }
    }
    for family in declared {
        if family.result != "pass" {
            continue;
        }
        let evidence = by_family.get(family.family.as_str()).with_context(|| {
            format!(
                "{} claims pass but the committed generated evidence has no such family",
                family.family
            )
        })?;
        // The hash covers the family's whole accept/discard stream and cannot
        // be recomputed here without rerunning 100,000 cases. It is bound
        // instead to the digest the RUN recorded, which is the strongest check
        // available at verification time: the packet may no longer cite a
        // number no run produced. Determinism of that digest is pinned
        // separately by `generated_evidence_is_byte_identical_across_runs`, so
        // editing the committed artifact to match a doctored packet is
        // detectable by rerunning the generator.
        let claimed = family
            .evidence_hash
            .as_deref()
            .context("inventory layer requires a hash before `pass`")?;
        if !claimed.eq_ignore_ascii_case(&evidence.evidence_hash) {
            anyhow::bail!(
                "{}: packet claims evidence hash {claimed} but the committed run recorded {}",
                family.family,
                evidence.evidence_hash
            );
        }
        for (label, packet_value, evidence_value) in [
            ("accepted", family.accepted, evidence.accepted),
            ("attempts", family.attempts, evidence.attempts),
            ("discards", family.discards, evidence.discards),
            ("negatives", family.negatives, evidence.negatives),
        ] {
            if packet_value != evidence_value {
                anyhow::bail!(
                    "{}: packet declares {packet_value} {label} but the committed run recorded {evidence_value}",
                    family.family
                );
            }
        }
        if family.seed != Some(evidence.seed) {
            anyhow::bail!(
                "{}: packet declares seed {:?} but the committed run used {}",
                family.family,
                family.seed,
                evidence.seed
            );
        }
        verify_generated_contract(root, family, evidence)?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "generated relation contract keeps all cross-artifact invariants together"
)]
fn verify_generated_contract(
    root: &Utf8Path,
    family: &Generated,
    evidence: &GeneratedEvidence,
) -> Result<()> {
    let expected = generated_relations(family.family.as_str());
    if expected.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        !family.relations.is_empty(),
        "{}: a passing generated family must declare every relation",
        family.family
    );
    let oracle = evidence
        .oracle
        .as_ref()
        .with_context(|| format!("{}: generated evidence has no oracle", family.family))?;
    let declared_oracle = family
        .oracle
        .as_ref()
        .with_context(|| format!("{}: generated packet has no oracle", family.family))?;
    require_eq(
        &format!("{} generated oracle id", family.family),
        &oracle.id,
        &declared_oracle.id,
    )?;
    require_eq(
        &format!("{} generated oracle source", family.family),
        &oracle.source,
        &declared_oracle.source,
    )?;
    require_eq(
        &format!("{} generated oracle source registry", family.family),
        &oracle.source,
        generated_oracle_source(family.family.as_str()),
    )?;
    require_exact_ids(
        family.relations.iter().map(String::as_str),
        expected.iter().copied().map(str::to_owned),
        "generated packet relation",
    )?;
    require_exact_ids(
        evidence.relations.iter().map(|row| row.relation.as_str()),
        expected.iter().copied().map(str::to_owned),
        "generated evidence relation",
    )?;
    require_unique(
        evidence.relations.iter().map(|row| row.relation.as_str()),
        "generated relation",
    )?;
    for relation in &evidence.relations {
        require_eq("generated relation result", &relation.result, "pass")?;
        let expected_artifact = blake3::hash(
            format!(
                "{}\0{}\0{}",
                family.family, relation.relation, evidence.evidence_hash
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        require_eq(
            &format!("{} relation artifact", family.family),
            &relation.artifact_blake3,
            &expected_artifact,
        )?;
        require_eq("generated relation oracle", &relation.oracle_id, &oracle.id)?;
    }
    require_exact_ids(
        evidence.category_counts.keys().map(String::as_str),
        generated_seed_categories(family.family.as_str())
            .iter()
            .copied()
            .map(str::to_owned),
        &format!("{} generated seed category", family.family),
    )?;
    let category_total = evidence
        .category_counts
        .values()
        .try_fold(0_u64, |total, count| total.checked_add(*count))
        .with_context(|| format!("{} generated category count overflow", family.family))?;
    anyhow::ensure!(
        category_total == evidence.attempts,
        "{} generated category total {} != attempts {}",
        family.family,
        category_total,
        evidence.attempts
    );
    anyhow::ensure!(
        evidence.category_counts.values().all(|count| *count > 0),
        "{} generated seed category has zero measured cases",
        family.family
    );
    anyhow::ensure!(
        oracle.independent,
        "{}: generated oracle is not marked independent",
        family.family
    );
    verify_oracle_source_coordinate(root, &oracle.source, &family.family)?;
    let expected_matrix = blake3::hash(serde_json::to_vec(&evidence.relations)?.as_slice())
        .to_hex()
        .to_string();
    require_eq(
        &format!("{} relation_matrix_blake3", family.family),
        &oracle.relation_matrix_blake3,
        &expected_matrix,
    )?;
    Ok(())
}

fn generated_relations(family: &str) -> &'static [&'static str] {
    match family {
        "source/CST/formatting" => &["lossless_emit", "format_idempotence"],
        "graph/interchange codecs" => &["byte_canonical_stability", "field_fidelity"],
        "transforms/projections" => &["identity_merge", "outcome_class_symmetry"],
        "repair/ILRP/recovery" => &[
            "dependency_order",
            "permutation_invariance",
            "independent_edge_check",
        ],
        "Basis/revision/query invalidation" => &[
            "irrelevant_input_invariance",
            "monotonicity",
            "independent_read_set_oracle",
        ],
        _ => &[],
    }
}

const GENERATED_SEED_CATEGORIES: [(&str, &[&str]); 5] = [
    (
        "source/CST/formatting",
        &[
            "empty",
            "single-token",
            "whitespace",
            "unicode",
            "long-line",
            "truncated",
            "unterminated",
            "nested",
            "deep",
            "wide",
            "control-byte",
            "invalid-utf8",
            "comment",
            "escape",
            "boundary-offset",
            "hostile",
        ],
    ),
    (
        "graph/interchange codecs",
        &[
            "empty",
            "single-node",
            "single-edge",
            "duplicate-id",
            "missing-node",
            "cycle",
            "dag",
            "wide",
            "deep",
            "unknown-kind",
            "external-value",
            "comment",
            "large-payload",
            "truncated-json",
            "invalid-json",
            "hostile",
        ],
    ),
    (
        "transforms/projections",
        &[
            "identity",
            "insert",
            "delete",
            "replace",
            "move",
            "overlap",
            "commute",
            "conflict",
            "empty",
            "deep",
            "wide",
            "invalid-span",
            "boundary-span",
            "large-patch",
            "truncated",
            "hostile",
        ],
    ),
    (
        "repair/ILRP/recovery",
        &[
            "prepared",
            "applying",
            "external-applied",
            "finalizing",
            "committed",
            "needs-review",
            "graph-step",
            "file-step",
            "two-step",
            "contested",
            "poststate",
            "cycle",
            "missing-step",
            "duplicate-ack",
            "truncated-intent",
            "hostile",
            "boundary",
        ],
    ),
    (
        "Basis/revision/query invalidation",
        &[
            "empty-basis",
            "single-revision",
            "branch",
            "merge",
            "client-scoped",
            "durable-only",
            "published",
            "federated",
            "stale",
            "fresh",
            "invalid-token",
            "large-frontier",
            "unknown-perspective",
            "truncated",
            "reordered",
            "hostile",
        ],
    ),
];

fn generated_seed_categories(family: &str) -> &'static [&'static str] {
    GENERATED_SEED_CATEGORIES
        .iter()
        .find_map(|(name, categories)| (*name == family).then_some(*categories))
        .unwrap_or(&[])
}

/// Closed oracle-source registry. Packet prose cannot choose what counts as
/// independent evidence; each family names one exact committed source anchor.
fn generated_oracle_source(family: &str) -> &'static str {
    match family {
        "source/CST/formatting" => "crates/liminal-xtask/src/haq.rs:independent_oracle_source_cst",
        "graph/interchange codecs" => {
            "crates/liminal-xtask/src/haq.rs:independent_oracle_source_graph"
        }
        "transforms/projections" => {
            "crates/liminal-xtask/src/haq.rs:independent_oracle_source_transform"
        }
        "repair/ILRP/recovery" => {
            "crates/liminal-xtask/src/haq.rs:independent_oracle_source_repair"
        }
        "Basis/revision/query invalidation" => {
            "crates/liminal-xtask/src/haq.rs:independent_oracle_source_invalidation"
        }
        _ => "",
    }
}

fn verify_oracle_source_coordinate(root: &Utf8Path, source: &str, family: &str) -> Result<()> {
    let expected = generated_oracle_source(family);
    anyhow::ensure!(!expected.is_empty(), "{family}: unknown oracle family");
    require_eq(
        &format!("{family} oracle source registry"),
        source,
        expected,
    )?;
    let (file, anchor) = source
        .split_once(':')
        .with_context(|| format!("{family}: oracle source lacks exact coordinate"))?;
    let path = safe_repo_path(root, file, "oracle source")?;
    let text = fs::read_to_string(&path)
        .with_context(|| format!("{family}: read oracle source {path}"))?;
    anyhow::ensure!(
        has_exact_coordinate_anchor(&text, anchor),
        "{family}: oracle source anchor {anchor:?} is absent from {path}"
    );
    Ok(())
}

/// Independent source/CST oracle. It compares raw observed bytes, not parser
/// equality or a formatter-owned snapshot.
/// The coarse scan judged against the source directly, never against the fine
/// parse: presence, ranges, classification and the hash relation. Returns the
/// classifications and hashes so the case witness binds them.
fn verify_coarse_scan(
    source: &str,
    coarse: &liminal_cst::CstDocument,
) -> Result<(Vec<&'static str>, Vec<String>)> {
    // The coarse scan is judged against the source directly, never against
    // the fine parse: a document with a non-blank line has at least one block,
    // every block's range lies inside the source and holds no blank line, and
    // the blocks do not overlap (A01).
    anyhow::ensure!(
        coarse.source == source,
        "coarse_parse changed the source it was given"
    );
    let has_content = source.lines().any(|line| !line.trim().is_empty());
    anyhow::ensure!(
        has_content != coarse.blocks.is_empty(),
        "coarse_parse found {} blocks for a document that {} content",
        coarse.blocks.len(),
        if has_content { "has" } else { "has no" }
    );
    let mut previous_end = 0usize;
    let mut kinds = Vec::new();
    let mut hashes = Vec::new();
    let mut by_text: BTreeMap<&str, String> = BTreeMap::new();
    let mut by_hash: BTreeMap<String, &str> = BTreeMap::new();
    for block in &coarse.blocks {
        let start = usize::try_from(block.range.start).unwrap_or(usize::MAX);
        let end = usize::try_from(block.range.end).unwrap_or(usize::MAX);
        anyhow::ensure!(
            start < end && end <= source.len() && start >= previous_end,
            "coarse block {start}..{end} is empty, out of range, or overlaps its predecessor"
        );
        anyhow::ensure!(
            source
                .get(start..end)
                .is_some_and(|text| text.lines().any(|line| !line.trim().is_empty())),
            "coarse block {start}..{end} holds no content"
        );
        // Blind pass 1 at c29bc0ea (A01): the scan's ranges were judged and
        // its classification was not, so `coarse_parse` could label every
        // block alike and no generated case would notice. The kind is
        // re-derived from the block's own first line, never read back from
        // the production classifier ADR-0020 §5 forbids this oracle to reach.
        // What this catches is the classifier drifting from its rule — a
        // mutation, a reordered branch, a dropped case. It cannot catch the
        // rule itself being wrong, because the rule is what it restates.
        let text = source.get(start..end).unwrap_or_default();
        let first = text.lines().next().unwrap_or_default().trim_start();
        let hash_marks = first.chars().take_while(|ch| *ch == '#').count();
        let expected = if first.starts_with("```") {
            "fence"
        } else if hash_marks > 0 && first[hash_marks..].starts_with(' ') {
            "heading"
        } else if first.starts_with("- ") || first.starts_with("* ") {
            "list"
        } else if first.starts_with('@') {
            "directive"
        } else if first.starts_with("![") || first.contains("](") {
            "resource-reference"
        } else {
            "paragraph-like"
        };
        // `DocumentRoot`, `EmbeddedLanguage` and `Unknown` are declared by the
        // enum and produced by nothing the coarse scan does. If one appears,
        // the scan has grown a rule this oracle was never given, and saying so
        // is more use than a mismatch that reads like a misclassification.
        let actual = match block.coarse_kind {
            liminal_cst::CoarseKind::Fence => "fence",
            liminal_cst::CoarseKind::Heading => "heading",
            liminal_cst::CoarseKind::List => "list",
            liminal_cst::CoarseKind::Directive => "directive",
            liminal_cst::CoarseKind::ResourceReference => "resource-reference",
            liminal_cst::CoarseKind::ParagraphLike => "paragraph-like",
            other => anyhow::bail!(
                "coarse block {start}..{end} is classified {other:?}, a kind the coarse scan \
                 did not emit when this oracle was written: {text:?}"
            ),
        };
        anyhow::ensure!(
            actual == expected,
            "coarse block {start}..{end} is classified {actual}, not {expected}: {text:?}"
        );
        // Blind pass 1 at ee646f8c (A01): ranges and kinds were judged and the
        // block hash was not, so a hash that ignored its bytes went unseen.
        // Checked as a relation rather than a value: equal text hashes alike,
        // different text does not. That catches a constant or text-blind hash
        // without this oracle restating the digest production computes.
        // `to_hex`, not `{:?}`: the witness is a durable digest, and keying it
        // on a Debug impl would move it if that impl were ever reformatted
        // (review of 98f65011).
        let digest = block.hash.to_hex();
        if let Some(seen) = by_text.insert(text, digest.clone()) {
            anyhow::ensure!(
                seen == digest,
                "coarse blocks with identical text hash differently: {text:?}"
            );
        }
        if let Some(seen) = by_hash.insert(digest.clone(), text) {
            anyhow::ensure!(
                seen == text,
                "coarse blocks with different text share hash {digest}: {seen:?} and {text:?}"
            );
        }
        kinds.push(actual);
        hashes.push(digest);
        previous_end = end;
    }
    Ok((kinds, hashes))
}

/// Characters the dialect lowers into structure: lists, headings, quotes,
/// tables and fences. Losing one of these to formatting is the formatter
/// working; losing anything else that is not whitespace is losing content.
const STRUCTURAL_MARKUP: [char; 7] = ['-', '*', '#', '>', '|', '+', '`'];

/// A document's words in order. An escape stands for the character it
/// denotes, so `\n` inside an emitted literal is a separator and not the
/// letter `n` glued to the next word — without that, escaping a newline
/// looks like reordering.
///
/// Review of `ec045588`: an escape the emitter does not produce — `\u` with
/// too few hex digits, say — is normalised imperfectly, and that is safe
/// because this runs over BOTH sides. Mangling can only add words to the
/// output, never remove one from the source, and added words are what a
/// subsequence tolerates. Every character the emitter actually escapes is
/// non-alphanumeric, so it already separates words in the source.
fn sequence(text: &str) -> Vec<String> {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            plain.push(ch);
            continue;
        }
        // `\uXXXX` and `\xNN` denote one character each; anything else
        // after a backslash denotes the character that follows it.
        let digits = match chars.peek() {
            Some('u') => 4,
            Some('x') => 2,
            _ => 0,
        };
        chars.next();
        for _ in 0..digits {
            if chars.peek().is_some_and(char::is_ascii_hexdigit) {
                chars.next();
            }
        }
        plain.push(' ');
    }
    plain
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|run| !run.is_empty())
        .map(str::to_owned)
        .collect()
}

fn independent_oracle_source_cst(
    source: &str,
    emitted: &str,
    once: &str,
    twice: &str,
    content: &str,
    coarse: &liminal_cst::CstDocument,
) -> Result<Vec<u8>> {
    anyhow::ensure!(emitted == source, "CST emit is not lossless for {source:?}");
    anyhow::ensure!(once == twice, "formatting is not idempotent");
    // Blind pass 1 at 5fb1b57 (A01): those two say nothing about whether
    // formatting KEPT the document. A formatter returning a constant is
    // idempotent, and the lossless check never touches the formatter at all.
    // The generated word is content, not layout: formatting may move it, never
    // lose it. A durable-marker set would be the natural check and is nearly
    // vacuous for this corpus, where only the malformed categories carry `{#`.
    // Alphanumeric runs, not the raw token: a generated word can begin with
    // markdown syntax (`- v`), and lowering `- ` into list structure is the
    // formatter doing its job, not losing content.
    // Blind pass 1 at 91b54842 (A01): the runs were filtered to three
    // characters and up. `Rng::word` draws one to twelve characters from an
    // alphabet holding `-`, `_` and a space, so `a`, `x y` and `a-b` are all
    // reachable tokens with no run that long — for those the survival check
    // had nothing to check and a formatter could drop the content outright.
    // Every non-empty run counts now. The dialect numbers nothing (an ordered
    // block is the keyword `ordered`, never `1.`), so no digit legitimately
    // disappears, and a one-character run is weak rather than false: it fires
    // only when that character is absent from the whole output.
    let words = |text: &str| {
        text.split(|ch: char| !ch.is_alphanumeric())
            .filter(|run| !run.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
    };
    let lost = words(content)
        .into_iter()
        .filter(|word| source.contains(word.as_str()) && !once.contains(word.as_str()))
        .collect::<Vec<_>>();
    // Blind pass 1 at `6b36bbb9` (A09): survival was judged on alphanumeric
    // runs, so content that is punctuation raised nothing — a token of `_`
    // has no run at all and could be dropped in silence. A character is
    // markup only if the dialect lowers it: `-`, `*`, `#`, `>`, `|`, `+` and
    // a backtick begin lists, headings, quotes, tables and fences, and
    // lowering those is the formatter doing its job. Everything else that is
    // not whitespace is content and must come back.
    let lost_marks = content
        .chars()
        .filter(|ch| {
            !ch.is_whitespace()
                && !ch.is_alphanumeric()
                && !STRUCTURAL_MARKUP.contains(ch)
                && source.contains(*ch)
                && !once.contains(*ch)
        })
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        lost_marks.is_empty(),
        "formatting dropped the document's punctuation content {lost_marks:?}: {source:?} -> \
         {once:?}"
    );
    anyhow::ensure!(
        lost.is_empty(),
        "formatting dropped the document's content {lost:?}: {source:?} -> {once:?}"
    );
    // Blind pass 1 at `0a0b4b4b` (A01): membership was checked and ORDER was
    // not, so a formatter that scrambled a document's words kept every word,
    // every mark, non-emptiness and idempotence, and lost the meaning. A
    // formatter may move a word between lines; it may never move one past
    // another, so the sequence of alphanumeric runs is invariant.
    // Formatting may ADD words — the explicit dialect wraps content in `node`,
    // `paragraph`, `literal` — so the source's words must appear in the output
    // in their own order, not as the whole of it.
    let emitted_words = sequence(once);
    let source_words = sequence(source);
    let mut next = emitted_words.iter();
    if !source_words
        .iter()
        .all(|word| next.any(|candidate| candidate == word))
    {
        // Review of `ec045588`: a subsequence fails for two different reasons
        // and they are not the same defect. If some word appears fewer times
        // in the output than in the source, the formatter dropped a copy —
        // which the loss check above cannot see, because it asks whether the
        // word survives at all and not how often. Otherwise every word is
        // present the right number of times and the order changed.
        let count = |words: &[String], word: &String| words.iter().filter(|w| *w == word).count();
        let dropped = source_words
            .iter()
            .find(|word| count(&source_words, word) > count(&emitted_words, word));
        anyhow::ensure!(
            dropped.is_none(),
            "formatting dropped a copy of {:?}: {source:?} -> {once:?}",
            dropped.expect("checked")
        );
        anyhow::bail!("formatting reordered the document's words: {source:?} -> {once:?}");
    }
    anyhow::ensure!(
        source.trim().is_empty() || !once.trim().is_empty(),
        "formatting emptied a non-empty document"
    );
    let (kinds, hashes) = verify_coarse_scan(source, coarse)?;
    let mut witness = emitted.as_bytes().to_vec();
    witness.extend_from_slice(once.as_bytes());
    witness.extend_from_slice(format!("coarse:{}", coarse.blocks.len()).as_bytes());
    witness.extend_from_slice(kinds.join(",").as_bytes());
    witness.extend_from_slice(hashes.join(",").as_bytes());
    Ok(witness)
}

/// Independent merge oracle. It checks raw content and outcome classes, not
/// merge-result equality.
fn independent_oracle_source_transform(
    base: &str,
    ours: &str,
    identity: &liminal_source::merge::MergeOutcome,
    forward: &liminal_source::merge::MergeOutcome,
    swapped: &liminal_source::merge::MergeOutcome,
) -> Result<Vec<u8>> {
    let liminal_source::merge::MergeOutcome::Disjoint { merged } = identity else {
        anyhow::bail!("identity merge reported non-disjoint outcome: {identity:?}");
    };
    let merged_tokens = merged.split_whitespace().collect::<Vec<_>>();
    let ours_tokens = ours.split_whitespace().collect::<Vec<_>>();
    let mut merged_multiset = merged_tokens.clone();
    let mut ours_multiset = ours_tokens.clone();
    merged_multiset.sort_unstable();
    ours_multiset.sort_unstable();
    anyhow::ensure!(
        ours_multiset == merged_multiset,
        "identity merge dropped content: {ours:?} -> {merged:?}"
    );
    // Token multisets alone permit an implementation to drop or duplicate
    // durable markers. The merge contract keeps base slot order as its primary
    // layout; a pure move is therefore intentionally not an order-preserving
    // relation, but its marker set must still survive exactly.
    let marker_order = |text: &str| {
        text.split_whitespace()
            .filter(|token| token.starts_with("{#") && token.ends_with('}'))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let base_markers = marker_order(base);
    let ours_markers = marker_order(ours);
    if ours_markers != base_markers {
        let mut expected = ours_markers.clone();
        let mut actual = marker_order(merged);
        expected.sort_unstable();
        actual.sort_unstable();
        anyhow::ensure!(
            actual == expected,
            "identity merge changed durable marker set under layout normalization"
        );
        // Blind pass 1 at 612cbcc (A02): the marker SET surviving a move said
        // nothing about which content each marker kept; swapping the contents
        // of two moved markers passed. A move keeps every marker's content.
        let markered = |text: &str| {
            ordered_block_contents(text)
                .into_iter()
                .filter(|(marker, _)| marker.starts_with("{#"))
                .collect::<BTreeMap<_, _>>()
        };
        anyhow::ensure!(
            markered(merged) == markered(ours),
            "identity merge re-associated content across moved markers: {ours:?} -> {merged:?}"
        );
    }
    if ours_markers == base_markers {
        anyhow::ensure!(
            marker_order(merged) == ours_markers,
            "identity merge reordered durable markers"
        );
        anyhow::ensure!(
            ordered_block_contents(merged) == ordered_block_contents(ours),
            "identity merge reordered ordinary content: {ours:?} -> {merged:?}"
        );
    }
    let disjoint = |outcome: &liminal_source::merge::MergeOutcome| {
        matches!(
            outcome,
            liminal_source::merge::MergeOutcome::Disjoint { .. }
        )
    };
    anyhow::ensure!(
        disjoint(forward) == disjoint(swapped),
        "merge disjointness is not symmetric under swapping sides"
    );
    Ok(format!("{identity:?}|{forward:?}|{swapped:?}").into_bytes())
}

fn ordered_block_contents(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut anonymous = 0usize;
    let mut blocks = BTreeMap::new();
    for block in text.split("\n\n") {
        let marker = block
            .split_whitespace()
            .find(|token| token.starts_with("{#") && token.ends_with('}'))
            .map_or_else(
                || {
                    let key = format!("anon:{anonymous}");
                    anonymous += 1;
                    key
                },
                str::to_owned,
            );
        let tokens = block
            .split_whitespace()
            .filter(|token| !token.starts_with("{#") || !token.ends_with('}'))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        // Blocks containing only marker tokens carry no ordinary content whose
        // order can falsify this ordering oracle; marker order is checked above.
        if !tokens.is_empty() {
            blocks.insert(marker, tokens);
        }
    }
    blocks
}

/// Independent repair oracle. It checks the declared edge set against the
/// returned order and records that permutation of input edges is stable.
fn independent_oracle_source_repair(
    order: &[liminal_id::RepairStepId],
    ids: &[liminal_id::RepairStepId],
    edges: &[(usize, usize)],
    permuted_order: &[liminal_id::RepairStepId],
) -> Result<Vec<u8>> {
    let position: BTreeMap<liminal_id::RepairStepId, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    // Blind pass 1 at f360e90 (A03): equal length let a duplicated step stand
    // in for a missing one. The order is exactly the declared steps, once each.
    let mut declared = ids.to_vec();
    let mut returned = order.to_vec();
    declared.sort_unstable();
    returned.sort_unstable();
    anyhow::ensure!(
        declared == returned && declared.windows(2).all(|pair| pair[0] != pair[1]),
        "ordering dropped, duplicated or invented steps"
    );
    for (before, after) in edges {
        anyhow::ensure!(
            position[&ids[*before]] < position[&ids[*after]],
            "ordering violated a declared dependency"
        );
    }
    anyhow::ensure!(
        permuted_order == order,
        "topological order changed under permutation of dependency edges"
    );
    Ok(order
        .iter()
        .flat_map(|id| id.as_uuid().into_bytes())
        .collect())
}

/// Independent invalidation oracle. It compares expected read membership to
/// observed invalidation answers, including a negative witness.
fn independent_oracle_source_invalidation(
    deps: &mut liminal_revision::ComponentDeps,
    expected: &BTreeSet<liminal_id::JurisdictionKey>,
    unrelated: &liminal_id::JurisdictionKey,
    extra: &liminal_id::JurisdictionKey,
) -> Result<Vec<u8>> {
    for key in expected {
        anyhow::ensure!(
            deps.invalidated_by(key),
            "a recorded read did not invalidate: {key:?}"
        );
    }
    anyhow::ensure!(
        !deps.invalidated_by(unrelated),
        "an unread key invalidated the computation: {unrelated:?}"
    );
    // Add a new dependency only between observations. This makes the second
    // check a real monotonicity assertion instead of repeating the first one.
    anyhow::ensure!(
        !deps.invalidated_by(extra),
        "extra dependency was already present before monotonicity probe: {extra:?}"
    );
    deps.record(extra.clone());
    // Blind pass 1 at aa00d41 (A03): the relation only re-checked the keys that
    // already invalidated, so a `record` that did nothing passed. Monotonicity
    // is that the new read is now observed AND no old one was lost.
    anyhow::ensure!(
        deps.invalidated_by(extra),
        "recording a read did not make it invalidate: {extra:?}"
    );
    for key in expected {
        anyhow::ensure!(
            deps.invalidated_by(key),
            "recording another read un-invalidated {key:?}"
        );
    }
    anyhow::ensure!(
        !deps.invalidated_by(unrelated),
        "recording a read invalidated an unread key: {unrelated:?}"
    );
    let mut witness = Vec::new();
    for key in expected.iter().chain([unrelated, extra]) {
        witness.push(u8::from(deps.invalidated_by(key)));
    }
    Ok(witness)
}

fn generated_category_class(category: &str) -> &'static str {
    let category = category.to_ascii_lowercase();
    if category.contains("hostile") {
        "hostile"
    } else if category.contains("truncated") || category.contains("unterminated") {
        "truncated"
    } else if category.contains("invalid")
        || category.contains("malformed")
        || category.contains("unknown")
        || category.contains("missing")
        || category.contains("duplicate")
        || category.contains("cycle")
        || category.contains("conflict")
    {
        "malformed"
    } else if category.contains("boundary")
        || category.contains("single")
        || category.contains("empty")
        || category.contains("large")
        || category.contains("deep")
        || category.contains("wide")
        || category.contains("zero")
    {
        "boundary"
    } else {
        "valid"
    }
}

fn verify_crash_rows(recorded: &CrashEvidence, declared: &BTreeSet<String>) -> Result<()> {
    require_unique(
        recorded.boundaries.iter().map(|row| row.boundary.as_str()),
        "crash evidence boundary",
    )?;
    let evidenced = recorded
        .boundaries
        .iter()
        .map(|row| row.boundary.clone())
        .collect::<BTreeSet<_>>();
    if declared != &evidenced {
        anyhow::bail!(
            "crash evidence covers different boundaries than the packet declares: \
             packet={declared:?}, evidence={evidenced:?}"
        );
    }
    let before = evidenced
        .iter()
        .filter(|boundary| boundary.contains("/before_"))
        .count();
    let after = evidenced
        .iter()
        .filter(|boundary| boundary.contains("/after_"))
        .count();
    anyhow::ensure!(
        before > 0 && after > 0,
        "crash evidence must identify both before-transition and after-transition boundaries"
    );
    for boundary in &evidenced {
        anyhow::ensure!(
            boundary.contains("/before_") || boundary.contains("/after_"),
            "crash boundary {boundary:?} lacks an injection-side coordinate"
        );
    }
    anyhow::ensure!(
        recorded.registered == evidenced.len() && recorded.exercised == evidenced.len(),
        "crash evidence registered/exercised counts do not match evidenced boundary count"
    );
    // ADR-0020 §5: runtime discovery and declared inventory must match exactly,
    // and drift on EITHER side fails.
    if recorded.registered != recorded.exercised {
        anyhow::bail!(
            "crash evidence records {} registered boundaries but {} exercised — an \
             unexercised boundary is indistinguishable from a dead one",
            recorded.registered,
            recorded.exercised
        );
    }
    verify_crash_scenario_coverage(recorded, declared)?;
    for row in &recorded.boundaries {
        if row.occurrences_exercised == 0 {
            anyhow::bail!("crash boundary {} was never exercised", row.boundary);
        }
        anyhow::ensure!(
            row.injected,
            "crash boundary {} has no measured injection receipt",
            row.boundary
        );
        if row.result != "pass" {
            anyhow::bail!(
                "crash boundary {} reports result {:?}",
                row.boundary,
                row.result
            );
        }
        verify_recovery_pairs(row)?;
        if row.staged_residue != "none" {
            anyhow::bail!(
                "crash boundary {} left staged residue {:?}",
                row.boundary,
                row.staged_residue
            );
        }
    }
    Ok(())
}

/// Packet booleans describe both sides of each durable transition. Bind them
/// to measured evidence rows, rather than accepting packet-controlled claims
/// that the runner injected both sides (P1-A12).
fn verify_crash_injection_bindings(packet: &Packet, recorded: &CrashEvidence) -> Result<()> {
    // verify_crash_rows already enforces exact packet/evidence boundary-set
    // equality. This second pass binds each packet before/after boolean to the
    // measured injection receipt for its closed authoritative pair.
    let measured = recorded
        .boundaries
        .iter()
        .map(|row| (row.boundary.as_str(), row.injected))
        .collect::<BTreeMap<_, _>>();
    for declaration in &packet.crash_boundaries {
        let (before, after) = crash_injection_pair(&declaration.boundary).with_context(|| {
            format!(
                "crash boundary {:?} lacks closed before/after pair",
                declaration.boundary
            )
        })?;
        let measured_before = measured.get(before).copied().unwrap_or(false);
        let measured_after = measured.get(after).copied().unwrap_or(false);
        anyhow::ensure!(
            declaration.before == measured_before,
            "crash boundary {} before injection claim differs from measured evidence",
            declaration.boundary
        );
        anyhow::ensure!(
            declaration.after == measured_after,
            "crash boundary {} after injection claim differs from measured evidence",
            declaration.boundary
        );
    }
    Ok(())
}

/// Closed pair registry. `after_finalize_before_notify` is intentionally not
/// a mechanical `after_finalize` spelling; reconstructing names from packet
/// prose caused P1-A12 to evade the finalization-after side.
fn crash_injection_pair(boundary: &str) -> Option<(&'static str, &'static str)> {
    match boundary {
        "ilrp/before_intent_commit" | "ilrp/after_intent_commit" => {
            Some(("ilrp/before_intent_commit", "ilrp/after_intent_commit"))
        }
        "ilrp/before_external_apply" | "ilrp/after_external_apply" => {
            Some(("ilrp/before_external_apply", "ilrp/after_external_apply"))
        }
        "ilrp/before_ack" | "ilrp/after_ack" => Some(("ilrp/before_ack", "ilrp/after_ack")),
        "ilrp/before_finalize" | "ilrp/after_finalize_before_notify" => {
            Some(("ilrp/before_finalize", "ilrp/after_finalize_before_notify"))
        }
        _ => None,
    }
}

fn verify_crash_scenario_coverage(
    recorded: &CrashEvidence,
    declared: &BTreeSet<String>,
) -> Result<()> {
    anyhow::ensure!(
        !recorded.scenarios.is_empty(),
        "crash evidence records no scenarios; an empty fault matrix proves nothing"
    );
    for scenario in &recorded.scenarios {
        require_eq("crash scenario result", &scenario.result, "pass")?;
        anyhow::ensure!(
            scenario.faults_injected > 0,
            "crash scenario {} injected no faults, so it exercised nothing",
            scenario.scenario
        );
        anyhow::ensure!(
            !scenario.boundaries.is_empty(),
            "crash scenario {} injected faults but names no boundaries",
            scenario.scenario
        );
        for boundary in &scenario.boundaries {
            anyhow::ensure!(
                declared.contains(boundary),
                "crash scenario {} exercised unregistered boundary {boundary}",
                scenario.scenario
            );
        }
        anyhow::ensure!(
            scenario.faults_injected >= scenario.boundaries.len() as u64,
            "crash scenario {} reports {} faults across {} distinct boundaries",
            scenario.scenario,
            scenario.faults_injected,
            scenario.boundaries.len()
        );
    }
    let scenario_union = recorded
        .scenarios
        .iter()
        .flat_map(|scenario| scenario.boundaries.iter().cloned())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        scenario_union == *declared,
        "crash scenario matrix does not cover every declared boundary"
    );
    for row in &recorded.boundaries {
        for pair in &row.recovery_pairs {
            anyhow::ensure!(
                recorded.scenarios.iter().any(|scenario| {
                    scenario.scenario == pair.scenario
                        && scenario.boundaries.contains(&row.boundary)
                }),
                "crash recovery pair {}:{} is not covered by its scenario boundary matrix",
                pair.scenario,
                pair.occurrence
            );
        }
    }
    Ok(())
}

fn verify_recovery_pairs(row: &CrashEvidenceBoundary) -> Result<()> {
    anyhow::ensure!(
        row.recovery_pairs.len() as u64 == row.occurrences_exercised,
        "crash boundary {} records {} recovery pairs for {} exercised occurrences",
        row.boundary,
        row.recovery_pairs.len(),
        row.occurrences_exercised
    );
    let mut pair_ids = BTreeSet::new();
    for pair in &row.recovery_pairs {
        anyhow::ensure!(
            !pair.scenario.trim().is_empty(),
            "crash boundary {} has empty recovery scenario",
            row.boundary
        );
        anyhow::ensure!(
            pair_ids.insert((pair.scenario.as_str(), pair.occurrence)),
            "crash boundary {} repeats recovery case {}:{}",
            row.boundary,
            pair.scenario,
            pair.occurrence
        );
        let mut digests = vec![
            ("first_recovery_digest", &pair.first_recovery_digest),
            ("second_recovery_digest", &pair.second_recovery_digest),
        ];
        let enhanced = !pair.first_terminal_digest.is_empty()
            || !pair.second_terminal_digest.is_empty()
            || !pair.first_basis_digest.is_empty()
            || !pair.second_basis_digest.is_empty()
            || !pair.first_effect_digest.is_empty()
            || !pair.second_effect_digest.is_empty();
        if enhanced {
            digests.extend([
                ("first_terminal_digest", &pair.first_terminal_digest),
                ("second_terminal_digest", &pair.second_terminal_digest),
                ("first_basis_digest", &pair.first_basis_digest),
                ("second_basis_digest", &pair.second_basis_digest),
                ("first_effect_digest", &pair.first_effect_digest),
                ("second_effect_digest", &pair.second_effect_digest),
            ]);
        }
        for (label, digest) in digests {
            anyhow::ensure!(
                digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()),
                "crash boundary {} has invalid {label}",
                row.boundary
            );
        }
        anyhow::ensure!(
            pair.first_recovery_digest == pair.second_recovery_digest,
            "crash boundary {} recovery case {}:{} changed world digest on second recovery",
            row.boundary,
            pair.scenario,
            pair.occurrence
        );
        if enhanced {
            for (label, first, second) in [
                (
                    "terminal digest",
                    &pair.first_terminal_digest,
                    &pair.second_terminal_digest,
                ),
                (
                    "Basis digest",
                    &pair.first_basis_digest,
                    &pair.second_basis_digest,
                ),
                (
                    "effect digest",
                    &pair.first_effect_digest,
                    &pair.second_effect_digest,
                ),
            ] {
                anyhow::ensure!(
                    first == second,
                    "crash boundary {} recovery case {}:{} changed {label} on second recovery",
                    row.boundary,
                    pair.scenario,
                    pair.occurrence
                );
            }
        }
    }
    Ok(())
}

/// Bind each review row to a committed record (M17.5 pass-2 #20).
///
/// A review row was four self-asserted numbers with nothing behind them. The
/// internal-consistency rules hold at every layer; the committed record is
/// demanded only once a review claims `pass`, matching how
/// `verify_generated_inventory` treats seeds and evidence hashes.
fn verify_review_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let mut records: Vec<(String, ReviewRecord)> = Vec::new();
    for review in &packet.reviews {
        let raised = review.findings.iter().collect::<BTreeSet<_>>();
        let unverified = review
            .independently_reproduced
            .iter()
            .filter(|id| !raised.contains(id))
            .collect::<Vec<_>>();
        if !unverified.is_empty() {
            anyhow::bail!(
                "{} claims to have independently reproduced {unverified:?}, which it \
                 never raised as findings",
                review.reviewer
            );
        }
        // A finding counted as VERIFIED and unresolved must be one that survived
        // independent reproduction; otherwise "verified" means one model said so.
        if review.unresolved_verified_findings > review.independently_reproduced.len() as u64 {
            anyhow::bail!(
                "{} counts {} unresolved VERIFIED findings but independently reproduced \
                 only {} — a finding one reviewer asserted is not a verified finding \
                 (ADR-0020 §6)",
                review.reviewer,
                review.unresolved_verified_findings,
                review.independently_reproduced.len()
            );
        }
        if review.result != "pass" {
            continue;
        }
        let Some(evidence) = &review.evidence else {
            anyhow::bail!(
                "{} claims pass with no committed record; the lane's own output lives \
                 under target/ and cannot be read back",
                review.reviewer
            );
        };
        // The packet is the very thing this verifier exists to distrust, so its
        // paths are not taken on faith. `join` DISCARDS the base when handed an
        // absolute path, so `evidence: "/etc/passwd"` would read straight out of
        // the repository; `..` walks out just as effectively.
        let candidate = Utf8Path::new(evidence);
        if candidate.is_absolute()
            || candidate
                .components()
                .any(|part| part == camino::Utf8Component::ParentDir)
        {
            anyhow::bail!(
                "{} points its record at {evidence:?}, which escapes the repository; \
                 evidence paths must be relative and contain no `..`",
                review.reviewer
            );
        }
        let path = root.join(candidate);
        let canonical_root = fs::canonicalize(root.as_std_path())
            .context("canonicalize repository root for review evidence")?;
        let canonical_path = fs::canonicalize(&path).with_context(|| {
            format!(
                "{path}: {} claims pass but its record is missing",
                review.reviewer
            )
        })?;
        anyhow::ensure!(
            canonical_path.starts_with(&canonical_root),
            "{} review record resolves outside repository via symlink",
            review.reviewer
        );
        let bytes = fs::read(&path).with_context(|| {
            format!(
                "{path}: {} claims pass but its record is missing",
                review.reviewer
            )
        })?;
        let record: ReviewRecord =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
        verify_review_record(root, review, &record, &path)?;
        let provenance = packet
            .provenance
            .as_ref()
            .context("qualified review evidence requires packet provenance")?;
        require_eq(
            "review record fixed_base.commit",
            &record.fixed_base.commit,
            &provenance.fixed_commit,
        )?;
        require_eq(
            "review record fixed_base.tree",
            &record.fixed_base.tree,
            &provenance.fixed_tree,
        )?;
        anyhow::ensure!(
            record.fixed_base.clean,
            "{} review record was produced from a dirty fixed base",
            review.reviewer
        );
        records.push((review.reviewer.clone(), record));
    }
    verify_reviewer_independence(&records)?;
    verify_cross_pass_reproduction(&records)
}

/// A reviewer cannot make its own "independent reproduction" true by setting
/// two booleans in one record. Every reproduced finding must have one matching
/// verified defect in the other isolated pass, with the same attack class,
/// exact source target, and identical falsification claim/observation. This
/// prevents two semantically different reports at one line from becoming a
/// concurrence by coordinate coincidence; finding IDs remain pass-local.
/// Whether two blinded passes are reporting the same defect. Blind pass 1 at
/// `0a0b4b4b` (A10): this required the `attempt` and `observed_result` strings
/// to be byte-identical. Two reviewers who cannot see each other's work will
/// not write the same sentence, so a genuine independent reproduction was
/// rejected unless they coordinated wording — which is the one thing the
/// blinding exists to prevent. The class and the coordinate already match by
/// the time this is asked; what is left to establish is that the two are about
/// the same thing, and shared vocabulary establishes it. Four significant
/// words in common is far more than coincidence at one coordinate and far less
/// than dictation.
fn reports_the_same_defect(left: &ReviewRecordAttempt, right: &ReviewRecordAttempt) -> bool {
    // Blind pass 1 at `ec045588` (A02): both reports name the coordinate, so
    // `crates`, `liminal`, `xtask` and `haq` were four shared words before
    // either said anything — the coordinate matched twice, once as itself and
    // once as vocabulary. Words the coordinate already carries are struck out,
    // and so are the handful this campaign puts in every report.
    const UBIQUITOUS: [&str; 8] = [
        "verifier",
        "verify",
        "packet",
        "evidence",
        "check",
        "checks",
        "because",
        "committed",
    ];
    let coordinate: BTreeSet<String> = format!("{} {}", left.target, right.target)
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .map(str::to_owned)
        .collect();
    let vocabulary = |attempt: &ReviewRecordAttempt| {
        format!("{} {}", attempt.attempt, attempt.observed_result)
            .to_ascii_lowercase()
            .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .filter(|word| word.len() >= 4)
            .filter(|word| !coordinate.contains(*word))
            .filter(|word| !UBIQUITOUS.contains(word))
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
    };
    vocabulary(left).intersection(&vocabulary(right)).count() >= 4
}

fn verify_cross_pass_reproduction(records: &[(String, ReviewRecord)]) -> Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        records.len() == 2,
        "cross-pass reproduction requires two records"
    );
    for (index, (who, record)) in records.iter().enumerate() {
        let other = &records[1 - index].1;
        let findings = review_finding_ids(&record.findings, &record.attempts, who)?;
        for finding_id in &record.independently_reproduced {
            let attempt_id = findings.get(finding_id.as_str()).with_context(|| {
                format!("{who}: reproduced finding {finding_id:?} is not linked")
            })?;
            let attempt = record
                .attempts
                .iter()
                .find(|attempt| attempt.id == *attempt_id)
                .expect("finding linkage validated");
            let matches = other
                .attempts
                .iter()
                .filter(|candidate| {
                    candidate.classification == "verified_defect"
                        && candidate.independently_reproduced
                        && candidate.attack_class == attempt.attack_class
                        && candidate.target == attempt.target
                        && reports_the_same_defect(candidate, attempt)
                })
                .count();
            anyhow::ensure!(
                matches == 1,
                "{who}: reproduced finding {finding_id:?} needs exactly one independently reproduced defect in the other pass with the same attack class, the same source coordinate, and a report about the same defect; found {matches}"
            );
        }
    }
    Ok(())
}

/// One review row against its own committed record (M17.5 F-27 / P1-A05, A06).
///
/// The previous version counted the record's `attempts` array and stopped, so a
/// record of twelve EMPTY objects satisfied it, and a record whose own `result`
/// was `fail` satisfied a packet row claiming `pass`. Everything a reviewer
/// actually concluded was unread.
#[allow(
    clippy::too_many_lines,
    reason = "one review record's contract is one contract; splitting it by line count would scatter the checks a reader must see together"
)]
/// The reviewer's raw answer lives beside its record as `<record>.raw.txt`;
/// its SHA-256 must be the record's `raw_response_sha256` (A09).
fn verify_review_raw_response(
    root: &Utf8Path,
    record_path: &Utf8Path,
    expected: &str,
) -> Result<()> {
    let raw_path = record_path.with_extension("raw.txt");
    let relative = raw_path.strip_prefix(root).unwrap_or(&raw_path);
    let raw = fs::read(&raw_path).with_context(|| {
        format!("{relative}: the reviewer's raw answer must be retained beside its record")
    })?;
    let actual = format!("{:x}", Sha256::digest(&raw));
    require_eq(
        "review raw_response_sha256 (of the retained answer)",
        &actual,
        expected,
    )
}

/// The record's digests: hex-shaped, and the raw one the SHA-256 of the
/// retained answer (A09: a digest of nothing retained bound a record to no
/// answer).
/// A standing ruling on a class of reviewer finding (grilling decision 6,
/// 2026-09-03): a finding Brian rules incorrect is cleared by a ruling
/// commit signed against the pinned signer, never by editing the record.
/// Rulings live in `docs/execution/rulings/*.md` with a front matter of
/// `id`, `attack_class`, `target` (a repository path) and `status`
/// (`draft` or `ruled`). A ruling cannot name its own commit, so the commit
/// that counts is the last one that touched the ruling's file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ruling {
    id: String,
    attack_class: String,
    target: String,
    /// Phrases that must all appear in an attempt's prose for this ruling to
    /// answer it. Blind pass 1 at aa00d41 (A08): matching on class and file
    /// alone cleared an unrelated second claim in the same class and file —
    /// R-001, written for "stage reads are leakage", silently cleared a
    /// genuine untracked-chdir defect. A ruling answers a claim, not a
    /// coordinate.
    claim_requires: Vec<String>,
    /// Phrases whose presence means this ruling does NOT answer the claim.
    /// Blind pass 1 at 5fb1b57 (A08): a required phrase can be found inside a
    /// sentence that negates it, so a ruling could suppress a defect it was
    /// never shown. Required phrases say what a claim must be about;
    /// excluded ones say what it must not be about.
    claim_excludes: Vec<String>,
    status: String,
    file: String,
}

const RULING_SIGNERS: &str = "conformance/haqp/ruling-signers";

fn rulings(root: &Utf8Path) -> Result<Vec<Ruling>> {
    let dir = root.join("docs/execution/rulings");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = fs::read_dir(dir.as_std_path())?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let file = Utf8PathBuf::from_path_buf(file)
            .map_err(|p| anyhow::anyhow!("non-UTF-8 path {}", p.display()))?;
        if file.extension() != Some("md") {
            continue;
        }
        let text = fs::read_to_string(&file)?;
        let front = text
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n"))
            .map(|(front, _)| front)
            .with_context(|| format!("{file}: ruling has no front matter"))?;
        let field = |key: &str| -> Result<String> {
            front
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{key}: ")))
                .map(|v| v.trim().trim_matches('"').to_owned())
                .with_context(|| format!("{file}: ruling front matter lacks {key}"))
        };
        let excludes = field("claim_excludes").unwrap_or_default();
        let requires = field("claim_requires")?;
        anyhow::ensure!(
            !requires.trim().is_empty(),
            "{file}: ruling claim_requires is empty; a ruling that answers any claim in its \
             class and file clears defects it was never shown (A08)"
        );
        out.push(Ruling {
            id: field("id")?,
            attack_class: field("attack_class")?,
            target: field("target")?,
            claim_requires: requires
                .split(';')
                .map(|phrase| phrase.trim().to_ascii_lowercase())
                .filter(|phrase| !phrase.is_empty())
                .collect(),
            claim_excludes: excludes
                .split(';')
                .map(|phrase| phrase.trim().to_ascii_lowercase())
                .filter(|phrase| !phrase.is_empty())
                .collect(),
            status: field("status")?,
            file: file.strip_prefix(root).unwrap_or(&file).to_string(),
        });
    }
    Ok(out)
}

/// A ruling is in force when its status is `ruled` and the last commit that
/// touched its file verifies against the pinned signers. A draft, an
/// uncommitted edit, or an unsigned commit clears nothing.
fn ruling_in_force(root: &Utf8Path, ruling: &Ruling) -> Result<bool> {
    if ruling.status != "ruled" {
        return Ok(false);
    }
    let signers = root.join(RULING_SIGNERS);
    anyhow::ensure!(
        signers.is_file(),
        "{RULING_SIGNERS} is missing; a ruling cannot be verified against no signer"
    );
    let dirty = git_text(root, &["status", "--porcelain", "--", &ruling.file])?;
    if !dirty.trim().is_empty() {
        return Ok(false);
    }
    let commit = git_text(root, &["log", "-1", "--format=%H", "--", &ruling.file])?;
    if commit.trim().is_empty() {
        return Ok(false);
    }
    let verified = Command::new("git")
        .current_dir(root)
        .args(["-c", "gpg.format=ssh", "-c"])
        .arg(format!("gpg.ssh.allowedSignersFile={signers}"))
        .args(["verify-commit", commit.trim()])
        .output()
        .context("git verify-commit")?;
    Ok(verified.status.success())
}

/// The findings a record leaves unresolved after standing rulings: verified,
/// independently reproduced, not resolved by a fix proof, and not covered by
/// a ruling in force on the same attack class and target file. Read as plain
/// JSON so the count is over what was persisted.
pub fn effective_unresolved_findings_repo(root: &Utf8Path, record: &Utf8Path) -> Result<u64> {
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(record)?).with_context(|| format!("parse {record}"))?;
    let in_force = rulings(root)?
        .into_iter()
        .filter(|ruling| ruling_in_force(root, ruling).unwrap_or(false))
        .collect::<Vec<_>>();
    effective_unresolved_findings(&in_force, &value, record)
}

/// The count, given the rulings already established to be in force. Split from
/// the repository walk so the clearance rules can be exercised without a
/// signing key: a rule the tests cannot reach is a rule nothing checks.
fn effective_unresolved_findings(
    in_force: &[Ruling],
    value: &serde_json::Value,
    record: &Utf8Path,
) -> Result<u64> {
    // Counted from each attempt's own flags, not through the findings list:
    // a verified, reproduced attempt the reviewer forgot to list as a finding
    // is still unresolved.
    let mut unresolved = 0u64;
    let mut cleared: BTreeMap<String, u64> = BTreeMap::new();
    for attempt in value["attempts"].as_array().into_iter().flatten() {
        let class = attempt["attack_class"].as_str().unwrap_or_default();
        let target = attempt["target"].as_str().unwrap_or_default();
        if attempt["classification"].as_str() != Some("verified_defect")
            || !attempt["independently_reproduced"]
                .as_bool()
                .unwrap_or(false)
            || attempt["resolved"].as_bool().unwrap_or(false)
        {
            continue;
        }
        let target_file = target.split(':').next().unwrap_or(target);
        let prose = format!(
            "{} {}",
            attempt["attempt"].as_str().unwrap_or_default(),
            attempt["observed_result"].as_str().unwrap_or_default()
        )
        .to_ascii_lowercase();
        // Blind pass 1 at c29bc0ea (A08): a ruling matched on class, file and
        // prose substrings, so an unrelated defect in the same file whose
        // wording happened to carry the required phrases was cleared in
        // silence. Two things changed. A ruling that names a coordinate is
        // held to it, not merely to the file. And a ruling clears at most one
        // attempt per record: matching a second is evidence the phrases are
        // too broad, and the answer to that is a refusal, not two clearances.
        let matched = in_force
            .iter()
            .filter(|ruling| {
                ruling.attack_class == class
                    && if ruling.target.contains(':') {
                        ruling.target == target
                    } else {
                        ruling.target == target_file
                    }
                    && ruling
                        .claim_requires
                        .iter()
                        .all(|phrase| prose.contains(phrase.as_str()))
                    && !ruling
                        .claim_excludes
                        .iter()
                        .any(|phrase| prose.contains(phrase.as_str()))
            })
            .collect::<Vec<_>>();
        for ruling in &matched {
            *cleared.entry(ruling.id.clone()).or_insert(0u64) += 1;
        }
        if matched.is_empty() {
            unresolved += 1;
        }
    }
    for (id, count) in &cleared {
        anyhow::ensure!(
            *count <= 1,
            "ruling {id} cleared {count} separate findings in {record}; a ruling \
                 answers one claim, so phrases matching more than one are too \
                 broad to stand"
        );
    }
    Ok(unresolved)
}

/// The receipt must be the backend's own shape, and a Codex transcript must be
/// retained beside the record and hash to what the receipt claims (F-49).
fn verify_review_provider_receipt(record: &ReviewRecord, record_path: &Utf8Path) -> Result<()> {
    let receipt = &record.provider_receipt;
    require_eq(
        "review provider_receipt backend",
        &receipt.backend,
        &record.reviewer.backend,
    )?;
    match receipt.backend.as_str() {
        "codex" => {
            anyhow::ensure!(
                !receipt.session_id.trim().is_empty() && !receipt.session_file.trim().is_empty(),
                "a codex receipt must name the session that answered"
            );
            require_hex_digest("review transcript_sha256", &receipt.transcript_sha256)?;
            let transcript = record_path.with_extension("session.jsonl");
            let bytes = fs::read(&transcript).with_context(|| {
                format!("{transcript}: the codex transcript must be retained beside its record")
            })?;
            anyhow::ensure!(
                bytes.len() as u64 == receipt.transcript_bytes,
                "codex transcript is {} bytes, receipt claims {}",
                bytes.len(),
                receipt.transcript_bytes
            );
            require_eq(
                "review transcript_sha256 (of the retained transcript)",
                &format!("{:x}", Sha256::digest(&bytes)),
                &receipt.transcript_sha256,
            )?;
            // Blind pass 1 at `aeed70f9` (A09): the transcript's bytes were
            // authenticated and its IDENTITY was not — `session_id` and
            // `session_file` only had to be non-empty, so a record could name
            // one session and retain another's rollout. A rollout opens with
            // its own `session_meta`, and the receipt's id ends with the UUID
            // that record carries.
            let first = std::str::from_utf8(&bytes)
                .context("codex transcript is not UTF-8")?
                .lines()
                .next()
                .context("codex transcript is empty")?;
            let meta: serde_json::Value = serde_json::from_str(first)
                .context("codex transcript's first record is not JSON")?;
            anyhow::ensure!(
                meta["type"] == "session_meta",
                "a codex rollout opens with session_meta; this one opens with {:?}",
                meta["type"]
            );
            let declared = meta["payload"]["session_id"]
                .as_str()
                .context("codex session_meta carries no session_id")?;
            // Codex names a rollout `<timestamp>-<uuid>` and its `session_meta`
            // carries the bare uuid, so the receipt's id either IS the declared
            // id or ends with it after a separator. Review of `bba803d4`: a
            // bare `ends_with` would also accept `xfixture` for `fixture`,
            // which is a different session.
            anyhow::ensure!(
                receipt.session_id == declared
                    || receipt.session_id.ends_with(&format!("-{declared}")),
                "receipt names session {:?}, retained transcript is session {declared:?}",
                receipt.session_id
            );
            require_eq(
                "review provider_receipt session_file",
                &receipt.session_file,
                &format!("rollout-{}.jsonl", receipt.session_id),
            )
        }
        "mimo-direct" => {
            anyhow::ensure!(
                !receipt.response_id.trim().is_empty(),
                "a mimo receipt must carry the provider's response id"
            );
            anyhow::ensure!(
                !receipt.provider_model.trim().is_empty(),
                "a mimo receipt must carry the model the provider says answered"
            );
            anyhow::ensure!(
                receipt.created > 0,
                "a mimo receipt must carry the provider's clock"
            );
            for key in ["prompt_tokens", "completion_tokens", "total_tokens"] {
                anyhow::ensure!(
                    receipt.usage.contains_key(key),
                    "a mimo receipt must carry the provider's {key}; a changed usage envelope \
                     must be seen here, not discovered later through a binding that moved"
                );
            }
            let billed = receipt
                .usage
                .get("total_tokens")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            anyhow::ensure!(
                billed > 0,
                "a mimo receipt must carry billed usage; {:?} bills nothing",
                receipt.usage
            );
            Ok(())
        }
        other => anyhow::bail!(
            "review backend {other:?} produced no provider receipt; ADR-0020 §6 independence \
             rests on evidence from outside this runner (F-49)"
        ),
    }
}

/// SHA-256 of a record's `provider_receipt` exactly as persisted.
fn review_receipt_sha256(record_path: &Utf8Path) -> Result<String> {
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(record_path).with_context(|| format!("read {record_path}"))?,
    )
    .with_context(|| format!("parse {record_path}"))?;
    Ok(sha256_text(&serde_json::to_string(
        &value
            .get("provider_receipt")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )?))
}

/// The commands a resolution receipt may claim, and the gate RUNS the one it
/// claims (A09/A10 at 9eca4f0).
///
/// Before this, a receipt naming any command with a self-authored
/// `verification_exit_code: 0` was accepted without the command ever being
/// executed — the resolution asserted its own success. The command is drawn
/// from a closed registry rather than run as written: executing arbitrary text
/// out of an evidence file would hand whoever wrote the file this process.
const RESOLUTION_COMMANDS: [(&str, &[&str]); 4] = [
    ("just ci", &["just", "ci"]),
    ("just haq-inventory", &["just", "haq-inventory"]),
    ("just haq-canaries", &["just", "haq-canaries"]),
    (
        "cargo nextest run --workspace --all-features",
        &["cargo", "nextest", "run", "--workspace", "--all-features"],
    ),
];

/// The registry's commands are the project's own gates, run with the gate's
/// environment and no timeout: they are what CI runs, so a hang here is a hang
/// in CI, visible either way (review of `e490671`). The working directory is a
/// disposable worktree at the commit the receipt names, never the live tree.
fn verify_resolution_command_ran(
    root: &Utf8Path,
    who: &str,
    evidence_text: &str,
    commit: &str,
) -> Result<()> {
    let claimed = evidence_text
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("verification_command:"))
        .map(str::trim)
        .unwrap_or_default();
    let argv = RESOLUTION_COMMANDS
        .iter()
        .find(|(name, _)| *name == claimed)
        .map(|(_, argv)| *argv)
        .with_context(|| {
            format!(
                "{who}: resolution claims verification command {claimed:?}, which is not one the \
                 gate can run; the registry is closed so a receipt cannot name a command that \
                 exists only in its own text"
            )
        })?;
    let (program, rest) = argv.split_first().context("registry command is empty")?;
    // Blind pass 1 at aecb2ec7 (A12): the command ran against the working
    // tree, so a receipt claiming a finding was resolved at some commit was
    // answered by whether the command passes at HEAD. Reachability was
    // checked and the commit then ignored. It runs in a disposable worktree
    // checked out at the claimed commit now, which is what the receipt says.
    let scratch = liminal_scratch::ScratchDir::new("haq-resolution-replay")?;
    let worktree = scratch.path().to_owned();
    let add = Command::new("git")
        .current_dir(root)
        .args(["worktree", "add", "--detach", "--quiet"])
        .arg(&worktree)
        .arg(commit)
        .output()?;
    anyhow::ensure!(
        add.status.success(),
        "{who}: resolution replay worktree add at {commit} failed: {}",
        String::from_utf8_lossy(&add.stderr).trim()
    );
    let mut guard = WorktreeGuard::new(root, &worktree);
    let status = Command::new(program)
        .current_dir(&worktree)
        .args(rest)
        .status()
        .with_context(|| format!("{who}: run resolution verification {claimed:?} at {commit}"))?;
    guard.remove()?;
    anyhow::ensure!(
        status.success(),
        "{who}: resolution claims {claimed:?} exited zero at {commit}; it exited {status}"
    );
    Ok(())
}

fn verify_review_record_digests(
    root: &Utf8Path,
    record: &ReviewRecord,
    record_path: &Utf8Path,
) -> Result<()> {
    require_hex_digest(
        "review sanitized_prompt_hash",
        &record.sanitized_prompt_hash,
    )?;
    require_hex_digest("review raw_response_sha256", &record.raw_response_sha256)?;
    verify_review_raw_response(root, record_path, &record.raw_response_sha256)?;
    verify_review_provider_receipt(record, record_path)?;
    require_hex_digest(
        "review integrity_binding_sha256",
        &record.integrity_binding_sha256,
    )?;
    anyhow::ensure!(
        record.schema_retries <= 3,
        "review record claims {} schema retries; the runner allows at most 3",
        record.schema_retries
    );
    Ok(())
}

fn verify_review_record(
    root: &Utf8Path,
    review: &Review,
    record: &ReviewRecord,
    record_path: &Utf8Path,
) -> Result<()> {
    let who = &review.reviewer;
    require_eq(
        "review record model_family",
        &record.reviewer.model_family,
        who,
    )?;
    require_eq(
        "review record schema_version",
        &record.schema_version,
        "haqp-blind-review-v1",
    )?;
    anyhow::ensure!(
        !record.reviewer.model_family.trim().is_empty()
            && !record.reviewer.backend.trim().is_empty(),
        "{who}: review record omits model family or backend"
    );
    let expected_identity = sha256_text(&format!(
        "pass{}:{}:haqp-blind-review-v1",
        record.pass, record.reviewer.model_family
    ));
    require_eq(
        "review identity_hash",
        &record.reviewer.identity_hash,
        &expected_identity,
    )?;
    verify_review_record_digests(root, record, record_path)?;
    require_eq(
        "review prompt_binding_sha256",
        &record.prompt_binding_sha256,
        &sha256_text(&format!(
            "haqp-blind-review-v1\0{}\0{}\0{}\0{}",
            record.pass,
            record.reviewer.model_family,
            record.fixed_base.commit,
            record.fixed_base.tree
        )),
    )?;
    require_git_object_id("review fixed_base.commit", &record.fixed_base.commit)?;
    require_git_object_id("review fixed_base.tree", &record.fixed_base.tree)?;
    if record.attempts.len() as u64 != review.attempts {
        anyhow::bail!(
            "{who} declares {} attempts but its record contains {}",
            review.attempts,
            record.attempts.len()
        );
    }
    verify_review_attempts(root, &record.fixed_base.commit, who, &record.attempts)?;
    verify_review_attack_classes(record, who)?;
    // P1-A06: the packet must not summarize the record more kindly than the
    // record summarizes itself. A row claiming `pass` over a record that says
    // `fail` is the whole failure mode.
    if record.result != review.result {
        anyhow::bail!(
            "{who} declares result {:?} but its record says {:?}",
            review.result,
            record.result
        );
    }
    if record.unresolved_verified_findings != review.unresolved_verified_findings {
        anyhow::bail!(
            "{who} declares {} unresolved verified findings but its record counts {}",
            review.unresolved_verified_findings,
            record.unresolved_verified_findings
        );
    }
    verify_review_findings(review, record, who, root)?;
    // P1-A07: blindness is a property of how the pass was RUN. The runner emits
    // this proof; before F-27 nothing read it back.
    if record.blindness_proof.prior_pass_artifact_supplied {
        anyhow::bail!("{who} was shown a prior pass's artifacts; ADR-0020 §6 requires blindness");
    }
    if record.pass == 2 && !record.blindness_proof.pass_two_original_spec_only {
        anyhow::bail!("{who} is pass 2 but did not start from the original spec alone");
    }
    anyhow::ensure!(
        matches!(
            record.blindness_proof.session_state.as_str(),
            "fresh-codex-session" | "ephemeral-cloud-session"
        ),
        "{who} has unknown blindness session state {:?}",
        record.blindness_proof.session_state
    );
    let expected_ephemeral = record.blindness_proof.session_state == "ephemeral-cloud-session";
    anyhow::ensure!(
        record.blindness_proof.ephemeral_session_requested == expected_ephemeral,
        "{who} blindness session state disagrees with ephemeral_session_requested"
    );
    // Blind pass 1 at 78c8f9b (A11): the binding covered the verdict and the
    // raw digest but not the attempts and findings, so those could be edited
    // under an intact binding. Version 2 binds the structured claims as
    // persisted, through the same canonical JSON the runner hashes.
    require_eq(
        "review integrity_binding_sha256",
        &record.integrity_binding_sha256,
        &sha256_text(&format!(
            "haqp-review-integrity-v3\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            record.pass,
            record.reviewer.model_family,
            record.fixed_base.commit,
            record.fixed_base.tree,
            record.prompt_binding_sha256,
            record.raw_response_sha256,
            review_claims_sha256(record_path)?,
            review_receipt_sha256(record_path)?,
            record.result,
            record.unresolved_verified_findings
        )),
    )?;
    Ok(())
}

/// SHA-256 of the record's persisted attempts, findings and reproduced ids in
/// canonical JSON (sorted keys, compact separators, UTF-8 unescaped), read from
/// the file rather than the typed struct so the bytes are what was written.
fn review_claims_sha256(record_path: &Utf8Path) -> Result<String> {
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(record_path).with_context(|| format!("read {record_path}"))?,
    )
    .with_context(|| format!("parse {record_path}"))?;
    let claims = serde_json::json!({
        "attempts": value.get("attempts").cloned().unwrap_or(serde_json::Value::Null),
        "findings": value.get("findings").cloned().unwrap_or(serde_json::Value::Null),
        "independently_reproduced": value
            .get("independently_reproduced")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    });
    Ok(sha256_text(&serde_json::to_string(&claims)?))
}

const REVIEW_ATTACK_CLASSES: [&str; 9] = [
    "vacuity",
    "shared-oracle coupling",
    "missing negatives",
    "weak mutants",
    "fault omissions",
    "nondeterminism",
    "corpus leakage",
    "exception broadening",
    "evidence/report drift",
];

fn verify_review_attack_classes(record: &ReviewRecord, who: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for attempt in &record.attempts {
        anyhow::ensure!(
            REVIEW_ATTACK_CLASSES.contains(&attempt.attack_class.as_str()),
            "{who}: attempt {} uses unknown attack class {:?}",
            attempt.id,
            attempt.attack_class
        );
        seen.insert(attempt.attack_class.as_str());
    }
    if record.pass == 1 {
        let missing = REVIEW_ATTACK_CLASSES
            .iter()
            .filter(|class| !seen.contains(**class))
            .copied()
            .collect::<Vec<_>>();
        anyhow::ensure!(
            missing.is_empty(),
            "{who}: Pass 1 is missing attack classes {missing:?}"
        );
    }
    Ok(())
}

fn verify_review_findings(
    review: &Review,
    record: &ReviewRecord,
    who: &str,
    root: &Utf8Path,
) -> Result<()> {
    require_hex_digest(
        "review isolated_session_hash",
        &record.isolated_session_hash,
    )?;
    let record_findings = review_finding_ids(&record.findings, &record.attempts, who)?;
    let packet_findings = review
        .findings
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if record_findings.len() != review.findings.len() {
        anyhow::bail!(
            "{who} lists {} findings but its record contains {}",
            review.findings.len(),
            record.findings.len()
        );
    }
    if record_findings.keys().copied().collect::<BTreeSet<_>>() != packet_findings {
        anyhow::bail!("{who} packet findings do not match its committed record");
    }
    let mut reproduced = BTreeSet::new();
    for finding_id in &record.independently_reproduced {
        anyhow::ensure!(
            reproduced.insert(finding_id.as_str()),
            "{who}: independently reproduced finding id {finding_id:?} appears twice"
        );
        anyhow::ensure!(
            record_findings.contains_key(finding_id.as_str()),
            "{who}: independently reproduced finding {finding_id:?} is absent from record"
        );
        let attempt_id = record_findings[finding_id.as_str()];
        let attempt = record
            .attempts
            .iter()
            .find(|attempt| attempt.id == attempt_id)
            .expect("finding linkage validated above");
        anyhow::ensure!(
            attempt.independently_reproduced,
            "{who}: finding {finding_id:?} is independently reproduced but its attempt says false"
        );
    }
    let packet_reproduced = review
        .independently_reproduced
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        reproduced == packet_reproduced,
        "{who} packet independent reproductions do not match its committed record"
    );
    anyhow::ensure!(
        reproduced.len() == record_findings.len(),
        "{who}: every finding must have independent reproduction; reproduced {} of {}",
        reproduced.len(),
        record_findings.len()
    );
    for attempt in &record.attempts {
        if attempt.classification == "verified_defect" {
            let linked = record_findings
                .iter()
                .filter(|(_, attempt_id)| **attempt_id == attempt.id)
                .map(|(finding_id, _)| *finding_id)
                .collect::<BTreeSet<_>>();
            anyhow::ensure!(
                linked
                    .iter()
                    .any(|finding_id| reproduced.contains(finding_id)),
                "{who}: verified defect attempt {:?} lacks independent reproduction",
                attempt.id
            );
            verify_review_resolution(root, &record.fixed_base, who, attempt)?;
        }
    }
    let verified_unresolved = record
        .independently_reproduced
        .iter()
        .filter(|finding_id| {
            let Some(attempt_id) = record_findings.get(finding_id.as_str()) else {
                return false;
            };
            record.attempts.iter().any(|attempt| {
                attempt.id == *attempt_id
                    && attempt.classification == "verified_defect"
                    && !attempt.resolved
            })
        })
        .count() as u64;
    if record.unresolved_verified_findings != verified_unresolved {
        anyhow::bail!(
            "{who} record counts {} unresolved verified findings but its attempts prove {verified_unresolved}",
            record.unresolved_verified_findings
        );
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "each attempt field is checked once, in order; the length is the field count"
)]
fn verify_review_attempts(
    root: &Utf8Path,
    fixed_commit: &str,
    who: &str,
    attempts: &[ReviewRecordAttempt],
) -> Result<()> {
    // P1-A05: the attempts must be attempt RECORDS, not array padding.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut seen_attempts = BTreeSet::new();
    for attempt in attempts {
        for (field, value) in [
            ("id", &attempt.id),
            ("attack_class", &attempt.attack_class),
            ("target", &attempt.target),
            ("attempt", &attempt.attempt),
            ("observed_result", &attempt.observed_result),
            ("classification", &attempt.classification),
        ] {
            if value.trim().is_empty() {
                anyhow::bail!("{who}: attempt {:?} has an empty {field}", attempt.id);
            }
        }
        if !matches!(
            attempt.classification.as_str(),
            "verified_defect" | "false_positive" | "caught_violation"
        ) {
            anyhow::bail!(
                "{who}: attempt {:?} has unknown classification {:?}",
                attempt.id,
                attempt.classification
            );
        }
        if !seen.insert(attempt.id.as_str()) {
            anyhow::bail!("{who}: attempt id {:?} appears twice", attempt.id);
        }
        if attempt.classification == "false_positive" && !attempt.independently_reproduced {
            anyhow::bail!(
                "{who}: false-positive attempt {:?} lacks independent reproduction evidence",
                attempt.id
            );
        }
        for (field, value) in [
            ("attempt", attempt.attempt.as_str()),
            ("observed_result", attempt.observed_result.as_str()),
        ] {
            let words = value.split_whitespace().count();
            anyhow::ensure!(
                value.trim().len() >= 24 && words >= 4,
                "{who}: attempt {:?} {field} is not a substantive falsification record",
                attempt.id
            );
        }
        let (target_file, target_coordinate) =
            attempt.target.split_once(':').with_context(|| {
                format!(
                    "{who}: attempt {:?} target lacks file:coordinate",
                    attempt.id
                )
            })?;
        anyhow::ensure!(
            !target_file.trim().is_empty()
                && !target_coordinate.trim().is_empty()
                && !target_file.starts_with('/')
                && !target_file.split('/').any(|part| part == ".."),
            "{who}: attempt {:?} target must be a safe file:coordinate",
            attempt.id
        );
        anyhow::ensure!(
            contains_exact_coordinate(&attempt.attempt, &attempt.target),
            "{who}: attempt {:?} falsification claim must quote exact target {}",
            attempt.id,
            attempt.target
        );
        anyhow::ensure!(
            contains_exact_coordinate(&attempt.observed_result, &attempt.target),
            "{who}: attempt {:?} observation must quote exact target {}",
            attempt.id,
            attempt.target
        );
        let target_lower = target_file.to_ascii_lowercase();
        anyhow::ensure!(
            !is_locked_acceptance_path(&target_lower),
            "{who}: attempt {:?} target may not name locked acceptance corpus paths",
            attempt.id
        );
        let signature = format!(
            "{}\0{}\0{}\0{}",
            attempt.attack_class, attempt.target, attempt.attempt, attempt.observed_result
        );
        anyhow::ensure!(
            seen_attempts.insert(signature),
            "{who}: attempt {:?} duplicates another substantive falsification attempt",
            attempt.id
        );
        let target_source = git_text(
            root,
            &["show", &format!("{fixed_commit}:{target_file}")],
        )
        .with_context(|| {
            format!(
                "{who}: attempt {:?} target file {target_file:?} is absent at fixed review base",
                attempt.id
            )
        })?;
        let line: usize = target_coordinate.parse().with_context(|| {
            format!(
                "{who}: attempt {:?} target coordinate must be a positive line number",
                attempt.id
            )
        })?;
        anyhow::ensure!(
            line > 0 && line <= target_source.lines().count().max(1),
            "{who}: attempt {:?} target line {line} is outside {target_file}",
            attempt.id
        );
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn verify_review_resolution(
    root: &Utf8Path,
    fixed_base: &ReviewFixedBase,
    who: &str,
    attempt: &ReviewRecordAttempt,
) -> Result<()> {
    if !attempt.resolved {
        return Ok(());
    }
    let resolution = attempt.resolution.as_ref().with_context(|| {
        format!(
            "{who}: resolved finding attempt {:?} has no resolution proof",
            attempt.id
        )
    })?;
    require_git_object_id(&format!("{who} resolution commit"), &resolution.commit)?;
    let fixed_oid = git_text(root, &["rev-parse", &fixed_base.commit])?;
    let resolution_oid = git_text(root, &["rev-parse", &resolution.commit])?;
    anyhow::ensure!(
        fixed_oid != resolution_oid,
        "{who}: resolution commit must be a strict descendant of fixed base"
    );
    git_is_ancestor(root, &fixed_base.commit, &resolution.commit).with_context(|| {
        format!(
            "{who}: resolution commit {} is not a descendant of fixed base {}",
            resolution.commit, fixed_base.commit
        )
    })?;
    git_is_ancestor(root, &resolution.commit, "HEAD").with_context(|| {
        format!(
            "{who}: resolution commit {} is not reachable from current qualification HEAD",
            resolution.commit
        )
    })?;
    let evidence_path = safe_repo_path(root, &resolution.evidence_path, who)?;
    let evidence_bytes = fs::read(&evidence_path)
        .with_context(|| format!("{evidence_path}: resolution evidence is missing"))?;
    let evidence_text = String::from_utf8_lossy(&evidence_bytes);
    anyhow::ensure!(
        evidence_text.contains(&attempt.id)
            && evidence_text.contains(&attempt.target)
            && evidence_text.contains(&resolution.coordinate),
        "{who}: resolution evidence must name attempted finding {}, target {}, and fix coordinate {}",
        attempt.id,
        attempt.target,
        resolution.coordinate
    );
    anyhow::ensure!(
        evidence_text
            .lines()
            .any(|line| line.trim() == "resolution_result: pass")
            && evidence_text
                .lines()
                .any(|line| line.trim_start().starts_with("verification_command:"))
            && evidence_text
                .lines()
                .any(|line| line.trim() == "verification_exit_code: 0"),
        "{who}: resolution evidence must include a passing verification command receipt"
    );
    verify_resolution_command_ran(root, who, &evidence_text, &resolution.commit)?;
    let mut hasher = Sha256::new();
    hasher.update(&evidence_bytes);
    let digest = format!("{:x}", hasher.finalize());
    require_eq(
        "resolution evidence_sha256",
        &resolution.evidence_sha256,
        &digest,
    )?;
    let committed_bytes = git_blob(root, &resolution.commit, &resolution.evidence_path)
        .with_context(|| {
            format!(
                "{who}: resolution evidence {} is absent at commit {}",
                resolution.evidence_path, resolution.commit
            )
        })?;
    let mut committed_hasher = Sha256::new();
    committed_hasher.update(&committed_bytes);
    let committed_digest = format!("{:x}", committed_hasher.finalize());
    require_eq(
        "resolution committed evidence_sha256",
        &resolution.evidence_sha256,
        &committed_digest,
    )?;
    anyhow::ensure!(
        !resolution.coordinate.trim().is_empty(),
        "{who}: resolved finding attempt {:?} has empty fix coordinate",
        attempt.id
    );
    require_hex_digest(
        &format!("{who} resolution evidence_sha256"),
        &resolution.evidence_sha256,
    )?;
    let (coordinate_file, coordinate_line) = resolution
        .coordinate
        .rsplit_once(':')
        .with_context(|| format!("{who}: resolution coordinate must be file:line"))?;
    let target_file = attempt
        .target
        .rsplit_once(':')
        .map(|(file, _)| file)
        .unwrap_or_default();
    anyhow::ensure!(
        coordinate_file == target_file,
        "{who}: resolution coordinate file {coordinate_file:?} does not match attacked target file {target_file:?}"
    );
    let line: usize = coordinate_line
        .parse()
        .with_context(|| format!("{who}: resolution coordinate line is not numeric"))?;
    anyhow::ensure!(
        line > 0,
        "{who}: resolution coordinate line must be positive"
    );
    anyhow::ensure!(
        !coordinate_file.trim().is_empty()
            && !coordinate_file.starts_with('/')
            && !coordinate_file.split('/').any(|part| part == ".."),
        "{who}: resolution coordinate path must stay inside repository"
    );
    anyhow::ensure!(
        !is_locked_acceptance_path(coordinate_file),
        "{who}: resolution coordinate may not name locked acceptance corpus"
    );
    let source = git_text(
        root,
        &["show", &format!("{}:{coordinate_file}", resolution.commit)],
    )?;
    anyhow::ensure!(
        line <= source.lines().count().max(1),
        "{who}: resolution coordinate line {line} is outside {coordinate_file}"
    );
    verify_resolution_coordinate_changed(
        root,
        &fixed_base.commit,
        &resolution.commit,
        who,
        coordinate_file,
        line,
    )?;
    Ok(())
}

fn git_blob(root: &Utf8Path, commit: &str, path: &str) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["show", &format!("{commit}:{path}")])
        .output()
        .with_context(|| format!("read {commit}:{path} from Git"))?;
    anyhow::ensure!(
        output.status.success(),
        "git show {commit}:{path} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout)
}

fn verify_resolution_coordinate_changed(
    root: &Utf8Path,
    fixed_commit: &str,
    resolution_commit: &str,
    who: &str,
    coordinate_file: &str,
    line: usize,
) -> Result<()> {
    let range = format!("{fixed_commit}..{resolution_commit}");
    let changed = git_text(
        root,
        &["diff", "--name-only", &range, "--", coordinate_file],
    )?;
    anyhow::ensure!(
        changed.lines().any(|path| path == coordinate_file),
        "{who}: resolution commit did not change coordinate file {coordinate_file}"
    );
    let diff = git_text(
        root,
        &["diff", "--unified=0", &range, "--", coordinate_file],
    )?;
    anyhow::ensure!(
        diff_contains_added_line(&diff, line),
        "{who}: resolution coordinate {coordinate_file}:{line} is outside changed hunks"
    );
    Ok(())
}

fn review_finding_ids<'a>(
    findings: &'a [serde_json::Value],
    attempts: &'a [ReviewRecordAttempt],
    who: &str,
) -> Result<BTreeMap<&'a str, &'a str>> {
    let attempts_by_id = attempts
        .iter()
        .map(|attempt| (attempt.id.as_str(), attempt))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeMap::new();
    for finding in findings {
        let id = finding
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .with_context(|| format!("{who}: record finding has no non-empty id"))?;
        let attempt_id = finding
            .get("attempt_id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .with_context(|| format!("{who}: finding {id:?} has no non-empty attempt_id"))?;
        let attempt = attempts_by_id.get(attempt_id).with_context(|| {
            format!("{who}: finding {id:?} names unknown attempt {attempt_id:?}")
        })?;
        anyhow::ensure!(
            attempt.classification == "verified_defect",
            "{who}: finding {id:?} must link to a verified_defect attempt"
        );
        anyhow::ensure!(
            ids.insert(id, attempt_id).is_none(),
            "{who}: record finding id {id:?} appears twice"
        );
    }
    Ok(ids)
}

fn require_hex_digest(label: &str, digest: &str) -> Result<()> {
    anyhow::ensure!(
        digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()),
        "{label} must be a 64-hex digest"
    );
    Ok(())
}

fn require_git_object_id(label: &str, digest: &str) -> Result<()> {
    anyhow::ensure!(
        matches!(digest.len(), 40 | 64) && digest.chars().all(|ch| ch.is_ascii_hexdigit()),
        "{label} must be a 40- or 64-hex Git object id"
    );
    Ok(())
}

fn git_is_ancestor(root: &Utf8Path, ancestor: &str, descendant: &str) -> Result<()> {
    let status = Command::new("git")
        .current_dir(root)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .status()
        .with_context(|| format!("check Git ancestry {ancestor} -> {descendant}"))?;
    anyhow::ensure!(
        status.success(),
        "Git ancestry check failed for {ancestor} -> {descendant}"
    );
    Ok(())
}

fn safe_repo_path(root: &Utf8Path, value: &str, who: &str) -> Result<Utf8PathBuf> {
    let candidate = Utf8Path::new(value);
    anyhow::ensure!(
        !candidate.is_absolute()
            && !candidate
                .components()
                .any(|part| part == camino::Utf8Component::ParentDir),
        "{who} resolution evidence path {value:?} escapes repository"
    );
    anyhow::ensure!(
        !is_locked_acceptance_path(value),
        "{who} resolution evidence path {value:?} names locked acceptance corpus"
    );
    let path = root.join(candidate);
    let canonical_root =
        fs::canonicalize(root.as_std_path()).context("canonicalize repository root")?;
    let canonical_path = fs::canonicalize(&path)
        .with_context(|| format!("{path}: resolution evidence is missing"))?;
    anyhow::ensure!(
        canonical_path.starts_with(&canonical_root),
        "{who} resolution evidence path resolves outside repository"
    );
    Utf8PathBuf::from_path_buf(canonical_path)
        .map_err(|_| anyhow::anyhow!("{path}: resolution evidence path is not UTF-8"))
}

fn is_locked_acceptance_path(value: &str) -> bool {
    let components = value
        .split(['/', '\\'])
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    components.iter().any(|component| component == "heldout")
        || components
            .windows(2)
            .any(|pair| pair == ["conformance", "corpora"])
}

fn diff_contains_added_line(diff: &str, target_line: usize) -> bool {
    diff.lines()
        .filter(|line| line.starts_with("@@ "))
        .any(|header| {
            let Some(range) = header
                .split_whitespace()
                .find(|token| token.starts_with('+'))
                .and_then(|token| token.strip_prefix('+'))
            else {
                return false;
            };
            let (start, count) = range
                .split_once(',')
                .map_or((range, "1"), |(start, count)| (start, count));
            let Ok(start) = start.parse::<usize>() else {
                return false;
            };
            let Ok(count) = count.parse::<usize>() else {
                return false;
            };
            count > 0 && target_line >= start && target_line.saturating_sub(start) < count
        })
}

/// ADR-0020 §6's independence, checked rather than assumed (M17.5 F-27 / P1-A07).
///
/// The runner records `reviewer.model_family`, `reviewer.backend` and an
/// identity hash precisely so independence is auditable. The qualification
/// verifier never looked at any of them — this session BUILT that evidence and
/// then failed to check it, which is the same mistake as trusting a status
/// string. Two reviews from one family satisfy every other check in this file.
fn verify_reviewer_independence(records: &[(String, ReviewRecord)]) -> Result<()> {
    // M17.5 Stage 3: `< 2` -> `> 2` survives here and is EQUIVALENT, not
    // untested. With fewer than two records the loop below builds sets of size
    // `records.len()`, and `distinct != records.len()` is then false, so the
    // function returns Ok either way. The early return states the intent —
    // independence is a claim about a PAIR — and costs nothing.
    if records.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        records.len() == 2,
        "blind review evidence must contain exactly two records"
    );
    let passes = records
        .iter()
        .map(|(_, record)| record.pass)
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        passes == BTreeSet::from([1, 2]),
        "blind review records must have exactly pass ordinals 1 and 2"
    );
    let mut families: BTreeSet<&str> = BTreeSet::new();
    let mut backends: BTreeSet<&str> = BTreeSet::new();
    let mut identities: BTreeSet<&str> = BTreeSet::new();
    let mut prompts: BTreeSet<&str> = BTreeSet::new();
    let mut sessions: BTreeSet<&str> = BTreeSet::new();
    for (who, record) in records {
        if record.reviewer.model_family.trim().is_empty() {
            anyhow::bail!("{who}: record names no model family");
        }
        if record.reviewer.identity_hash.trim().is_empty() {
            anyhow::bail!("{who}: record carries no reviewer identity hash");
        }
        require_hex_digest(
            &format!("{who} reviewer identity_hash"),
            &record.reviewer.identity_hash,
        )?;
        require_hex_digest(
            &format!("{who} sanitized_prompt_hash"),
            &record.sanitized_prompt_hash,
        )?;
        require_hex_digest(
            &format!("{who} isolated_session_hash"),
            &record.isolated_session_hash,
        )?;
        families.insert(record.reviewer.model_family.as_str());
        backends.insert(record.reviewer.backend.as_str());
        identities.insert(record.reviewer.identity_hash.as_str());
        prompts.insert(record.sanitized_prompt_hash.as_str());
        sessions.insert(record.isolated_session_hash.as_str());
    }
    for (label, distinct) in [
        ("model families", families.len()),
        ("backends", backends.len()),
        ("reviewer identities", identities.len()),
        ("sanitized prompts", prompts.len()),
        ("isolated sessions", sessions.len()),
    ] {
        if distinct != records.len() {
            anyhow::bail!(
                "{} reviews share {label} ({distinct} distinct across {} reviews); \
                 ADR-0020 §6 requires distinct model families with isolated state",
                records.len(),
                records.len()
            );
        }
    }
    Ok(())
}

/// Every declared test name must name a test that EXISTS (M17.5 pass-2 #11).
///
/// `Test.name` was carried as an opaque string and never checked, so renaming a
/// row to `does_not_exist` changed nothing — the traceability matrix could name
/// 38 tests none of which were real.
///
/// This runs only at the QUALIFIED layer, not the inventory layer, and that
/// asymmetry is deliberate: the proposed packet legitimately predeclares tests
/// that milestones M19–M24 have not yet written, which is the whole point of an
/// inventory. But a packet claiming `qualification_state: complete` while
/// naming a test nobody wrote is exactly the false green this gate exists to
/// stop.
/// The declared stage must match what the packet actually claims (ADR-0021).
///
/// HAQP-1 is evaluated in two stages because ADR-0020 §3 assumed the suite it
/// mutates already exists, and for Phase 1 it does not (F-28). Stage `1a` is
/// everything provable before Phase 1 authorization; `1b` adds the mutation
/// requirement at M24.
///
/// Both directions are errors. A `1a` packet claiming kills would smuggle the
/// deferred requirement back in as an unwitnessed assertion — the exact thing
/// the deferral exists to prevent. A `1b` packet without kills would claim the
/// stronger stage while supplying the weaker evidence.
fn verify_qualification_stage(packet: &Packet) -> Result<()> {
    let claims_resolved = packet
        .mutants
        .iter()
        .any(|mutant| mutant.disposition != "predeclared");
    match packet.qualification_stage.as_str() {
        "1a" if claims_resolved => anyhow::bail!(
            "packet declares stage 1a but claims evaluated mutants; ADR-0021 defers the \
             mutation requirement to 1b at M24, when the runner can produce evidence"
        ),
        "1b" if !claims_resolved => anyhow::bail!(
            "packet declares stage 1b, which carries ADR-0020 §3's mutation \
             requirement, but no mutant is killed or otherwise evaluated"
        ),
        "1a" | "1b" => Ok(()),
        other => anyhow::bail!("unknown qualification stage {other:?}; expected 1a or 1b"),
    }
}

/// A mutant claiming `killed` must be killed by a test that CAN RUN
/// (M17.5 F-27 / P1-A02, F-28).
///
/// ADR-0020 §4 wants at least 64 semantic mutants at a 100% kill rate. The
/// packet declares 65, each naming the tests that would catch it — and as of
/// 2026-08-03 **not one of those 35 distinct tests is runnable**: 27 exist only
/// as strings in the packet (M18–M24 have not been authorized, so
/// `conformance/tests/milestones/` has no m18..m24 at all), and the remaining 8
/// exist but are `#[ignore]`d under the AM-17.2 Phase 1 quarantine.
///
/// A kill demonstrated by a test that does not exist is not demonstrated. A
/// kill demonstrated by a test nobody runs is not demonstrated either — an
/// `#[ignore]`d test cannot fail, so it cannot witness anything.
///
/// This check exists so that gap cannot be closed by editing dispositions to
/// `killed`. It is deliberately unsatisfiable today, and that is the correct
/// state: the mutation requirement is GATED on the Phase 1 milestones writing
/// their tests, not on anything M17.5 can produce.
/// How each mutation operator is observed. AM-17.10: a killer is derived, not
/// declared, and the primary killer is the test whose evidence kind is the way
/// this operator's defect shows itself. Closed table — a new operator must
/// name its observation here before the plan can carry it.
const MUTANT_OBSERVATION_KIND: [(&str, &str); 13] = [
    ("broadened-allow-list", "negative"),
    ("disabled-crash-point", "recovery"),
    ("missing-enum-dispatch", "malformed"),
    ("oracle-short-circuit", "positive"),
    ("ordering-nondeterminism", "replay"),
    ("predicate-deletion", "positive"),
    ("predicate-inversion", "negative"),
    ("skipped-durable-transition", "recovery"),
    ("stale-basis-acceptance", "basis"),
    ("success-error-substitution", "negative"),
    ("threshold-minus-one", "malformed"),
    ("threshold-plus-one", "malformed"),
    ("wrong-holder-selection", "basis"),
];

/// The killing tests AM-17.10 derives for `mutant`: the candidates are the
/// tests the packet maps to its requirement, in packet order; the primary is
/// the first candidate observed the way this operator shows itself; the
/// secondary is the first remaining candidate of a different evidence kind.
/// Each fallback is to the next candidate, never outside the requirement.
fn derive_killing_tests(packet: &Packet, mutant: &Mutant) -> Result<Vec<String>> {
    // Position among the mutants sharing this requirement. F-05 caps any one
    // test at a quarter of the plan, and a rule that always picked the first
    // apt candidate put P1-T07 on 46% of it. Rotation spreads the claim across
    // the tests that defend the requirement instead of piling it on the first.
    let ordinal = packet
        .mutants
        .iter()
        .take_while(|other| other.id != mutant.id)
        .filter(|other| other.requirement == mutant.requirement)
        .count();
    let observed = MUTANT_OBSERVATION_KIND
        .iter()
        .find(|(operator, _)| *operator == mutant.operator)
        .map(|(_, kind)| *kind)
        .with_context(|| {
            format!(
                "{} carries operator {:?}, which names no observation kind; AM-17.10 requires \
                 one before the plan can carry the operator",
                mutant.id, mutant.operator
            )
        })?;
    let candidates: Vec<&Test> = packet
        .tests
        .iter()
        .filter(|test| test.requirements.contains(&mutant.requirement))
        .collect();
    anyhow::ensure!(
        candidates.len() >= 2,
        "{} names requirement {}, which {} test(s) defend; a pair of killers needs two",
        mutant.id,
        mutant.requirement,
        candidates.len()
    );
    let observed_candidates: Vec<&&Test> = candidates
        .iter()
        .filter(|test| test.evidence.iter().any(|kind| kind == observed))
        .collect();
    // Rotation applies whether or not the operator's kind is available: the
    // fallback candidate list is still a list, and taking its head every time
    // is what put one test on 46% of the plan (review of `95a97b8b`).
    let primary = if observed_candidates.is_empty() {
        candidates[ordinal % candidates.len()]
    } else {
        observed_candidates[ordinal % observed_candidates.len()]
    };
    let start = candidates
        .iter()
        .position(|test| test.id == primary.id)
        .context("the primary is one of the candidates")?;
    let after = |step: usize| candidates[(start + step) % candidates.len()];
    let secondary = (1..candidates.len())
        .map(after)
        .find(|test| test.evidence != primary.evidence)
        .unwrap_or_else(|| after(1));
    Ok(vec![primary.id.clone(), secondary.id.clone()])
}

/// Print the killing tests AM-17.10 derives for every mutant, so the packet is
/// transcribed from the same implementation the gate verifies against rather
/// than from a second one that could disagree.
pub fn derive_killers_repo(root: &Utf8Path) -> Result<String> {
    let packet = read_packet(root)?;
    let mut out = String::new();
    for mutant in &packet.mutants {
        let derived = derive_killing_tests(&packet, mutant)?;
        out.push_str(&serde_json::to_string(&serde_json::json!({
            "id": mutant.id,
            "killing_tests": derived,
        }))?);
        out.push('\n');
    }
    Ok(out)
}

fn verify_mutant_killing_tests(root: &Utf8Path, packet: &Packet) -> Result<()> {
    // AM-17.10: the column is derived for every mutant, whatever its
    // disposition, and is checked before the clauses that inspect only killed
    // ones. This function used to return here when nothing was killed, so at
    // stage 1a — where all 65 mutants are predeclared — it read nothing at
    // all, and a generated killer column rode through fourteen lanes.
    let killed: Vec<&Mutant> = packet
        .mutants
        .iter()
        .filter(|mutant| mutant.disposition == "killed")
        .collect();
    let by_id: BTreeMap<&str, &Test> = packet
        .tests
        .iter()
        .map(|test| (test.id.as_str(), test))
        .collect();
    let ignored = ignored_test_names(root);
    for mutant in killed {
        if mutant.killing_tests.is_empty() {
            anyhow::bail!("{} claims killed but names no killing test", mutant.id);
        }
        for test_id in &mutant.killing_tests {
            let test = by_id.get(test_id.as_str()).with_context(|| {
                format!(
                    "{} names killing test {test_id}, which the packet does not declare",
                    mutant.id
                )
            })?;
            let leaf = test.name.rsplit("::").next().unwrap_or(test.name.as_str());
            if ignored.contains(leaf) {
                anyhow::bail!(
                    "{} claims killed by {} ({}), which is #[ignore]d — an ignored test \
                     never runs, so it cannot witness a kill",
                    mutant.id,
                    test_id,
                    test.name
                );
            }
        }
    }
    // AM-17.10: every mutant's column is derived, whatever its disposition.
    // Checked last so a kill witnessed by an ignored or missing test reports
    // that, rather than being answered by the derivation. The clauses above
    // inspect only `killed` mutants, which is why a plan of 65 predeclared
    // mutants carried a generated killer column through fourteen lanes with
    // no gate reading it at all.
    for mutant in &packet.mutants {
        let derived = derive_killing_tests(packet, mutant)?;
        anyhow::ensure!(
            mutant.killing_tests == derived,
            "{} names killing tests {:?}; AM-17.10 derives {:?} from requirement {} and \
             operator {:?}",
            mutant.id,
            mutant.killing_tests,
            derived,
            mutant.requirement,
            mutant.operator
        );
    }
    Ok(())
}

fn verify_mutant_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/mutants.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed mutant evidence is required; run `just haq-mutants`")
    })?;
    let evidence: MutantEvidence =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    require_eq(
        "mutant evidence schema_version",
        &evidence.schema_version,
        "haqp-mutants-v1",
    )?;
    require_eq(
        "mutant evidence source_commit",
        &evidence.source_commit,
        &packet
            .provenance
            .as_ref()
            .context("qualified mutant evidence requires packet provenance")?
            .fixed_commit,
    )?;
    let lockfile_blake3 = hex_digest(&fs::read(root.join("Cargo.lock"))?);
    require_eq(
        "mutant evidence lockfile_blake3",
        &evidence.lockfile_blake3,
        &lockfile_blake3,
    )?;
    require_eq(
        "mutant evidence runner",
        &evidence.runner,
        "liminal-xtask haq mutants",
    )?;

    let declared = packet
        .mutants
        .iter()
        .map(|mutant| (mutant.id.as_str(), mutant))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::<String>::new();
    let mut evaluated = BTreeSet::<String>::new();
    for row in &evidence.rows {
        verify_mutant_evidence_row(row, &declared, &mut seen, &mut evaluated)?;
    }
    // Every declared mutant has a row (not-ready or evaluated), and the
    // EVALUATED rows are exactly the non-predeclared dispositions.
    let all = packet
        .mutants
        .iter()
        .map(|m| m.id.clone())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        seen == all,
        "every declared mutant must have an evidence row (not-ready or evaluated): recorded={seen:?}, declared={all:?}"
    );
    let expected = packet
        .mutants
        .iter()
        .filter(|mutant| mutant.disposition != "predeclared")
        .map(|mutant| mutant.id.clone())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        evaluated == expected,
        "evaluated mutant rows differ: recorded={evaluated:?}, expected={expected:?}"
    );
    Ok(())
}

/// Re-run every semantic killed mutant from its fixed source commit and
/// compare the complete durable row. Exit codes and digests are claims until
/// this replay observes them from a fresh disposable worktree.
fn verify_mutant_evidence_replay(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let selected = packet
        .mutants
        .iter()
        .filter(|mutant| mutant.disposition == "killed")
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(());
    }
    let path = root.join("conformance/haqp/evidence/mutants.json");
    let evidence: MutantEvidence = serde_json::from_slice(&fs::read(&path)?)?;
    let source_commit = packet
        .provenance
        .as_ref()
        .context("mutant replay requires packet provenance")?
        .fixed_commit
        .clone();
    anyhow::ensure!(
        git_text(root, &["status", "--porcelain"])?.is_empty(),
        "mutant replay requires a clean source tree"
    );
    let ignored = ignored_test_names(root);
    let scratch = liminal_scratch::ScratchDir::new("haq-mutant-replay")?;
    let worktree = scratch.path().to_owned();
    let add = Command::new("git")
        .current_dir(root)
        .args(["worktree", "add", "--detach", "--quiet"])
        .arg(&worktree)
        .arg(&source_commit)
        .output()?;
    anyhow::ensure!(
        add.status.success(),
        "mutant replay worktree add failed: {}",
        String::from_utf8_lossy(&add.stderr).trim()
    );
    let mut guard = WorktreeGuard::new(root, &worktree);
    for mutant in selected {
        let expected = evidence
            .rows
            .iter()
            .find(|row| row.id == mutant.id)
            .with_context(|| format!("mutant replay missing evidence row {}", mutant.id))?;
        let actual = run_mutant_row(&worktree, packet, mutant, &ignored, false)?;
        anyhow::ensure!(
            actual == *expected,
            "mutant {} replay row differs from committed evidence",
            mutant.id
        );
        reset_mutant_worktree(&worktree, &source_commit)?;
    }
    guard.remove()?;
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "mutation evidence gate keeps all row invariants together"
)]
fn verify_mutant_evidence_row(
    row: &MutantEvidenceRow,
    declared: &BTreeMap<&str, &Mutant>,
    seen: &mut BTreeSet<String>,
    evaluated: &mut BTreeSet<String>,
) -> Result<()> {
    anyhow::ensure!(
        seen.insert(row.id.clone()),
        "duplicate mutant evidence row {}",
        row.id
    );
    let mutant = declared
        .get(row.id.as_str())
        .with_context(|| format!("mutant evidence names undeclared {}", row.id))?;
    // Stage 1a (M17.5 F-41). `haq mutants` records every mutant as `not-ready`
    // -- "recorded, not claimed" -- and this verifier, written for 1b's
    // evaluated rows, refused each one as "evaluated evidence". A not-ready row
    // is the ABSENCE of a claim: it may exist only for a predeclared mutant,
    // may carry no result, and does not count toward the evaluated set.
    if row.status == "not-ready" {
        anyhow::ensure!(
            mutant.disposition == "predeclared",
            "{} is declared {:?} but its evidence is not ready",
            row.id,
            mutant.disposition
        );
        require_eq(
            "mutant evidence disposition",
            &row.disposition,
            "predeclared",
        )?;
        anyhow::ensure!(
            row.failed_tests.is_empty() && row.exit_codes.is_empty(),
            "{} is not ready yet carries results",
            row.id
        );
        return Ok(());
    }
    anyhow::ensure!(
        mutant.disposition != "predeclared",
        "predeclared mutant {} has evaluated evidence",
        row.id
    );
    evaluated.insert(row.id.clone());
    require_eq(
        "mutant evidence disposition",
        &row.disposition,
        &mutant.disposition,
    )?;
    let mutant_patch = mutant
        .patch
        .as_ref()
        .context("evaluated mutant lacks patch")?;
    anyhow::ensure!(
        row.patch.as_ref() == Some(mutant_patch),
        "{} evidence patch differs from packet",
        row.id
    );
    anyhow::ensure!(
        row.runnable_tests.len() == row.exit_codes.len()
            && row.runnable_tests.len() == row.baseline_exit_codes.len()
            && row.runnable_tests.len() == row.stdout_blake3.len()
            && row.runnable_tests.len() == row.stderr_blake3.len(),
        "{} evidence vectors have different lengths",
        row.id
    );
    anyhow::ensure!(
        row.baseline_exit_codes.iter().all(|code| *code == 0),
        "{} baseline witness suite failed before mutation",
        row.id
    );
    if row.status == "error" {
        anyhow::ensure!(
            matches!(
                row.failure_class.as_deref(),
                Some("infrastructure" | "compilation" | "timeout")
            ),
            "{} error result must classify compilation, timeout, or infrastructure failure",
            row.id
        );
        anyhow::ensure!(
            row.reason
                .as_deref()
                .is_some_and(|reason| !reason.trim().is_empty()),
            "{} error result has no reason",
            row.id
        );
    }
    for witness in &mutant.killing_tests {
        anyhow::ensure!(
            row.runnable_tests.iter().any(|test| test == witness),
            "{} evidence omits declared killing test {}",
            row.id,
            witness
        );
    }
    for digest in row.stdout_blake3.iter().chain(row.stderr_blake3.iter()) {
        anyhow::ensure!(
            digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()),
            "{} has invalid output digest",
            row.id
        );
    }
    match row.disposition.as_str() {
        "killed" => {
            require_eq("mutant evidence status", &row.status, "killed")?;
            anyhow::ensure!(
                !row.failed_tests.is_empty(),
                "{} claims killed with no failed test",
                row.id
            );
            let mut seen_failed = BTreeSet::new();
            for failed in &row.failed_tests {
                anyhow::ensure!(
                    seen_failed.insert(failed),
                    "{} lists failed test {:?} more than once",
                    row.id,
                    failed
                );
                let index = row
                    .runnable_tests
                    .iter()
                    .position(|test| test == failed)
                    .with_context(|| {
                        format!(
                            "{} lists failed test {:?} that was not runnable",
                            row.id, failed
                        )
                    })?;
                anyhow::ensure!(
                    row.exit_codes[index] != 0,
                    "{} lists failed test {:?} with zero exit code",
                    row.id,
                    failed
                );
            }
        }
        "equivalent" | "duplicate" => {
            require_eq(
                "mutant evidence status",
                &row.status,
                row.disposition.as_str(),
            )?;
            anyhow::ensure!(
                row.failed_tests.is_empty(),
                "{} {} evidence cannot list failed tests",
                row.id,
                row.disposition
            );
            anyhow::ensure!(
                row.exit_codes.iter().all(|code| *code == 0),
                "{} {} evidence cannot contain nonzero exit codes",
                row.id,
                row.disposition
            );
        }
        other => anyhow::bail!("{} has unsupported evaluated disposition {other:?}", row.id),
    }
    Ok(())
}

fn validate_mutant_patch(patch: &MutantPatch) -> Result<()> {
    anyhow::ensure!(!patch.file.trim().is_empty(), "mutant patch names no file");
    let source_path = Path::new(&patch.file);
    anyhow::ensure!(
        !source_path.is_absolute(),
        "mutant patch path {:?} is absolute",
        patch.file
    );
    anyhow::ensure!(
        !source_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }),
        "mutant patch path {:?} escapes worktree",
        patch.file
    );
    anyhow::ensure!(
        !patch.before.is_empty(),
        "mutant patch {:?} has empty before text",
        patch.file
    );
    anyhow::ensure!(
        patch.before != patch.after,
        "mutant patch {:?} is a no-op replacement",
        patch.file
    );
    Ok(())
}

fn apply_mutant_patch(root: &Utf8Path, patch: &MutantPatch) -> Result<()> {
    validate_mutant_patch(patch)?;
    let target_path = root.join(&patch.file);
    let text = fs::read_to_string(&target_path)
        .with_context(|| format!("read mutant target {target_path}"))?;
    let matches = text.match_indices(&patch.before).count();
    anyhow::ensure!(
        matches == 1,
        "mutant patch {:?} expected one before occurrence, found {matches}",
        patch.file
    );
    let replaced = text.replacen(&patch.before, &patch.after, 1);
    fs::write(&target_path, replaced)
        .with_context(|| format!("write mutant target {target_path}"))?;
    Ok(())
}

fn validate_disposition(mutant: &Mutant) -> Result<()> {
    if mutant.disposition == "predeclared" {
        return Ok(());
    }
    let patch = mutant
        .patch
        .as_ref()
        .with_context(|| format!("{} has evaluated disposition but no patch", mutant.id))?;
    validate_mutant_patch(patch)?;
    if matches!(mutant.disposition.as_str(), "equivalent" | "duplicate") {
        anyhow::ensure!(
            mutant
                .disposition_proof
                .as_deref()
                .is_some_and(|proof| !proof.trim().is_empty()),
            "{} requires written proof for {} disposition",
            mutant.id,
            mutant.disposition
        );
        let mut concurrence = BTreeSet::new();
        let mut records = BTreeSet::new();
        for proof in &mutant.disposition_concurrence {
            anyhow::ensure!(
                !proof.reviewer.trim().is_empty()
                    && !proof.record.trim().is_empty()
                    && !proof.finding.trim().is_empty(),
                "{} has empty concurrence",
                mutant.id
            );
            anyhow::ensure!(
                matches!(proof.record.as_str(), "pass-1" | "pass-2"),
                "{} concurrence record {:?} must be pass-1 or pass-2",
                mutant.id,
                proof.record
            );
            concurrence.insert(proof.reviewer.as_str());
            records.insert(proof.record.as_str());
        }
        anyhow::ensure!(
            concurrence.len() >= 2 && records == BTreeSet::from(["pass-1", "pass-2"]),
            "{} requires distinct reviewer concurrence from both adversarial passes",
            mutant.id
        );
    }
    if mutant.disposition == "killed" {
        anyhow::ensure!(
            !mutant.killing_tests.is_empty(),
            "{} claims killed but names no killing test",
            mutant.id
        );
    }
    Ok(())
}

/// Equivalent/duplicate mutant dispositions must point at findings that exist
/// in the committed blind records. Reviewer names and pass labels alone are
/// not concurrence evidence: each tuple resolves to a verified, independently
/// reproduced finding in its named pass.
fn verify_mutant_concurrence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    if !packet
        .mutants
        .iter()
        .any(|mutant| matches!(mutant.disposition.as_str(), "equivalent" | "duplicate"))
    {
        return Ok(());
    }
    let mut records = BTreeMap::<u8, (String, ReviewRecord)>::new();
    for review in &packet.reviews {
        anyhow::ensure!(
            review.result == "pass",
            "mutant concurrence requires passing committed review record for {}",
            review.reviewer
        );
        let evidence = review
            .evidence
            .as_deref()
            .with_context(|| format!("{} has no committed review record", review.reviewer))?;
        let path = safe_repo_path(root, evidence, "mutant concurrence review")?;
        let record: ReviewRecord =
            serde_json::from_slice(&fs::read(&path).with_context(|| format!("read {path}"))?)
                .with_context(|| format!("parse {path}"))?;
        anyhow::ensure!(
            records
                .insert(record.pass, (review.reviewer.clone(), record))
                .is_none(),
            "duplicate committed review pass in mutant concurrence"
        );
    }
    for mutant in packet
        .mutants
        .iter()
        .filter(|mutant| matches!(mutant.disposition.as_str(), "equivalent" | "duplicate"))
    {
        for concurrence in &mutant.disposition_concurrence {
            let pass = match concurrence.record.as_str() {
                "pass-1" => 1,
                "pass-2" => 2,
                other => anyhow::bail!(
                    "{} concurrence record {:?} must be pass-1 or pass-2",
                    mutant.id,
                    other
                ),
            };
            let (reviewer, record) = records
                .get(&pass)
                .with_context(|| format!("{} concurrence names absent {pass:?}", mutant.id))?;
            anyhow::ensure!(
                concurrence.reviewer == *reviewer
                    || concurrence.reviewer == record.reviewer.model_family,
                "{} concurrence reviewer {:?} does not identify committed pass {}",
                mutant.id,
                concurrence.reviewer,
                pass
            );
            let findings = review_finding_ids(&record.findings, &record.attempts, reviewer)?;
            let attempt_id = findings
                .get(concurrence.finding.as_str())
                .with_context(|| {
                    format!(
                        "{} concurrence finding {:?} is absent from committed pass {}",
                        mutant.id, concurrence.finding, pass
                    )
                })?;
            let attempt = record
                .attempts
                .iter()
                .find(|attempt| attempt.id == *attempt_id)
                .expect("finding linkage validated");
            anyhow::ensure!(
                attempt.classification == "verified_defect"
                    && attempt.independently_reproduced
                    && record
                        .independently_reproduced
                        .iter()
                        .any(|id| id == &concurrence.finding),
                "{} concurrence finding {:?} is not independently reproduced verified defect",
                mutant.id,
                concurrence.finding
            );
            // Blind pass 1 at `6b36bbb9` (A04): existence and reproduction were
            // checked and SUBSTANCE was not, so any reproduced defect in the
            // record satisfied the concurrence for any mutant claiming
            // equivalence. A reviewer agreeing that a mutant is equivalent has
            // to have been talking about that mutant, which means naming it or
            // the line it sits on — the same binding resolution evidence uses.
            let coordinate = mutant.source.as_str();
            let prose = format!(
                "{} {} {}",
                attempt.attempt, attempt.observed_result, attempt.target
            );
            anyhow::ensure!(
                prose.contains(mutant.id.as_str()) || prose.contains(coordinate),
                "{} concurrence cites finding {:?}, which names neither the mutant nor its \
                 coordinate {coordinate}: a reviewer agreeing a mutant is equivalent must have \
                 been talking about that mutant",
                mutant.id,
                concurrence.finding
            );
        }
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn sha256_text(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn git_text(root: &Utf8Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .with_context(|| format!("run git {args:?}"))?;
    anyhow::ensure!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn regex_literal(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' => {
                vec!['\\', ch]
            }
            other => vec![other],
        })
        .collect()
}

fn run_one_mutant_test(
    worktree: &Utf8Path,
    test_name: &str,
    run_ignored: bool,
) -> Result<(i32, Vec<u8>, Vec<u8>)> {
    let leaf = test_name.rsplit("::").next().unwrap_or(test_name);
    let expression = format!("test(/^{}$/)", regex_literal(leaf));
    let mut command = Command::new("cargo");
    command
        .current_dir(worktree)
        .args([
            "nextest",
            "run",
            "--workspace",
            "--all-features",
            "--profile",
            "ci",
        ])
        .arg("-E")
        .arg(expression);
    if run_ignored {
        command.args(["--run-ignored", "all"]);
    }
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("run mutation witness {test_name}"))?;
    let code = output.status.code().unwrap_or(-1);
    Ok((code, output.stdout, output.stderr))
}

fn is_compilation_failure(stdout: &[u8], stderr: &[u8]) -> bool {
    stdout
        .iter()
        .chain(stderr)
        .copied()
        .collect::<Vec<_>>()
        .windows(b"could not compile".len())
        .any(|window| window.eq_ignore_ascii_case(b"could not compile"))
}

fn is_semantic_test_failure(test_name: &str, stdout: &[u8], stderr: &[u8]) -> bool {
    let leaf = test_name.rsplit("::").next().unwrap_or(test_name);
    let text = String::from_utf8_lossy(stdout);
    let err = String::from_utf8_lossy(stderr);
    // Require named-test and failure token on one line. This rejects launcher
    // infrastructure prose such as "launcher FAILED before tests" instead of
    // counting it as a semantic mutant kill.
    [text.as_ref(), err.as_ref()].iter().any(|output| {
        output
            .lines()
            .any(|line| line.trim_start().starts_with("FAIL [") && line.contains(leaf))
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "keeps mutation execution and its evidence assembly in one auditable path"
)]
fn run_mutant_row(
    worktree: &Utf8Path,
    packet: &Packet,
    mutant: &Mutant,
    ignored: &BTreeSet<String>,
    run_ignored: bool,
) -> Result<MutantEvidenceRow> {
    let Some(patch) = mutant.patch.clone() else {
        return Ok(MutantEvidenceRow {
            id: mutant.id.clone(),
            disposition: mutant.disposition.clone(),
            status: "not-ready".to_owned(),
            patch: None,
            runnable_tests: Vec::new(),
            baseline_exit_codes: Vec::new(),
            failed_tests: Vec::new(),
            exit_codes: Vec::new(),
            stdout_blake3: Vec::new(),
            stderr_blake3: Vec::new(),
            reason: Some("no patch declared".to_owned()),
            failure_class: None,
        });
    };
    let runnable = mutant
        .killing_tests
        .iter()
        .filter_map(|test_id| packet.tests.iter().find(|test| &test.id == test_id))
        .filter(|test| {
            run_ignored
                || !ignored.contains(test.name.rsplit("::").next().unwrap_or(test.name.as_str()))
        })
        .map(|test| (test.id.clone(), test.name.clone()))
        .collect::<Vec<_>>();
    if runnable.is_empty() {
        return Ok(MutantEvidenceRow {
            id: mutant.id.clone(),
            disposition: mutant.disposition.clone(),
            status: "not-runnable".to_owned(),
            patch: Some(patch),
            runnable_tests: Vec::new(),
            baseline_exit_codes: Vec::new(),
            failed_tests: Vec::new(),
            exit_codes: Vec::new(),
            stdout_blake3: Vec::new(),
            stderr_blake3: Vec::new(),
            reason: Some("no runnable killing tests".to_owned()),
            failure_class: None,
        });
    }
    let baseline_exit_codes = runnable
        .iter()
        .map(|(_, test_name)| {
            run_one_mutant_test(worktree, test_name, run_ignored).map(|(code, _, _)| code)
        })
        .collect::<Result<Vec<_>>>()?;
    if let Some((index, code)) = baseline_exit_codes
        .iter()
        .enumerate()
        .find(|(_, code)| **code != 0)
        .map(|(index, code)| (index, *code))
    {
        return Ok(MutantEvidenceRow {
            id: mutant.id.clone(),
            disposition: mutant.disposition.clone(),
            status: "error".to_owned(),
            patch: Some(patch),
            runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
            baseline_exit_codes,
            failed_tests: Vec::new(),
            exit_codes: Vec::new(),
            stdout_blake3: Vec::new(),
            stderr_blake3: Vec::new(),
            reason: Some(format!(
                "baseline witness {} exited with {code}",
                runnable[index].0
            )),
            failure_class: Some("infrastructure".to_owned()),
        });
    }
    if let Err(error) = apply_mutant_patch(worktree, &patch) {
        return Ok(MutantEvidenceRow {
            id: mutant.id.clone(),
            disposition: mutant.disposition.clone(),
            status: "error".to_owned(),
            patch: Some(patch),
            runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
            baseline_exit_codes,
            failed_tests: Vec::new(),
            exit_codes: Vec::new(),
            stdout_blake3: Vec::new(),
            stderr_blake3: Vec::new(),
            reason: Some(error.to_string()),
            failure_class: Some("infrastructure".to_owned()),
        });
    }
    let mut failed_tests = Vec::new();
    let mut exit_codes = Vec::new();
    let mut stdout_blake3 = Vec::new();
    let mut stderr_blake3 = Vec::new();
    for (test_id, test_name) in &runnable {
        let (code, stdout, stderr) = run_one_mutant_test(worktree, test_name, run_ignored)?;
        exit_codes.push(code);
        stdout_blake3.push(hex_digest(&stdout));
        stderr_blake3.push(hex_digest(&stderr));
        if code != 0 && is_compilation_failure(&stdout, &stderr) {
            return Ok(MutantEvidenceRow {
                id: mutant.id.clone(),
                disposition: mutant.disposition.clone(),
                status: "error".to_owned(),
                patch: Some(patch),
                runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
                baseline_exit_codes,
                failed_tests: Vec::new(),
                exit_codes,
                stdout_blake3,
                stderr_blake3,
                reason: Some(format!(
                    "mutation caused compilation failure while running {test_id}"
                )),
                failure_class: Some("compilation".to_owned()),
            });
        }
        if code != 0 && !is_semantic_test_failure(test_name, &stdout, &stderr) {
            return Ok(MutantEvidenceRow {
                id: mutant.id.clone(),
                disposition: mutant.disposition.clone(),
                status: "error".to_owned(),
                patch: Some(patch),
                runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
                baseline_exit_codes,
                failed_tests: Vec::new(),
                exit_codes,
                stdout_blake3,
                stderr_blake3,
                reason: Some(format!(
                    "mutation witness {test_id} exited {code} without semantic test-failure evidence"
                )),
                failure_class: Some("infrastructure".to_owned()),
            });
        }
        if code != 0 {
            failed_tests.push(test_id.clone());
        }
    }
    let status = if failed_tests.is_empty() {
        "survived"
    } else {
        "killed"
    };
    Ok(MutantEvidenceRow {
        id: mutant.id.clone(),
        disposition: mutant.disposition.clone(),
        status: status.to_owned(),
        patch: Some(patch),
        runnable_tests: runnable.iter().map(|(id, _)| id.clone()).collect(),
        baseline_exit_codes,
        failed_tests,
        exit_codes,
        stdout_blake3,
        stderr_blake3,
        reason: None,
        failure_class: None,
    })
}

fn reset_mutant_worktree(worktree: &Utf8Path, source_commit: &str) -> Result<()> {
    let reset = Command::new("git")
        .current_dir(worktree)
        .args(["reset", "--hard", "--quiet", source_commit])
        .output()?;
    anyhow::ensure!(reset.status.success(), "reset mutant worktree failed");
    let clean = Command::new("git")
        .current_dir(worktree)
        .args(["clean", "-fdx", "-q"])
        .output()?;
    anyhow::ensure!(clean.status.success(), "clean mutant worktree failed");
    Ok(())
}

struct WorktreeGuard {
    repo: Utf8PathBuf,
    path: Utf8PathBuf,
    active: bool,
}

impl WorktreeGuard {
    fn new(repo: &Utf8Path, path: &Utf8Path) -> Self {
        Self {
            repo: repo.to_owned(),
            path: path.to_owned(),
            active: true,
        }
    }

    fn remove(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        let output = Command::new("git")
            .current_dir(&self.repo)
            .args(["worktree", "remove", "-f"])
            .arg(&self.path)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "git worktree remove failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        self.active = false;
        Ok(())
    }
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = Command::new("git")
                .current_dir(&self.repo)
                .args(["worktree", "remove", "-f"])
                .arg(&self.path)
                .output();
        }
    }
}

/// Execute source patches in disposable git worktrees. This command writes only
/// digests and statuses; raw test/model output never enters the repository.
pub fn run_mutants_repo(root: &Utf8Path, ids: &[String], run_ignored: bool) -> Result<()> {
    let packet = read_packet(root)?;
    verify_packet_shape(&packet)?;
    let dirt = source_dirt(root)?;
    anyhow::ensure!(
        dirt.is_empty(),
        "mutant runner requires a clean SOURCE tree; dirty: {dirt:?}"
    );
    let source_commit = git_text(root, &["rev-parse", "HEAD"])?;
    let lockfile_blake3 = hex_digest(&fs::read(root.join("Cargo.lock"))?);
    let selected = if ids.is_empty() {
        packet.mutants.iter().collect::<Vec<_>>()
    } else {
        let wanted = ids.iter().cloned().collect::<BTreeSet<_>>();
        let selected = packet
            .mutants
            .iter()
            .filter(|mutant| wanted.contains(&mutant.id))
            .collect::<Vec<_>>();
        let present = selected
            .iter()
            .map(|mutant| mutant.id.clone())
            .collect::<BTreeSet<_>>();
        anyhow::ensure!(
            present.len() == wanted.len(),
            "unknown mutant id(s): {:?}",
            wanted.difference(&present).collect::<Vec<_>>()
        );
        selected
    };

    let ignored = ignored_test_names(root);
    let scratch = liminal_scratch::ScratchDir::new("haq-mutants")?;
    let worktree = scratch.path().to_owned();
    let add = Command::new("git")
        .current_dir(root)
        .args(["worktree", "add", "--detach", "--quiet"])
        .arg(&worktree)
        .arg(&source_commit)
        .output()?;
    anyhow::ensure!(
        add.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&add.stderr).trim()
    );
    let mut worktree_guard = WorktreeGuard::new(root, &worktree);

    let mut rows = Vec::with_capacity(selected.len());
    for mutant in selected {
        rows.push(run_mutant_row(
            &worktree,
            &packet,
            mutant,
            &ignored,
            run_ignored,
        )?);
        let error = rows.last().and_then(|row| {
            (row.status == "error").then(|| {
                (
                    row.id.clone(),
                    row.reason
                        .clone()
                        .unwrap_or_else(|| "unknown mutation error".to_owned()),
                )
            })
        });
        if let Some((id, reason)) = error {
            reset_mutant_worktree(&worktree, &source_commit)?;
            worktree_guard.remove()?;
            anyhow::bail!("{id}: {reason}");
        }
        reset_mutant_worktree(&worktree, &source_commit)?;
    }
    worktree_guard.remove()?;
    drop(scratch);

    let evidence = MutantEvidence {
        schema_version: "haqp-mutants-v1".to_owned(),
        source_commit,
        lockfile_blake3,
        runner: "liminal-xtask haq mutants".to_owned(),
        rows,
    };
    let path = root.join("conformance/haqp/evidence/mutants.json");
    fs::create_dir_all(path.parent().expect("evidence parent"))?;
    fs::write(&path, serde_json::to_vec_pretty(&evidence)?)?;
    println!(
        "mutant evidence written to {path}; rows={}",
        evidence.rows.len()
    );
    Ok(())
}

/// Leaf names of every `#[ignore]`d test in the tree.
fn ignored_test_names(root: &Utf8Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut stack = vec![root.join("conformance"), root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                if path.file_name() != Some("target") {
                    stack.push(path);
                }
            } else if path.extension() == Some("rs")
                && let Ok(text) = fs::read_to_string(&path)
            {
                collect_ignored(&text, &mut names);
            }
        }
    }
    names
}

/// An `#[ignore]` attribute applies to the next `fn` declaration.
fn collect_ignored(text: &str, names: &mut BTreeSet<String>) {
    let mut pending = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        // A test is non-runnable if it is ignored OUTRIGHT, ignored
        // CONDITIONALLY, or compiled out. All three cannot witness a kill, and
        // only the first was recognised: `#[cfg_attr(unix, ignore)]` and
        // `#[cfg(any())]` both vanish at run time while reading as live source.
        if trimmed.starts_with("#[ignore")
            || (trimmed.starts_with("#[cfg_attr(") && trimmed.contains("ignore"))
            || trimmed.starts_with("#[cfg(any())]")
        {
            pending = true;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("fn ") {
            if pending && let Some(name) = rest.split('(').next() {
                names.insert(name.trim().to_owned());
            }
            pending = false;
        }
    }
}

/// Blind pass 1 at aa00d41 (A10): mutation runs and the `#[ignore]` check both
/// filter by a test's LEAF name, so two tests sharing a leaf make the filter
/// ambiguous — an unrelated namesake could pass and certify a declared kill,
/// or an ignored namesake could mask one. A leaf a mutant relies on must name
/// exactly one runnable test in the tree.
fn verify_killing_test_leaves_are_unambiguous(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let mut leaves: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut stack = vec![root.join("conformance"), root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                if path.file_name() != Some("target") {
                    stack.push(path);
                }
            } else if path.extension() == Some("rs")
                && let Ok(text) = fs::read_to_string(&path)
            {
                let mut local = BTreeMap::new();
                collect_runnable_test_functions(&text, &mut local);
                let where_ = path.strip_prefix(root).unwrap_or(&path).to_string();
                for name in local.keys() {
                    leaves.entry(name.clone()).or_default().push(where_.clone());
                }
            }
        }
    }
    let declared = packet
        .tests
        .iter()
        .map(|test| (test.id.as_str(), test.name.as_str()))
        .collect::<BTreeMap<_, _>>();
    for mutant in &packet.mutants {
        for test_id in &mutant.killing_tests {
            let Some(name) = declared.get(test_id.as_str()) else {
                continue;
            };
            let leaf = name.rsplit("::").next().unwrap_or(name);
            if let Some(files) = leaves.get(leaf)
                && files.len() > 1
            {
                anyhow::bail!(
                    "{} is killed by {test_id} ({name}), whose leaf {leaf:?} names {} tests \
                     ({files:?}); a leaf the mutation run filters on must be unique, or a \
                     namesake can certify the kill",
                    mutant.id,
                    files.len()
                );
            }
        }
    }
    Ok(())
}

fn verify_test_names_exist(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let mut runnable = BTreeMap::new();
    let mut stack = vec![root.join("conformance"), root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            if path.is_dir() {
                if path.file_name() != Some("target") {
                    stack.push(path);
                }
            } else if path.extension() == Some("rs")
                && let Ok(text) = fs::read_to_string(&path)
            {
                let mut local = BTreeMap::new();
                collect_runnable_test_functions(&text, &mut local);
                let prefix = path
                    .strip_prefix(root.join("conformance/tests"))
                    .ok()
                    .and_then(|relative| {
                        let mut parts = relative
                            .components()
                            .map(|component| component.as_str().to_owned())
                            .collect::<Vec<_>>();
                        let stem = parts.pop()?.strip_suffix(".rs")?.to_owned();
                        parts.push(stem);
                        Some(parts.join("::"))
                    });
                for (leaf, count) in local {
                    if let Some(prefix) = &prefix {
                        runnable.insert(format!("{prefix}::{leaf}"), count);
                    }
                }
            }
        }
    }

    let mut missing = Vec::new();
    for test in &packet.tests {
        if test.name.trim().is_empty() {
            anyhow::bail!("{} declares an empty test name", test.id);
        }
        if !runnable.contains_key(test.name.as_str()) {
            missing.push(format!("{} -> {}", test.id, test.name));
        } else if runnable.get(test.name.as_str()) != Some(&1) {
            missing.push(format!(
                "{} -> {} (module-qualified test name is ambiguous)",
                test.id, test.name
            ));
        }
    }
    if !missing.is_empty() {
        anyhow::bail!(
            "qualification claims {} test(s) that do not exist in the tree: {}",
            missing.len(),
            missing.join(", ")
        );
    }
    Ok(())
}

fn collect_runnable_test_functions(text: &str, names: &mut BTreeMap<String, usize>) {
    let mut test_pending = false;
    let mut excluded = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[test]") {
            test_pending = true;
        } else if trimmed.starts_with("#[ignore")
            || (trimmed.starts_with("#[cfg_attr(") && trimmed.contains("ignore"))
            || cfg_attribute_excludes(trimmed)
        {
            // Keep an exclusion only across the attribute list immediately
            // preceding one test. A module-level cfg or one ignored function
            // must not poison every later runnable test in the file.
            excluded = true;
        } else if test_pending {
            let declaration = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
            let declaration = declaration.strip_prefix("async ").unwrap_or(declaration);
            if let Some(rest) = declaration.strip_prefix("fn ") {
                if let Some(name) = rest
                    .split('(')
                    .next()
                    .filter(|name| !name.trim().is_empty())
                    && !excluded
                {
                    *names.entry(name.trim().to_owned()).or_default() += 1;
                }
                test_pending = false;
                excluded = false;
            }
        } else if !test_pending {
            // An attribute that did not lead directly to a test belongs to a
            // module/helper declaration, not to the next test function.
            excluded = false;
        }
    }
}

fn cfg_attribute_excludes(trimmed: &str) -> bool {
    if !trimmed.starts_with("#[cfg(") {
        return false;
    }
    if trimmed == "#[cfg(any())]" {
        return true;
    }
    if trimmed == "#[cfg(test)]" {
        return false;
    }
    if trimmed == "#[cfg(not(test))]" {
        return true;
    }
    if trimmed == "#[cfg(unix)]" {
        return !cfg!(unix);
    }
    if trimmed == "#[cfg(windows)]" {
        return !cfg!(windows);
    }
    if trimmed.contains("target_os = \"linux\"") {
        return !cfg!(target_os = "linux");
    }
    if trimmed.contains("target_os = \"windows\"") {
        return !cfg!(target_os = "windows");
    }
    // Unknown predicates remain fail-closed: scanner cannot prove they are
    // active on this host.
    true
}

/// The recorded fuzz campaign is too long to re-run per verification (150
/// target-minutes), so its artifact is COMMITTED and the packet must agree
/// with it exactly. Cheap lanes are re-run instead — see `haq verify-full`.
/// Target identity is checked here; family budget reconciliation happens in
/// `verify_fuzz_budget`, which derives each family's minutes from its declared
/// target mapping. There is one source of campaign duration truth: this
/// artifact's per-target seconds.
fn verify_fuzz_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/fuzz.json");
    let bytes =
        fs::read(&path).with_context(|| format!("{path}: committed fuzz evidence is required"))?;
    let recorded: Vec<FuzzEvidence> =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;

    // M17.5 pass-2 #17: the count was compared against `packet.generated.len()`,
    // which is a category error — fuzz TARGETS are not generated FAMILIES. The
    // the counts can differ, so comparing them would be a category error and
    // could accept a row naming a target that does not exist.
    // Bind to the targets that actually exist in the tree instead.
    let mut present = BTreeSet::new();
    let targets_dir = root.join("fuzz/fuzz_targets");
    let entries = fs::read_dir(&targets_dir)
        .with_context(|| format!("{targets_dir}: fuzz targets are required to verify the lane"))?;
    for entry in entries.flatten() {
        let Ok(file) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if file.extension() == Some("rs")
            && let Some(stem) = file.file_stem()
        {
            present.insert(stem.to_owned());
        }
    }
    verify_fuzz_rows(&recorded, &present)?;
    for row in &recorded {
        let proof = row.sanitizer_proof.as_ref().with_context(|| {
            format!(
                "{}: qualified fuzz evidence lacks compiler/runtime sanitizer proof",
                row.target
            )
        })?;
        verify_sanitizer_proof(root, row, proof, packet.provenance.as_ref())?;
    }
    verify_fuzz_seed_manifests(root, &recorded)?;
    verify_fuzz_logs(root, &recorded)?;
    verify_fuzz_budget(&packet.generated, &recorded)
}

fn verify_fuzz_seed_manifests(root: &Utf8Path, recorded: &[FuzzEvidence]) -> Result<()> {
    for row in recorded {
        let seed_dir = root.join("fuzz/corpus").join(&row.target);
        let (count, digest) = seed_manifest_digest(root, &seed_dir)?;
        anyhow::ensure!(
            count >= 16,
            "{}: committed seed set has {count} inputs; D23.2 requires at least 16",
            row.target
        );
        anyhow::ensure!(
            row.seed_count == count,
            "{}: evidence records {} seeds but committed corpus has {count}",
            row.target,
            row.seed_count
        );
        require_eq(
            &format!("{} seed_manifest_blake3", row.target),
            &row.seed_manifest_blake3,
            &digest,
        )?;
        verify_seed_classes(root, &row.target, &seed_dir)?;
    }
    Ok(())
}

/// The seed classes ADR-0020 §4 requires a corpus to span, and the closed name
/// tokens that declare each. A seed matching no negative token is a valid
/// input by declaration; a corpus needs at least one seed in every class.
/// What a seed's name asserts about its bytes, where the name asserts anything
/// checkable. Blind pass 1 at `0a0b4b4b` (A03): the class came from the name
/// and F-66 added only byte-distinctness, so a valid payload named
/// `12-invalid-utf8.bin` witnessed the malformed class for five targets — and
/// five committed seeds were exactly that, their intended `0xff 0xfe` having
/// been UTF-8 encoded on the way to disk, which is the one thing that makes
/// those bytes valid. A name that asserts a byte property must exhibit it.
type SeedPredicate = fn(&[u8]) -> bool;

const SEED_NAME_PREDICATES: [(&str, SeedPredicate); 5] = [
    ("invalid-utf8", |bytes| std::str::from_utf8(bytes).is_err()),
    ("empty", |bytes: &[u8]| {
        bytes.iter().copied().all(|byte| byte.is_ascii_whitespace())
    }),
    ("control", |bytes| {
        bytes
            .iter()
            .any(|byte| byte.is_ascii_control() && !byte.is_ascii_whitespace())
    }),
    ("unicode", |bytes| {
        std::str::from_utf8(bytes).is_ok_and(|text| !text.is_ascii())
    }),
    ("long-line", |bytes| {
        bytes
            .split(|byte| *byte == b'\n')
            .any(|line| line.len() >= 64)
    }),
];

/// The names that DECLARE a well-formed seed. Blind pass 1 at `ec045588`
/// (A03): the valid class was whatever matched no negative token, so a seed of
/// malformed bytes under any unrecognised name satisfied ADR-0020 §4's demand
/// that the corpus span valid input. A default is not a declaration; the valid
/// class is now witnessed by a name that claims it, like every other class.
const SEED_VALID_TOKENS: [&str; 17] = [
    "empty",
    "single-token",
    "single-node",
    "single-edge",
    "single-step",
    "whitespace",
    "unicode",
    "long-line",
    "long-run",
    "nested",
    "deep",
    "wide",
    "comment",
    "escape",
    "dag",
    "legal-path",
    "large-payload",
];

const SEED_CLASSES: [(&str, &[&str]); 4] = [
    ("boundary", &["boundary"]),
    ("truncated", &["truncated", "unterminated"]),
    (
        "malformed",
        &[
            "malformed",
            "invalid",
            "illegal",
            "duplicate",
            "missing",
            "unknown",
            "control",
        ],
    ),
    ("hostile", &["hostile"]),
];

/// Blind pass 1 at 612cbcc (A10): the manifest check counted seeds and hashed
/// them; sixteen inputs of one class satisfied it. Each tracked seed's name
/// declares its class through the closed token table above.
fn verify_seed_classes(root: &Utf8Path, target: &str, seed_dir: &Utf8Path) -> Result<()> {
    let relative = seed_dir
        .strip_prefix(root)
        .map_or_else(|_| seed_dir.to_string(), ToString::to_string);
    let listed = git_text(root, &["ls-files", "-z", "--", &relative])?;
    let mut counts = BTreeMap::new();
    // Blind pass 1 at `6b36bbb9` (A05): the class came from the FILE NAME and
    // nothing read the bytes, so five copies of one seed under five names
    // satisfied "seeds spanning valid, boundary, truncated, malformed and
    // hostile". Content is what makes a seed a seed: two tracked seeds in one
    // target may not carry the same bytes. Emptiness is not forbidden — an
    // empty input is a real boundary for a fuzz target, and two targets commit
    // one on purpose — but it can only be committed once, like anything else.
    let mut by_digest: BTreeMap<String, String> = BTreeMap::new();
    for name in listed.split('\0').filter(|name| !name.trim().is_empty()) {
        let file = Utf8Path::new(name)
            .file_name()
            .unwrap_or(name)
            .to_ascii_lowercase();
        let bytes = fs::read(root.join(name))
            .with_context(|| format!("{target}: read committed seed {name}"))?;
        let digest = blake3::hash(&bytes).to_hex().to_string();
        if let Some(first) = by_digest.insert(digest, name.to_owned()) {
            anyhow::bail!(
                "{target}: committed seeds {first} and {name} carry identical bytes; a class \
                 spanned by copies of one seed is not a class the corpus covers"
            );
        }
        for (token, holds) in SEED_NAME_PREDICATES {
            anyhow::ensure!(
                !file.contains(token) || holds(&bytes),
                "{target}: seed {name} is named {token:?} and its bytes are not; a class \
                 witnessed by a mislabelled seed is a class the corpus does not cover"
            );
        }
        let class = SEED_CLASSES
            .iter()
            .find(|(_, tokens)| tokens.iter().any(|token| file.contains(token)))
            .map(|(class, _)| *class)
            .or_else(|| {
                SEED_VALID_TOKENS
                    .iter()
                    .any(|token| file.contains(token))
                    .then_some("valid")
            });
        if let Some(class) = class {
            *counts.entry(class).or_insert(0usize) += 1;
        }
    }
    // "valid" is the implicit class of a seed matching no negative token; the
    // required set is that plus every declared negative class, so adding a
    // class to the table enforces it.
    for class in std::iter::once("valid").chain(SEED_CLASSES.iter().map(|(class, _)| *class)) {
        anyhow::ensure!(
            counts.get(class).copied().unwrap_or(0) > 0,
            "{target}: committed seed set has no {class} seed; ADR-0020 §4 requires seeds \
             spanning valid, boundary, truncated, malformed and hostile input (declared by \
             name: {:?})",
            SEED_CLASSES
                .iter()
                .map(|(c, t)| (*c, *t))
                .collect::<Vec<_>>()
        );
    }
    Ok(())
}

/// Write a target's classed seed set (A10). Deterministic, so the committed
/// seeds are reproducible from this code. It writes files only: the seeds
/// become the corpus when they are `git add -f`ed (fuzz/corpus is ignored so
/// libFuzzer's accumulation stays out) and committed, since the manifest and
/// class checks read the tracked set. `graph_interchange_codec` seeds are
/// transactions built by the same constructors the generated cases use;
/// `ilrp_recovery` seeds are the byte walks its target consumes.
pub fn write_seed_corpus_repo(root: &Utf8Path, target: &str) -> Result<()> {
    let dir = root.join("fuzz/corpus").join(target);
    fs::create_dir_all(&dir)?;
    let mut rng = Rng(0x5eed_c0de);
    let seeds: Vec<(&str, Vec<u8>)> = match target {
        "graph_interchange_codec" => {
            let txn = |rng: &mut Rng, category: &str| {
                serde_json::to_vec(&interchange_transaction(category, rng, "seed", false).0)
                    .expect("transaction encodes")
            };
            let mut boundary = interchange_transaction("single-node", &mut rng, "seed", false).0;
            boundary.parent = liminal_id::GraphRevisionId(u64::MAX);
            boundary.meta.at = liminal_id::Timestamp(i64::MAX);
            vec![
                ("01-empty.json", Vec::new()),
                ("02-single-node.json", txn(&mut rng, "single-node")),
                ("03-single-edge.json", txn(&mut rng, "single-edge")),
                ("04-dag.json", txn(&mut rng, "dag")),
                ("05-wide.json", txn(&mut rng, "wide")),
                ("06-deep.json", txn(&mut rng, "deep")),
                ("07-large-payload.json", txn(&mut rng, "large-payload")),
                ("08-duplicate-id.json", br#"{"id":"00000000-0000-0000-0000-000000000001","id":"00000000-0000-0000-0000-000000000002","parent":0,"meta":{"actor":null,"origin":"human","at":0,"provenance":null,"inverse":null},"ops":[]}"#.to_vec()),
                ("09-missing-node.json", br#"{"ops":[{"delete-node":{"id":"00000000-0000-0000-0000-000000000001"}}]}"#.to_vec()),
                ("10-unknown-kind.json", br#"{"id":"00000000-0000-0000-0000-000000000001","parent":0,"meta":{"actor":null,"origin":"nobody","at":0,"provenance":null,"inverse":null},"ops":[]}"#.to_vec()),
                ("11-comment.json", b"// not json\n{\"ops\":[]}".to_vec()),
                ("12-truncated-json.json", {
                    let mut full = txn(&mut rng, "single-edge");
                    full.truncate(full.len() / 2);
                    full
                }),
                ("13-invalid-json.bin", b"{\"ops\":[\x00\xff".to_vec()),
                ("14-boundary-revision.json", serde_json::to_vec(&boundary).expect("encodes")),
                ("15-unicode-payload.json", serde_json::to_vec(&interchange_transaction("single-node", &mut rng, "\u{1F600} \u{202e}rtl \u{0}", false).0).expect("encodes")),
                ("16-hostile.bin", vec![0, 0xff, 0x7f, b'{', b'"', 0xc3, 0x28]),
            ]
        }
        "ilrp_recovery" => {
            // Byte b drives state STATES[b % 7]: 0 Prepared, 1 Applying,
            // 2 ExternalApplied, 3 Finalizing, 4 Committed, 5 NeedsReview,
            // 6 Aborted. Legal path to Committed: 1,2,3,4.
            vec![
                ("01-empty.bin", Vec::new()),
                ("02-single-step.bin", vec![1]),
                ("03-legal-path-committed.bin", vec![1, 2, 3, 4]),
                ("04-abort-path.bin", vec![1, 6]),
                ("05-needs-review-path.bin", vec![1, 2, 5]),
                ("06-truncated-path.bin", vec![1, 2, 3]),
                ("07-boundary-max-byte.bin", vec![255, 254, 253, 252]),
                ("08-boundary-all-terminals.bin", vec![4, 5, 6, 4, 5, 6]),
                ("09-illegal-steps.bin", vec![4, 0, 3, 0, 6, 0]),
                ("10-after-terminal.bin", vec![1, 2, 3, 4, 1, 2, 3]),
                (
                    "11-long-run.bin",
                    (0..4096u32)
                        .map(|i| u8::try_from(i % 7).expect("fits"))
                        .collect(),
                ),
                (
                    "12-wide-alternating.bin",
                    (0..64u8).map(|i| if i % 2 == 0 { 1 } else { 2 }).collect(),
                ),
                (
                    "13-deep-cycle-attempt.bin",
                    vec![1, 2, 1, 2, 1, 2, 3, 2, 3, 4],
                ),
                (
                    "14-invalid-high-bytes.bin",
                    vec![0x80, 0x90, 0xa0, 0xb0, 0xc0, 0xd0, 0xe0, 0xf0],
                ),
                (
                    "15-hostile-random.bin",
                    (0..256)
                        .map(|_| u8::try_from(rng.below(256)).expect("fits"))
                        .collect(),
                ),
                ("16-hostile-zero-fill.bin", vec![0; 1024]),
            ]
        }
        other => anyhow::bail!("no classed seed set is defined for {other}"),
    };
    for (name, bytes) in seeds {
        fs::write(dir.join(name), bytes)?;
    }
    println!("wrote 16 classed seeds to {dir}");
    Ok(())
}

/// Digest the seed corpus that is actually COMMITTED (M17.5 F-32).
///
/// This read the filesystem and called the result "the committed corpus". It is
/// not: `.gitignore` excludes `fuzz/corpus/`, so the directory holds sixteen
/// tracked seeds per target plus every input libFuzzer has written there. The
/// recorded `seed_count` was therefore a property of ONE MACHINE — 18,233 here
/// — and a fresh clone, holding the sixteen, could never satisfy it. Killing a
/// campaign turned a green tree red with no code change.
///
/// `git ls-files` fixes it in the direction the error message already claimed,
/// and sixteen tracked seeds per target is exactly ADR-0020 §4's "at least 16
/// predeclared seeds".
fn seed_manifest_digest(root: &Utf8Path, seed_dir: &Utf8Path) -> Result<(u64, String)> {
    let relative = seed_dir
        .strip_prefix(root)
        .unwrap_or(seed_dir)
        .as_str()
        .to_owned();
    let listed = git_text(root, &["ls-files", "-z", "--", &relative])?;
    let mut files = listed
        .split('\0')
        .filter(|name| !name.trim().is_empty())
        .map(|name| root.join(name))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !files.is_empty(),
        "{seed_dir}: no seed is tracked by git; a corpus that lives only on one \
         machine cannot reproduce a campaign (ADR-0020 §1)"
    );
    files.sort();
    let mut hasher = blake3::Hasher::new();
    for path in &files {
        let name = path
            .file_name()
            .with_context(|| format!("{path}: seed has no file name"))?;
        let bytes = fs::read(path).with_context(|| format!("read fuzz seed {path}"))?;
        let mut seed_hash = Sha256::new();
        seed_hash.update(bytes);
        let seed_hash = format!("{:x}", seed_hash.finalize());
        hasher.update(name.as_bytes());
        hasher.update(&[0]);
        hasher.update(seed_hash.as_bytes());
        hasher.update(&[0]);
    }
    Ok((
        u64::try_from(files.len()).context("fuzz seed count exceeds u64")?,
        hasher.finalize().to_hex().to_string(),
    ))
}

fn verify_fuzz_rows(recorded: &[FuzzEvidence], present: &BTreeSet<String>) -> Result<()> {
    let declared = recorded
        .iter()
        .map(|row| row.target.clone())
        .collect::<BTreeSet<_>>();
    if &declared != present {
        anyhow::bail!(
            "fuzz evidence does not cover the targets in the tree: recorded={declared:?}, \
             present={present:?}"
        );
    }
    require_unique(
        recorded.iter().map(|row| row.target.as_str()),
        "fuzz target",
    )?;

    // M17.5 F-17: `log` names a gitignored path, so the counts it would
    // substantiate were unfalsifiable once the run ended. The digest makes a
    // later-produced log checkable.
    for row in recorded {
        if row.log_blake3.len() != 64 || !row.log_blake3.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!(
                "{}: log_blake3 {:?} is not a 64-hex digest, so its log can never be checked",
                row.target,
                row.log_blake3
            );
        }
    }

    // M17.5 F-27 / P1-A09: ADR-0020 §4 requires a SANITIZER-ENABLED campaign
    // and the evidence had no way to say whether one ran. An unsanitized
    // campaign finds crashes it can see and silently misses every
    // memory-safety defect it cannot.
    for row in recorded {
        if !matches!(
            row.sanitizer.as_str(),
            "address" | "memory" | "thread" | "leak"
        ) {
            anyhow::bail!(
                "{}: sanitizer {:?} is not a sanitizer ADR-0020 §4 accepts",
                row.target,
                row.sanitizer
            );
        }
    }

    let total_minutes: u64 = recorded.iter().map(|row| row.seconds / 60).sum();
    if total_minutes < 150 {
        anyhow::bail!("recorded fuzz budget {total_minutes} target-minutes is below 150");
    }
    for row in recorded {
        if row.exit_code != 0 {
            anyhow::bail!(
                "fuzz target {} exited {} with {} artifact(s) — every crash is a failing \
                 result (ADR-0020 §4)",
                row.target,
                row.exit_code,
                row.artifacts
            );
        }
        if row.artifacts != 0 {
            anyhow::bail!(
                "fuzz target {} left {} artifacts",
                row.target,
                row.artifacts
            );
        }
        if row.execs == 0 {
            anyhow::bail!("fuzz target {} recorded zero executions", row.target);
        }
        if row.log.trim().is_empty() {
            anyhow::bail!("fuzz target {} records no log path", row.target);
        }
        // ADR-0020 line 90: "each family also receives one 30-minute
        // sanitizer-enabled fuzz campaign". That is a per-target floor, not just
        // a contribution to the 150 target-minute total — without it one long
        // target could carry the sum while another ran for seconds.
        //
        // The same 30-minute floor is used by family budget reconciliation.
        if row.seconds < 30 * 60 {
            anyhow::bail!(
                "fuzz target {} was budgeted {}s, below the 30-minute per-target floor \
                 (ADR-0020)",
                row.target,
                row.seconds
            );
        }
        // A clean campaign runs to its budget. Finishing far short of it means
        // the target stopped early, and a clean early exit is a contradiction
        // the evidence should not be able to state.
        if row.elapsed_s + 60 < row.seconds {
            anyhow::bail!(
                "fuzz target {} claims a clean {}s campaign but only ran {}s — an early \
                 exit with exit_code 0 and no artifacts is not a coherent result",
                row.target,
                row.seconds,
                row.elapsed_s
            );
        }
    }
    // `haqp_fuzz_campaign.sh` drives every target from ONE `SEED`, so the rows
    // must agree. Checking only for a nonzero seed would let the comment claim
    // more than the code enforced — rows carrying three different seeds describe
    // three campaigns, and none of them is the one the packet points at.
    let seeds = recorded.iter().map(|row| row.seed).collect::<BTreeSet<_>>();
    match seeds.iter().copied().next() {
        None => anyhow::bail!("fuzz evidence records no targets at all"),
        Some(0) => {
            anyhow::bail!(
                "fuzz evidence records no campaign seed, so the lane cannot be reproduced"
            )
        }
        Some(_) if seeds.len() > 1 => anyhow::bail!(
            "fuzz evidence carries {} distinct seeds {seeds:?}; one campaign has one seed, so \
             these rows describe runs the packet cannot point at",
            seeds.len()
        ),
        Some(_) => {}
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn verify_sanitizer_proof(
    root: &Utf8Path,
    row: &FuzzEvidence,
    proof: &SanitizerProof,
    expected: Option<&Provenance>,
) -> Result<()> {
    anyhow::ensure!(
        !proof.build_command.trim().is_empty()
            && proof.build_command
                == format!(
                    "cargo +nightly fuzz build -s {} {}",
                    row.sanitizer, row.target
                ),
        "{}: sanitizer proof build command does not name fuzz build and sanitizer",
        row.target
    );
    anyhow::ensure!(
        !proof.instrumentation_flags.is_empty()
            && proof
                .instrumentation_flags
                .iter()
                .any(|flag| flag.contains(&row.sanitizer)),
        "{}: sanitizer proof has no matching instrumentation flag",
        row.target
    );
    anyhow::ensure!(
        !proof.build_log.trim().is_empty()
            && !proof.build_log_blake3.trim().is_empty()
            && !proof.source_commit.trim().is_empty()
            && !proof.source_tree.trim().is_empty()
            && !proof.build_result.trim().is_empty(),
        "{} sanitizer proof lacks current build provenance; regenerate fuzz evidence",
        row.target
    );
    require_eq(
        &format!("{} sanitizer build_result", row.target),
        &proof.build_result,
        "pass",
    )?;
    require_eq(
        &format!("{} sanitizer build_log", row.target),
        &proof.build_log,
        &format!("conformance/haqp/evidence/build-logs/{}.log", row.target),
    )?;
    require_hex_digest(
        &format!("{} build_log_blake3", row.target),
        &proof.build_log_blake3,
    )?;
    require_git_object_id(
        &format!("{} sanitizer source_commit", row.target),
        &proof.source_commit,
    )?;
    require_git_object_id(
        &format!("{} sanitizer source_tree", row.target),
        &proof.source_tree,
    )?;
    if let Some(expected) = expected {
        require_eq(
            &format!("{} sanitizer source_commit/provenance", row.target),
            &proof.source_commit,
            &expected.fixed_commit,
        )?;
        require_eq(
            &format!("{} sanitizer source_tree/provenance", row.target),
            &proof.source_tree,
            &expected.fixed_tree,
        )?;
    }
    let build_log_path = safe_repo_path(root, &proof.build_log, "sanitizer build log")?;
    let build_log_bytes = read_evidence_bytes(&build_log_path)
        .with_context(|| format!("{build_log_path}: sanitizer build log is required"))?;
    require_eq(
        &format!("{} build_log_blake3", row.target),
        &proof.build_log_blake3,
        blake3::hash(&build_log_bytes).to_hex().as_ref(),
    )?;
    let build_log_text = String::from_utf8_lossy(&build_log_bytes);
    // Cargo's build output does not repeat the shell command when the wrapper
    // records stdout only. The exact command is closed by `build_command`,
    // artifact digests, and fixed-base replay; this log only proves successful
    // build completion.
    // M17.5 F-35: "Finished" alone is not success. cargo prints it for the
    // dependency graph and then reports `error: could not compile` on the final
    // binary, so a FAILED sanitizer build satisfied the success assertion —
    // demonstrated by appending one error line to a real log.
    anyhow::ensure!(
        build_log_text.contains("Finished") || build_log_text.contains("finished"),
        "{} sanitizer build log does not show successful build",
        row.target
    );
    for failure in ["error: could not compile", "error: aborting", "error[E"] {
        anyhow::ensure!(
            !build_log_text.contains(failure),
            "{} sanitizer build log reports {failure:?}; a log that finished the \
             dependency graph and then failed is not a successful build",
            row.target
        );
    }
    require_hex_digest(
        &format!("{} binary_blake3", row.target),
        &proof.binary_blake3,
    )?;
    require_hex_digest(
        &format!("{} runtime_probe_blake3", row.target),
        &proof.runtime_probe_blake3,
    )?;
    anyhow::ensure!(
        proof.runtime_probe_exit_code == 0,
        "{}: sanitizer runtime probe exited {}",
        row.target,
        proof.runtime_probe_exit_code
    );
    let binary_path = safe_repo_path(root, &proof.binary, "sanitizer binary")?;
    let probe_path = safe_repo_path(root, &proof.runtime_probe, "sanitizer runtime probe")?;
    anyhow::ensure!(
        binary_path != probe_path,
        "{}: sanitizer binary and runtime probe must be distinct files",
        row.target
    );
    let binary = Utf8Path::new(&proof.binary);
    anyhow::ensure!(
        binary.is_relative()
            && binary.starts_with("conformance/haqp/evidence/binaries")
            && binary.file_name() == Some(row.target.as_str())
            && !binary
                .components()
                .any(|component| component == camino::Utf8Component::ParentDir),
        "{}: sanitizer proof binary must be tracked evidence",
        row.target
    );
    let probe = Utf8Path::new(&proof.runtime_probe);
    anyhow::ensure!(
        probe.is_relative()
            && probe.starts_with("conformance/haqp/evidence/probes")
            && !probe
                .components()
                .any(|component| component == camino::Utf8Component::ParentDir),
        "{}: sanitizer proof runtime probe must be tracked evidence",
        row.target
    );
    for (label, path, expected) in [
        ("sanitizer binary", binary, &proof.binary_blake3),
        (
            "sanitizer runtime probe",
            probe,
            &proof.runtime_probe_blake3,
        ),
    ] {
        let path = if path == binary {
            binary_path.clone()
        } else {
            probe_path.clone()
        };
        let metadata =
            fs::symlink_metadata(&path).with_context(|| format!("{path}: {label} is required"))?;
        anyhow::ensure!(
            metadata.file_type().is_file(),
            "{path}: {label} must be a regular file"
        );
        let digest = blake3::hash(&fs::read(&path)?).to_hex().to_string();
        require_eq(&format!("{} {label} digest", row.target), expected, &digest)?;
    }
    let binary_bytes = fs::read(&binary_path)?;
    let marker = match row.sanitizer.as_str() {
        "address" | "leak" => b"asan_globals".as_slice(),
        "memory" => b"msan".as_slice(),
        "thread" => b"tsan".as_slice(),
        _ => unreachable!("sanitizer was checked before proof"),
    };
    anyhow::ensure!(
        binary_bytes
            .windows(marker.len())
            .any(|window| window == marker),
        "{}: sanitizer binary has no {} runtime marker",
        row.target,
        row.sanitizer
    );
    anyhow::ensure!(
        binary_bytes
            .windows(row.target.len())
            .any(|window| window == row.target.as_bytes()),
        "{}: sanitizer binary has no target identity marker",
        row.target
    );
    let probe_output = Command::new(&binary_path)
        .arg("-help=1")
        .output()
        .with_context(|| format!("{}: execute sanitizer runtime probe", row.target))?;
    anyhow::ensure!(
        probe_output.status.success(),
        "{}: sanitizer runtime probe execution failed",
        row.target
    );
    let mut probe_bytes = probe_output.stdout;
    probe_bytes.extend_from_slice(&probe_output.stderr);
    anyhow::ensure!(
        String::from_utf8_lossy(&probe_bytes).contains(&row.target),
        "{}: sanitizer runtime probe does not identify target",
        row.target
    );
    let recorded_probe = fs::read(&probe_path)?;
    anyhow::ensure!(
        probe_bytes == recorded_probe,
        "{}: retained sanitizer probe differs from fresh binary execution",
        row.target
    );
    if let Some(provenance) = expected {
        verify_sanitizer_build_replay(root, row, proof, provenance)?;
    }
    Ok(())
}

/// Build sanitizer target from fixed source commit and compare output. Marker
/// strings and self-authored logs cannot prove compiler/runtime instrumentation.
/// The one path and flag set sanitizer binaries are built with, keyed by commit.
///
/// Must match `BUILD_ROOT` / `BUILD_RUSTFLAGS` in scripts/haqp_fuzz_campaign.sh
/// verbatim; the proof records the campaign's values and the replay refuses any
/// that differ, which is how the two are kept from drifting.
/// The environment variable that makes a sanitizer runtime print its flags, and
/// the name it prints. Closed: an unknown sanitizer is refused, not guessed.
fn sanitizer_runtime_witness(sanitizer: &str) -> Result<(&'static str, &'static str)> {
    Ok(match sanitizer {
        "address" => ("ASAN_OPTIONS", "AddressSanitizer"),
        "memory" => ("MSAN_OPTIONS", "MemorySanitizer"),
        "thread" => ("TSAN_OPTIONS", "ThreadSanitizer"),
        "leak" => ("LSAN_OPTIONS", "LeakSanitizer"),
        other => anyhow::bail!("no runtime witness registered for sanitizer {other:?}"),
    })
}

/// Replace the machine-specific source of the `=/cargo` remap with a token.
fn normalize_build_rustflags(flags: &str) -> String {
    flags
        .split_whitespace()
        .map(|flag| match flag.strip_prefix("--remap-path-prefix=") {
            Some(rest) if rest.ends_with("=/cargo") => {
                "--remap-path-prefix=<CARGO_HOME>=/cargo".to_owned()
            }
            _ => flag.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Ask the sanitizer RUNTIME to identify itself (M17.5 F-42). A previous probe
/// ran `-help=1` and required the text to contain `-fsanitize=<san>`; libFuzzer's
/// help is a usage banner and never names the sanitizer, so the committed probe
/// logs contained that string zero times and the check could not pass for any
/// binary. `<SAN>_OPTIONS=help=1` makes the linked runtime print its own flag
/// table, which only happens if the instrumentation is actually present. The
/// target NAME is already bound by the caller's file-name guard; `-seed=1
/// -runs=0` keeps the output reproducible.
///
/// `detect_leaks=0` is the campaign's own policy for traced sanitizer runs
/// (`haqp_fuzz_campaign.sh`): LeakSanitizer stops the world with ptrace and
/// aborts with exit 1 when the process is already traced, and every lane stage
/// now runs under `strace -f` (F-43). The probe runs zero inputs, so leak
/// detection has nothing to observe here; ASan's memory checks stay on. The
/// campaign's probe uses the identical environment, and the recorded log is
/// digest-bound to this output.
/// A detached worktree that never materializes `conformance/corpora` (F-44):
/// the sanitizer build does not reference the corpus, so a build root without
/// it has nothing of the locked data to write, prune or alias. Mirrors the
/// campaign's `git worktree add --no-checkout` + sparse-checkout sequence, so
/// the replay builds the same tree the campaign built.
fn add_sparse_worktree(
    root: &Utf8Path,
    worktree: &Utf8Path,
    commit: &str,
    label: &str,
) -> Result<()> {
    let run = |args: &[&str], cwd: &Utf8Path, what: &str| -> Result<()> {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .with_context(|| format!("{label}: {what}"))?;
        anyhow::ensure!(
            output.status.success(),
            "{label} {what} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(())
    };
    run(
        &[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            "--no-checkout",
            worktree.as_str(),
            commit,
        ],
        root,
        "worktree add",
    )?;
    run(
        &[
            "sparse-checkout",
            "set",
            "--no-cone",
            "/*",
            "!/conformance/corpora/",
        ],
        worktree,
        "sparse-checkout",
    )?;
    run(
        &["checkout", "--quiet", "--detach", commit],
        worktree,
        "checkout",
    )?;
    anyhow::ensure!(
        !worktree.join("conformance/corpora").exists(),
        "{label}: sparse worktree still materialized conformance/corpora"
    );
    Ok(())
}

fn probe_sanitizer_runtime(
    candidate: &Utf8Path,
    worktree: &Utf8Path,
    row: &FuzzEvidence,
) -> Result<(Vec<u8>, &'static str)> {
    let (options_env, runtime_name) = sanitizer_runtime_witness(&row.sanitizer)?;
    let probe = Command::new(candidate)
        .env(options_env, "help=1:detect_leaks=0")
        .args(["-runs=0", "-seed=1"])
        .current_dir(worktree)
        .output()
        .with_context(|| format!("probe replayed sanitizer binary {}", row.target))?;
    // Carry the runtime's own words: the first lane that hit F-43 said only
    // "probe failed" and hid "LeakSanitizer does not work under ptrace".
    anyhow::ensure!(
        probe.status.success(),
        "replayed sanitizer runtime probe failed for {} ({}): {}",
        row.target,
        probe.status,
        String::from_utf8_lossy(&probe.stderr)
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .unwrap_or("<no stderr>")
    );
    let mut probe_bytes = probe.stdout;
    probe_bytes.extend_from_slice(&probe.stderr);
    Ok((probe_bytes, runtime_name))
}

fn sanitizer_build_canon(fixed_commit: &str) -> (Utf8PathBuf, String) {
    let root = Utf8PathBuf::from(format!("/var/tmp/liminal-haqp-build/{fixed_commit}"));
    let home = std::env::var("HOME").unwrap_or_default();
    let flags = format!(
        "-Zremap-cwd-prefix=/liminal --remap-path-prefix={root}=/liminal --remap-path-prefix={home}/.cargo=/cargo"
    );
    (root, flags)
}

fn verify_sanitizer_build_replay(
    root: &Utf8Path,
    row: &FuzzEvidence,
    proof: &SanitizerProof,
    provenance: &Provenance,
) -> Result<()> {
    // Rebuild at the CANONICAL path the campaign built at (M17.5 F-42), not a
    // scratch worktree. Measured: the same commit built at two paths differs in
    // .rodata by its codegen-unit name, because cargo hashes the fuzz crate's
    // absolute manifest path into `-C metadata`; two clean builds at one fixed
    // path are byte-identical. The path and flags are closed here and the proof
    // must record the same, so neither side can drift from the other.
    let (build_root, build_rustflags) = sanitizer_build_canon(&provenance.fixed_commit);
    require_eq(
        &format!("{} sanitizer build_root", row.target),
        &proof.build_root,
        build_root.as_str(),
    )?;
    // The cargo-home remap's SOURCE is the campaign machine's $HOME and does
    // not shape a single byte -- its TARGET (/cargo) does. Comparing the raw
    // string would refuse a byte-identical build from any machine with a
    // different home directory (HEAD review). Compare with the source
    // normalized; the build-root remap and the cwd prefix must still match
    // exactly, because those DO determine the bytes.
    require_eq(
        &format!("{} sanitizer build_rustflags", row.target),
        &normalize_build_rustflags(&proof.build_rustflags),
        &normalize_build_rustflags(&build_rustflags),
    )?;
    // Hold a SHARED lock on the build base while this tree is in use: the
    // campaign prunes sibling commits' trees under an exclusive lock, so a
    // replay mid-build cannot have its worktree removed from under it (HEAD
    // review). std's File::lock_shared is stable on this toolchain.
    let base = build_root
        .parent()
        .context("canonical build root has a parent")?;
    fs::create_dir_all(base)?;
    let lock = fs::File::create(base.join(".lock")).context("open sanitizer build lock")?;
    lock.lock_shared()
        .context("shared-lock sanitizer build base")?;
    let worktree = build_root.clone();
    if !worktree.join(".git").exists() {
        if worktree.exists() {
            fs::remove_dir_all(&worktree).with_context(|| format!("clear stale {worktree}"))?;
        }
        if let Some(parent) = worktree.parent() {
            fs::create_dir_all(parent)?;
        }
        add_sparse_worktree(
            root,
            &worktree,
            &provenance.fixed_commit,
            "sanitizer replay",
        )?;
    }
    fs::create_dir_all(worktree.join("fuzz/.cargo"))?;
    fs::write(
        worktree.join("fuzz/.cargo/config.toml"),
        "[unstable]\ntrim-paths = true\n\n[profile.release]\ntrim-paths = \"all\"\n",
    )?;
    let output = Command::new("cargo")
        .current_dir(&worktree)
        .env("RUSTFLAGS", &build_rustflags)
        .args([
            "+nightly",
            "fuzz",
            "build",
            "-s",
            row.sanitizer.as_str(),
            row.target.as_str(),
        ])
        .output()
        .with_context(|| format!("replay sanitizer build for {}", row.target))?;
    anyhow::ensure!(
        output.status.success(),
        "replayed sanitizer build failed for {}: {}",
        row.target,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let candidate = find_named_files(&worktree.join("fuzz/target"), &row.target)?
        .into_iter()
        .find(|path| path.file_name() == Some(row.target.as_str()))
        .with_context(|| format!("replayed sanitizer build produced no {} binary", row.target))?;
    let digest = blake3::hash(&fs::read(&candidate)?).to_hex().to_string();
    require_eq(
        &format!("{} replayed sanitizer binary digest", row.target),
        &proof.binary_blake3,
        &digest,
    )?;
    let (probe_bytes, runtime_name) = probe_sanitizer_runtime(&candidate, &worktree, row)?;
    let probe_text = String::from_utf8_lossy(&probe_bytes);
    anyhow::ensure!(
        probe_text.contains(runtime_name),
        "replayed binary for {} does not carry the {} runtime ({runtime_name} absent from its help)",
        row.target,
        row.sanitizer
    );
    Ok(())
}

fn find_named_files(root: &Utf8Path, name: &str) -> Result<Vec<Utf8PathBuf>> {
    let mut found = Vec::new();
    if !root.exists() {
        return Ok(found);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
            anyhow::anyhow!("non-UTF-8 path while finding {name}: {}", path.display())
        })?;
        if entry.file_type()?.is_dir() {
            found.extend(find_named_files(&path, name)?);
        } else if path.file_name() == Some(name) {
            found.push(path);
        }
    }
    Ok(found)
}

/// Counts and exit states are only useful if their retained logs are the bytes
/// the campaign actually produced. The release artifact keeps one log per
/// target under the tracked evidence directory and binds its digest here.
fn verify_fuzz_logs(root: &Utf8Path, recorded: &[FuzzEvidence]) -> Result<()> {
    for row in recorded {
        let expected = format!("conformance/haqp/evidence/logs/{}.log", row.target);
        require_eq("fuzz log path", &row.log, &expected)?;
        let path = root.join(&expected);
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("{path}: retained fuzz log is required"))?;
        anyhow::ensure!(
            metadata.file_type().is_file(),
            "{path}: retained fuzz log must be a regular file",
        );
        let bytes = fs::read(&path).with_context(|| format!("read retained fuzz log {path}"))?;
        let digest = blake3::hash(&bytes).to_hex().to_string();
        require_eq("fuzz log_blake3", &row.log_blake3, &digest)?;
        verify_fuzz_log_metrics(row, &bytes)?;
    }
    Ok(())
}

fn verify_fuzz_log_metrics(row: &FuzzEvidence, bytes: &[u8]) -> Result<()> {
    let text = String::from_utf8_lossy(bytes);
    let stat_execs = text
        .lines()
        .filter_map(|line| line.strip_prefix("stat::number_of_executed_units:"))
        .next_back()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .with_context(|| {
            format!(
                "{}: retained fuzz log has no execution-count footer",
                row.target
            )
        })?;
    require_eq(
        &format!("{} log execution count", row.target),
        &stat_execs.to_string(),
        &row.execs.to_string(),
    )?;
    let done = text
        .lines()
        .filter_map(|line| line.strip_prefix("Done "))
        .next_back()
        .with_context(|| format!("{}: retained fuzz log has no Done footer", row.target))?;
    let done_execs = done
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .with_context(|| format!("{}: malformed Done execution count", row.target))?;
    let done_elapsed = done
        .split(" in ")
        .nth(1)
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
        .with_context(|| format!("{}: malformed Done elapsed seconds", row.target))?;
    anyhow::ensure!(
        done_execs == row.execs,
        "{}: log Done count {} differs from evidence {}",
        row.target,
        done_execs,
        row.execs
    );
    anyhow::ensure!(
        // Fuzzer-reported duration may omit setup/teardown; allow one minute
        // for launcher overhead while still binding elapsed evidence to log.
        done_elapsed <= row.elapsed_s && row.elapsed_s <= done_elapsed.saturating_add(60),
        "{}: log elapsed {}s is not bound to evidence elapsed {}s",
        row.target,
        done_elapsed,
        row.elapsed_s
    );
    anyhow::ensure!(
        text.contains(&format!("-max_total_time={}", row.seconds))
            && text.contains(&format!("-seed={}", row.seed)),
        "{}: retained log command is not bound to recorded budget/seed",
        row.target
    );
    Ok(())
}

/// Bind the locked-corpus prohibition to an OS-level path audit rather than
/// trusting `locked_acceptance_corpora_touched: false`. The audit records only
/// normalized paths, never corpus bytes; forbidden held-out paths fail closed.
fn verify_corpus_access_audit(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/corpus-access.json");
    let bytes = fs::read(&path).with_context(|| {
        format!("{path}: committed corpus-access audit is required; run the traced fuzz lane")
    })?;
    let audit: CorpusAccessAudit =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    require_eq(
        "corpus access audit schema_version",
        &audit.schema_version,
        "haqp-corpus-access-v2",
    )?;
    require_eq(
        "corpus access audit tracer",
        &audit.tracer,
        "strace-open-paths",
    )?;
    require_git_object_id("corpus access source_commit", &audit.source_commit)?;
    require_git_object_id("corpus access source_tree", &audit.source_tree)?;
    if let Some(provenance) = &packet.provenance {
        require_eq(
            "corpus access source_commit/provenance",
            &audit.source_commit,
            &provenance.fixed_commit,
        )?;
        require_eq(
            "corpus access source_tree/provenance",
            &audit.source_tree,
            &provenance.fixed_tree,
        )?;
    }

    let expected = packet
        .generated
        .iter()
        .flat_map(|family| family.fuzz_targets.iter().cloned())
        .collect::<BTreeSet<_>>();
    let actual = audit
        .targets
        .iter()
        .map(|row| row.target.clone())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        actual == expected,
        "corpus access audit target set differs: recorded={actual:?}, expected={expected:?}"
    );
    require_unique(
        audit.targets.iter().map(|row| row.target.as_str()),
        "corpus access audit target",
    )?;
    verify_corpus_audit_campaign_binding(root, &audit)?;
    // Scope traces are required only of a qualified packet.
    if packet.provenance.is_some() {
        verify_corpus_scope_traces(root, &audit)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn verify_corpus_scope_traces(root: &Utf8Path, audit: &CorpusAccessAudit) -> Result<()> {
    verify_corpus_scope_presence(audit)?;
    verify_corpus_scope_rows(root, audit)
}

fn verify_corpus_scope_presence(audit: &CorpusAccessAudit) -> Result<()> {
    const QUALIFICATION_SCOPES: [&str; 8] = [
        "ci",
        "canaries",
        "generated",
        "crash",
        "replay",
        "mutation",
        "reviews",
        "fuzz",
    ];
    let expected = QUALIFICATION_SCOPES.into_iter().collect::<BTreeSet<_>>();
    let actual = audit
        .scope_traces
        .iter()
        .map(|row| row.scope.as_str())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        actual == expected,
        "corpus scope command does not cover lane: recorded={actual:?}, expected={expected:?}"
    );
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn verify_corpus_scope_rows(root: &Utf8Path, audit: &CorpusAccessAudit) -> Result<()> {
    verify_corpus_scope_replays(root, &audit.scope_traces, &audit.targets)?;
    require_unique(
        audit.scope_traces.iter().map(|row| row.scope.as_str()),
        "corpus audit scope",
    )?;
    for row in &audit.scope_traces {
        anyhow::ensure!(
            row.command.contains("strace") && row.command.contains("-f"),
            "{} corpus scope is not recursively traced",
            row.scope
        );
        require_eq("corpus scope exit_code", &row.exit_code.to_string(), "0")?;
        require_eq("corpus scope result", &row.result, "pass")?;
        require_hex_digest(
            &format!("{} corpus scope trace_blake3", row.scope),
            &row.trace_blake3,
        )?;
        anyhow::ensure!(
            row.trace_pid > 0 && row.trace_exit_code == 0 && row.trace_complete,
            "{} corpus scope trace lacks successful traced-process receipt",
            row.scope
        );
        require_eq("corpus scope tracer", &row.tracer_binary, "strace")?;
        require_eq(
            "corpus scope tracer version",
            &row.tracer_version,
            "conformance/haqp/evidence/access/strace.version",
        )?;
        require_hex_digest(
            &format!("{} corpus scope tracer_version_blake3", row.scope),
            &row.tracer_version_blake3,
        )?;
        require_hex_digest(
            &format!("{} corpus scope observed_paths_blake3", row.scope),
            &row.observed_paths_blake3,
        )?;
        let tracer_version_path =
            safe_repo_path(root, &row.tracer_version, "corpus scope tracer version")?;
        let tracer_version_bytes = fs::read(&tracer_version_path)
            .with_context(|| format!("{tracer_version_path}: tracer version required"))?;
        require_eq(
            &format!("{} corpus scope tracer_version_blake3", row.scope),
            &row.tracer_version_blake3,
            blake3::hash(&tracer_version_bytes).to_hex().as_ref(),
        )?;
        // Stored compressed, as the fuzz-target traces are: a `just ci` trace
        // is hundreds of MB raw. read_evidence_bytes decompresses; the digest
        // is over the raw bytes.
        let expected_path = format!(
            "conformance/haqp/evidence/access/scopes/{}.trace.zst",
            row.scope
        );
        require_eq("corpus scope trace path", &row.trace, &expected_path)?;
        let path = safe_repo_path(root, &row.trace, "corpus scope trace")?;
        // One streaming pass over the remainder: a `just ci` trace is a
        // gigabyte raw and the fuzz stage's is larger. The digest is over the
        // raw bytes exactly as before; the pid receipts come from the same
        // pass. Observed paths, resolution and the locked-corpus refusal were
        // judged over the union (remainder + carved parts) in
        // verify_corpus_scope_replays above.
        let scan = scan_scope_trace_file(&path, &format!("{} corpus scope", row.scope))?;
        require_eq(
            &format!("{} corpus scope trace_blake3", row.scope),
            &row.trace_blake3,
            &scan.raw_blake3,
        )?;
        anyhow::ensure!(
            scan.pids.contains(&row.trace_pid),
            "{} scope trace has no event from traced PID {}",
            row.scope,
            row.trace_pid
        );
        anyhow::ensure!(
            scan.exited_zero.contains(&row.trace_pid),
            "{} scope trace has no successful exit for PID {}",
            row.scope,
            row.trace_pid
        );
        let canonical_root = fs::canonicalize(root.as_std_path())
            .with_context(|| format!("{}: canonicalize repository root", row.scope))?;
        require_eq(
            &format!("{} corpus scope trace_root", row.scope),
            &row.trace_root,
            &canonical_root.to_string_lossy(),
        )?;
        let binding = blake3::hash(
            format!(
                "{}\0{}\0{}\0{}\0{}",
                row.command,
                row.trace_pid,
                row.trace_exit_code,
                row.trace_blake3,
                row.observed_paths_blake3
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        require_eq(
            &format!("{} corpus scope process_binding", row.scope),
            &row.process_binding,
            &binding,
        )?;
        let label = format!("{} corpus scope", row.scope);
        if let Some(path) = locked_corpus_write(root, &scan.locked_write_candidates, &label)? {
            anyhow::bail!("{label} trace wrote to the locked corpus ({path})");
        }
    }
    Ok(())
}

/// Verify each scope's CAPTURED trace, rather than re-running the campaign.
///
/// This used to re-execute every scope's lane command inside a worktree —
/// `just ci`, `haq generate --cases 100000`, `cargo test --workspace`,
/// `just haq-blind-review` and `scripts/haqp_fuzz_campaign.sh 1800`. That made
/// `haq-verify` a second complete campaign, with a second fuzz run and a second
/// pair of paid model reviews inside every lane, and it threatened §7's
/// eight-hour ceiling with work that produced no new evidence.
///
/// It had never fired: `scope_traces` was empty, so the loop body never ran.
/// Populating it — which `verify_corpus_scope_presence` requires — would have
/// switched it on.
///
/// Ruling (Brian, 2026-09-04): the trace is evidence FROM the campaign. The
/// lane captures each scope under strace as it actually runs, and the gate
/// checks the recorded bytes: the command matches the closed lane registry, the
/// path digest matches, no stage wrote to the repository's locked corpus and no
/// carved fuzz binary touched it at all (F-44 ruling, 2026-09-05), and every
/// traced corpus path resolves inside the tree. What is lost is the reproducibility claim that
/// re-execution made; what is gained is evidence describing the run that
/// happened rather than a different one.
fn verify_corpus_scope_replays(
    root: &Utf8Path,
    rows: &[CorpusScopeTrace],
    targets: &[CorpusAccessAuditTarget],
) -> Result<()> {
    for row in rows {
        let expected_command = format!(
            "strace -f -q -e trace=%file -o target/haqp/scope-{}.trace {}",
            row.scope,
            scope_lane_command(&row.scope)?
        );
        require_eq(
            &format!("{} corpus scope command", row.scope),
            &row.command,
            &expected_command,
        )?;
        verify_scope_trace_parts(row, targets)?;
        let (remainder, open, forbidden) = scan_scope_trace_union(root, row)?;
        if let Some(fragment) = forbidden {
            anyhow::bail!(
                "{} corpus scope trace opened a locked path ({fragment})",
                row.scope
            );
        }
        let label = format!("{} corpus scope", row.scope);
        if let Some(path) = locked_corpus_write(root, &remainder.locked_write_candidates, &label)? {
            anyhow::bail!("{label} trace wrote to the locked corpus ({path})");
        }
        require_eq(
            &format!("{} observed_paths_blake3", row.scope),
            &row.observed_paths_blake3,
            &scope_trace_paths_digest_from(&open),
        )?;
        require_hex_digest(
            &format!("{label} resolved_paths_blake3"),
            &row.resolved_paths_blake3,
        )?;
        let access = if row.scope == "fuzz" {
            CorpusAccess::Required
        } else {
            CorpusAccess::IfPresent
        };
        let actual = resolved_corpus_paths_digest_from(
            root,
            root.as_str(),
            &open,
            &label,
            access,
            LockedCorpus::WritesOnly,
        )?;
        require_eq(
            &format!("{label} resolved_paths_blake3"),
            &actual,
            &row.resolved_paths_blake3,
        )?;
    }
    Ok(())
}

/// The exact command the lane runs for each scope, under strace.
///
/// These must be what `haqp_qualify.sh` actually invokes, verbatim: the gate
/// requires `row.command` to equal `strace ... <this>`, so a registry that
/// describes a tidier command than the one that ran would fail the campaign it
/// is meant to describe. The fuzz entry therefore carries its output path, and
/// the wrappers are the `just` recipes rather than the cargo lines behind them.
fn scope_lane_command(scope: &str) -> Result<&'static str> {
    match scope {
        "ci" => Ok("just ci"),
        "canaries" => Ok("just haq-canaries"),
        "generated" => Ok("just haq-generated"),
        "crash" => Ok("just haq-crash"),
        "replay" => Ok("cargo test -q --workspace"),
        "mutation" => Ok("just haq-mutants"),
        "reviews" => Ok("just haq-blind-review"),
        "fuzz" => Ok("scripts/haqp_fuzz_campaign.sh 1800 conformance/haqp/evidence/fuzz.json"),
        _ => anyhow::bail!("unknown corpus scope {scope}"),
    }
}

/// Deterministic, read-only scope probe used by qualification-wide strace.
/// It intentionally touches only committed, non-held-out files for its lane.
pub fn run_scope_probe_repo(root: &Utf8Path, scope: &str) -> Result<()> {
    let files: &[&str] = match scope {
        "ci" => &["Cargo.toml", "Cargo.lock", "justfile"],
        "canaries" => &[
            "conformance/haqp/packet.json",
            "docs/execution/phase1-suite-review.md",
        ],
        "generated" | "mutation" => &[
            "crates/liminal-xtask/src/haq.rs",
            "conformance/haqp/packet.json",
        ],
        "crash" => &[
            "crates/liminal-jurisdiction/src/repair.rs",
            "conformance/haqp/packet.json",
        ],
        "replay" | "reviews" => &["conformance/haqp/packet.json"],
        "fuzz" => &["fuzz/Cargo.toml", "fuzz/fuzz_targets/cst_parse.rs"],
        _ => anyhow::bail!("unknown corpus scope {scope}"),
    };
    for path in files {
        let bytes =
            fs::read(root.join(path)).with_context(|| format!("scope probe read {path}"))?;
        anyhow::ensure!(!bytes.is_empty(), "scope probe read empty file {path}");
    }
    println!("scope-probe:{scope}:pass");
    Ok(())
}

#[cfg(test)]
fn scope_trace_paths_digest(bytes: &[u8]) -> String {
    scope_trace_paths_digest_from(&scope_trace_open_paths(bytes))
}

fn scope_trace_paths_digest_from(open_paths: &BTreeSet<String>) -> String {
    blake3::hash(
        open_paths
            .iter()
            .map(|path| normalize_scope_trace_path(path))
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    )
    .to_hex()
    .to_string()
}

/// The path argument of one traced file syscall, lexical, as strace printed it.
/// Every file syscall strace's `%file` class prints a path for. Write-class
/// entries can change what a path names; the open family is write-class only
/// with a writing flag.
const FILE_SYSCALLS: [&str; 43] = [
    "open(",
    "openat(",
    "openat2(",
    "creat(",
    "stat(",
    "statx(",
    "lstat(",
    "fstatat(",
    "newfstatat(",
    "readlink(",
    "readlinkat(",
    "access(",
    "faccessat(",
    "faccessat2(",
    "execve(",
    "execveat(",
    "name_to_handle_at(",
    "truncate(",
    "utimensat(",
    "unlink(",
    "unlinkat(",
    "rename(",
    "renameat(",
    "renameat2(",
    "mkdir(",
    "mkdirat(",
    "rmdir(",
    "chdir(",
    "link(",
    "linkat(",
    "symlink(",
    "symlinkat(",
    "chmod(",
    "fchmodat(",
    "chown(",
    "lchown(",
    "fchownat(",
    "mknod(",
    "mknodat(",
    "setxattr(",
    "lsetxattr(",
    "removexattr(",
    "lremovexattr(",
];
const OPEN_SYSCALLS: [&str; 3] = ["open(", "openat(", "openat2("];
const OPEN_WRITE_FLAGS: [&str; 5] = ["O_WRONLY", "O_RDWR", "O_CREAT", "O_TRUNC", "O_APPEND"];
const WRITE_SYSCALLS: [&str; 26] = [
    "creat(",
    "truncate(",
    "utimensat(",
    "unlink(",
    "unlinkat(",
    "rename(",
    "renameat(",
    "renameat2(",
    "mkdir(",
    "mkdirat(",
    "rmdir(",
    "link(",
    "linkat(",
    "symlink(",
    "symlinkat(",
    "chmod(",
    "fchmodat(",
    "chown(",
    "lchown(",
    "fchownat(",
    "mknod(",
    "mknodat(",
    "setxattr(",
    "lsetxattr(",
    "removexattr(",
    "lremovexattr(",
];
/// Syscalls whose second string is also a path being written (the target).
/// Write-class syscalls that name a DESCRIPTOR rather than a path. Their
/// path-bearing siblings are in `WRITE_SYSCALLS`; these resolve through the
/// descriptor map instead (blind pass 1 at `ec045588`, A06).
const FD_WRITE_SYSCALLS: [&str; 7] = [
    "fchmod(",
    "fchown(",
    "ftruncate(",
    "fsetxattr(",
    "fremovexattr(",
    "futimens(",
    "fallocate(",
];

const TWO_PATH_SYSCALLS: [&str; 7] = [
    "rename(",
    "renameat(",
    "renameat2(",
    "link(",
    "linkat(",
    "symlink(",
    "symlinkat(",
];

/// The path arguments of one traced file syscall, lexical, as strace printed
/// them, each with whether the call can change what the path names (F-44).
/// Two-path syscalls yield both strings; a symlink's target counts as written
/// because a link INTO the locked corpus is the second-name attack. strace
/// prints one syscall per line, so the earliest needle is the line's call.
fn scope_trace_line_accesses(line: &str) -> Vec<(String, bool)> {
    scope_trace_line_arguments(line)
        .into_iter()
        .map(|(path, write, _)| (path, write))
        .collect()
}

/// The same, plus the directory fd each path was resolved against: the nearest
/// preceding bare integer argument, `None` for `AT_FDCWD` and for calls that
/// take no dirfd. Blind pass 1 at 5fb1b57 (A10, A11): only `openat`'s FIRST
/// argument was read, so `renameat`'s destination dirfd was ignored and a
/// descriptor opened without `O_DIRECTORY` was dropped from the map entirely.
fn scope_trace_line_arguments(line: &str) -> Vec<(String, bool, Option<i64>)> {
    let Some((open, needle)) = FILE_SYSCALLS
        .into_iter()
        .filter_map(|needle| line.find(needle).map(|index| (index, needle)))
        .min_by_key(|(index, _)| *index)
    else {
        return Vec::new();
    };
    let rest = &line[open..];
    let write = WRITE_SYSCALLS.contains(&needle)
        || (OPEN_SYSCALLS.contains(&needle)
            && OPEN_WRITE_FLAGS.iter().any(|flag| rest.contains(flag)));
    let wanted = if TWO_PATH_SYSCALLS.contains(&needle) {
        2
    } else {
        1
    };
    let mut out = Vec::with_capacity(wanted);
    // The argument list, past the syscall name: without this the text before a
    // path reads `openat(7`, whose last comma-separated piece is the name
    // glued to the fd and parses as nothing.
    let args = &rest[needle.len()..];
    let mut cursor = args;
    let mut consumed = 0usize;
    for _ in 0..wanted {
        let Some(start) = cursor.find('"') else { break };
        let Some(end) = cursor[start + 1..].find('"') else {
            break;
        };
        let dirfd = args[..consumed + start]
            .rsplit(',')
            .find_map(|argument| argument.trim().parse::<i64>().ok());
        out.push((cursor[start + 1..start + 1 + end].to_owned(), write, dirfd));
        consumed += start + 1 + end + 1;
        cursor = &cursor[start + 1 + end + 1..];
    }
    out
}

fn scope_trace_open_paths(bytes: &[u8]) -> BTreeSet<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .flat_map(|line| {
            scope_trace_line_accesses(line)
                .into_iter()
                .map(|(path, _)| path)
        })
        .collect()
}

/// A per-target trace carved from a stage trace by pid (M17.5 F-43). The
/// lane's stage tracer holds the one ptrace slot a process may have, so the
/// campaign cannot trace its fuzz binaries itself: each binary's lines are
/// carved into the target's own trace and the stage keeps the remainder. The
/// scope row names its parts so the gate judges the union, and no line is
/// stored twice.
#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ScopeTracePart {
    trace: String,
    trace_blake3: String,
}

/// What one streaming pass over a captured trace yields. A 30-minute ASan
/// trace is gigabytes raw and the whole fuzz stage's is larger; nothing here
/// holds more than one line and the sets.
#[derive(Debug, Default)]
struct ScopeTraceScan {
    /// Every path argument of a file syscall, lexical, as strace printed it.
    open_paths: BTreeSet<String>,
    /// Pids that emitted at least one line.
    pids: BTreeSet<u32>,
    /// Pids whose `+++ exited with 0 +++` line was seen.
    exited_zero: BTreeSet<u32>,
    /// The first locked-corpus fragment seen anywhere in the text, if any.
    /// The carved fuzz binaries' traces may not carry it at all (F-44).
    forbidden: Option<&'static str>,
    /// Write-class paths carrying a locked-corpus fragment: the ones the
    /// F-44 write rule resolves against the repository.
    locked_write_candidates: Vec<String>,
    /// blake3 of the raw bytes, exactly what `read_evidence_bytes` would hash.
    raw_blake3: String,
}

const FORBIDDEN_TRACE_FRAGMENTS: [&str; 2] = ["heldout", "conformance/corpora"];

/// `chdir("x") = 0` moves this pid; a failed chdir moves nothing, and a
/// relative target moves it relative to where it already stood (A07).
/// Give a line's relative paths the directory they were actually opened
/// against: a directory fd if the call names one, else the pid's cwd (A07,
/// A12). An absolute path is already anchored.
fn anchor_relative_paths(
    pid: u32,
    dir_fds: &BTreeMap<(u32, i64), String>,
    cwds: &BTreeMap<u32, String>,
    arguments: &mut [(String, bool, Option<i64>)],
) {
    // Each path is anchored against ITS OWN dirfd — `renameat`'s destination
    // has a different one from its source — and falls back to the pid's cwd.
    for (path, _, dirfd) in arguments.iter_mut() {
        if path.starts_with('/') {
            continue;
        }
        if let Some(base) = dirfd
            .and_then(|fd| dir_fds.get(&(pid, fd)))
            .or_else(|| cwds.get(&pid))
        {
            *path = format!("{base}/{path}");
        }
    }
}

/// The child pid a successful `clone`, `fork` or `vfork` line reports, if any.
/// strace prints the new pid as the call's return value (A07).
fn clone_child_pid(line: &str) -> Option<u32> {
    let names = ["clone(", "clone3(", "fork(", "vfork("];
    if !names.iter().any(|name| line.contains(name)) {
        return None;
    }
    let child = line.rsplit(" = ").next()?.trim();
    let child = child.split_whitespace().next()?;
    child.parse::<u32>().ok().filter(|pid| *pid > 0)
}

/// The (source, duplicate) descriptor pair a successful `dup`, `dup2`, `dup3`
/// or `fcntl(F_DUPFD)` line reports (A07). strace prints the new descriptor as
/// the return value.
/// Blind pass 1 at 9eca4f0 (A07): only open-derived descriptors were tracked,
/// so `dup`ing a locked directory fd and writing through the duplicate
/// anchored against nothing. A duplicate names what its source named.
fn record_duplicated_fd(
    line: &str,
    pid: u32,
    returned_fd: Option<i64>,
    dir_fds: &mut BTreeMap<(u32, i64), String>,
) {
    if let Some((from, to)) = duplicated_fd(line, returned_fd)
        && let Some(dir) = dir_fds.get(&(pid, from)).cloned()
    {
        dir_fds.insert((pid, to), dir);
    }
}

fn duplicated_fd(line: &str, returned_fd: Option<i64>) -> Option<(i64, i64)> {
    let to = returned_fd.filter(|fd| *fd >= 0)?;
    let call = ["dup(", "dup2(", "dup3(", "fcntl("]
        .into_iter()
        .find(|name| line.contains(name))?;
    if call == "fcntl(" && !line.contains("F_DUPFD") {
        return None;
    }
    // strace prints these as `dup2(3, 7) = 7` — integer arguments only, so the
    // first comma-separated piece is the source descriptor.
    let args = line.split_once(call)?.1;
    let from = args
        .split(',')
        .next()?
        .trim_end_matches(')')
        .trim()
        .parse::<i64>()
        .ok()?;
    Some((from, to))
}

fn record_chdir(
    line: &str,
    pid: u32,
    dir_fds: &BTreeMap<(u32, i64), String>,
    cwds: &mut BTreeMap<u32, String>,
) {
    if !line.contains("chdir(") || !line.trim_end().ends_with("= 0") {
        return;
    }
    // Blind pass 1 at 5fb1b57 (A07): `fchdir` was skipped, which left a STALE
    // cwd and unanchored every relative path after it. It names a directory by
    // fd, so the pid moves wherever that fd was opened; an fd this trace never
    // saw opened leaves the cwd UNKNOWN rather than stale.
    if line.contains("fchdir(") {
        let fd = line
            .split_once("fchdir(")
            .and_then(|(_, rest)| rest.split(')').next())
            .and_then(|token| token.trim().parse::<i64>().ok());
        match fd.and_then(|fd| dir_fds.get(&(pid, fd))) {
            Some(dir) => cwds.insert(pid, dir.clone()),
            None => cwds.remove(&pid),
        };
        return;
    }
    let Some((target, _)) = scope_trace_line_accesses(line).first().cloned() else {
        return;
    };
    let resolved = if target.starts_with('/') {
        target
    } else {
        cwds.get(&pid)
            .map_or_else(|| target.clone(), |cwd| format!("{cwd}/{target}"))
    };
    cwds.insert(pid, resolved);
}

/// The path an fd-only write syscall on `line` names, resolved through the
/// descriptor map. Blind pass 1 at `ec045588` (A06): `chmod` names a path and
/// was listed; `fchmod` names a descriptor and was not, so changing the locked
/// corpus's mode through an open descriptor produced no write candidate.
fn fd_write_target(
    line: &str,
    pid: u32,
    open_fds: &BTreeMap<(u32, i64), String>,
) -> Option<String> {
    let call = FD_WRITE_SYSCALLS
        .iter()
        .find(|call| line.contains(**call))?;
    let fd = line
        .split_once(*call)?
        .1
        .split([',', ')'])
        .next()?
        .trim()
        .parse::<i64>()
        .ok()?;
    open_fds.get(&(pid, fd)).cloned()
}

fn scan_scope_trace<R: std::io::BufRead>(mut reader: R) -> Result<ScopeTraceScan> {
    let mut scan = ScopeTraceScan::default();
    let mut hasher = blake3::Hasher::new();
    let mut buf = Vec::new();
    // Blind pass 1 at f360e90 (A07): `openat(AT_FDCWD, ".../heldout", O_DIRECTORY) = 7`
    // followed by `openat(7, "x.md", O_WRONLY)` named the locked corpus only on
    // the permitted read; the write carried no fragment. Directory fds are
    // remembered per pid from their open's return value, and a path relative
    // to one is judged under that directory. An `<unfinished ...>` open is
    // completed by its `<... openat resumed>) = fd` line.
    let mut dir_fds: BTreeMap<(u32, i64), String> = BTreeMap::new();
    let mut unfinished: BTreeMap<u32, (String, bool)> = BTreeMap::new();
    // Blind pass 1 at aa00d41 (A07): a `chdir` into the locked corpus made
    // every later relative write nameless — no fragment, and resolution
    // against the repository root put it somewhere harmless. The cwd each pid
    // announces is remembered, and its relative paths resolve there.
    let mut cwds: BTreeMap<u32, String> = BTreeMap::new();
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        hasher.update(&buf);
        // Exactly `str::lines`: one trailing `\n`, then one `\r`, stripped.
        let line = buf.strip_suffix(b"\n").unwrap_or(&buf);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let line = String::from_utf8_lossy(line);
        if scan.forbidden.is_none() {
            let lower = line.to_ascii_lowercase();
            scan.forbidden = FORBIDDEN_TRACE_FRAGMENTS
                .into_iter()
                .find(|fragment| lower.contains(fragment));
        }
        let mut tokens = line.splitn(2, ' ');
        if let Some(pid) = tokens.next().and_then(|token| token.parse::<u32>().ok()) {
            scan.pids.insert(pid);
            if tokens.next() == Some("+++ exited with 0 +++") {
                scan.exited_zero.insert(pid);
            }
            // A pid that exited holds no directory fds; a later process
            // reusing the number must not inherit them.
            if line.contains(" +++ exited with ") || line.contains(" +++ killed by ") {
                dir_fds.retain(|(owner, _), _| *owner != pid);
            }
        }
        let pid = line
            .split(' ')
            .next()
            .and_then(|token| token.parse::<u32>().ok());
        let returned_fd = line
            .rsplit(" = ")
            .next()
            .and_then(|tail| tail.trim().parse::<i64>().ok());
        if let (Some(pid), true) = (pid, line.contains("<... openat resumed>"))
            && let (Some((path, is_dir)), Some(fd)) = (unfinished.remove(&pid), returned_fd)
        {
            if is_dir && fd >= 0 {
                dir_fds.insert((pid, fd), path);
            } else {
                dir_fds.remove(&(pid, fd));
            }
        }
        // Parsed ONCE and carried: `anchor_relative_paths` used to re-parse the
        // line and zip positionally against this vector, which is only safe
        // while the two parses agree on how many entries they yield (review of
        // `7e4ba39`). Passing the dirfds through removes the coupling.
        let mut arguments = scope_trace_line_arguments(&line);
        if let Some(pid) = pid {
            anchor_relative_paths(pid, &dir_fds, &cwds, &mut arguments);
        }
        let mut accesses = arguments
            .into_iter()
            .map(|(path, write, _)| (path, write))
            .collect::<Vec<_>>();
        // Blind pass 1 at `ec045588` (A06): `chmod` and `fchmodat` name a
        // path and were listed; `fchmod` names a descriptor and was not, so
        // changing the locked corpus's mode or owner through an open
        // descriptor produced no write candidate at all. Every successful
        // open's path is already remembered for its fd, so the descriptor
        // resolves to the file it names.
        if let Some(pid) = pid
            && let Some(path) = fd_write_target(&line, pid, &dir_fds)
        {
            accesses.push((path, true));
        }
        if let Some(pid) = pid {
            // Blind pass 1 at e09ae5e (A07): the cwd and dirfd maps are keyed
            // per pid and were never inherited, so a parent that chdir'd into
            // the locked corpus and then forked left the CHILD unanchored —
            // its relative write carried no forbidden text and resolved
            // against nothing. A child begins life with its parent's working
            // directory and its open descriptors.
            if let Some(child) = clone_child_pid(&line) {
                if let Some(cwd) = cwds.get(&pid).cloned() {
                    cwds.insert(child, cwd);
                }
                let inherited = dir_fds
                    .range((pid, i64::MIN)..=(pid, i64::MAX))
                    .map(|((_, fd), dir)| (*fd, dir.clone()))
                    .collect::<Vec<_>>();
                for (fd, dir) in inherited {
                    dir_fds.insert((child, fd), dir);
                }
            }
            record_chdir(&line, pid, &dir_fds, &mut cwds);
            record_duplicated_fd(&line, pid, returned_fd, &mut dir_fds);
            // Blind pass 1 at 5fb1b57 (A10): only `O_DIRECTORY` opens were
            // remembered, so a descriptor opened without it and later used as
            // a dirfd resolved against nothing. Every successful open's path
            // is remembered for its fd; one that is not a directory simply
            // never appears as a dirfd.
            if (line.contains("openat(") || line.contains("openat2(") || line.contains(" open("))
                && let Some((path, _)) = accesses.first()
            {
                if line.contains("<unfinished ...>") {
                    unfinished.insert(pid, (path.clone(), true));
                } else if let Some(fd) = returned_fd {
                    if fd >= 0 {
                        dir_fds.insert((pid, fd), path.clone());
                    } else {
                        dir_fds.remove(&(pid, fd));
                    }
                }
            }
        }
        for (path, write) in accesses {
            // Blind pass 1 at `6b36bbb9` (A02): candidates were collected only
            // when the path LOOKED like the corpus, so F-64's inode comparison
            // — added for exactly the case where it does not — never saw a
            // hard link under an unrelated name. Every write is a candidate;
            // the lexical rules still decide the lexical cases, and identity
            // decides the rest.
            if write {
                scan.locked_write_candidates.push(path.clone());
            }
            scan.open_paths.insert(path);
        }
    }
    scan.raw_blake3 = hasher.finalize().to_hex().to_string();
    Ok(scan)
}

/// Stream a stored trace, `.zst` accepted, without holding it whole.
fn open_evidence_reader(path: &Utf8Path) -> Result<Box<dyn std::io::BufRead>> {
    let file = fs::File::open(path).with_context(|| format!("open {path}"))?;
    if path.extension() == Some("zst") {
        let decoder = zstd::Decoder::new(file)
            .with_context(|| format!("{path}: evidence is not valid zstd"))?;
        // The decoder buffers its input; the outer BufReader is for read_until.
        Ok(Box::new(std::io::BufReader::new(decoder)))
    } else {
        Ok(Box::new(std::io::BufReader::new(file)))
    }
}

/// F-44 ruling (Brian, 2026-09-05). A stage may READ the locked corpus -- the
/// SLO graduation test measures the profile on it, which is why it is held
/// out -- but no stage may modify the repository's copy of it: the "never
/// hash-update" half of the lock, enforced on every trace. Copies under build
/// roots and scratch directories are outside the repository. The carved fuzz
/// binaries keep the full rule: their traces may not carry the fragment at
/// all (`ScopeTraceScan::forbidden`), because they are the one process family
/// whose evidence is about the corpus.
///
/// Relative paths are judged root-relative, conservatively: a stage's cwd is
/// not receipted, so a relative write that names the locked corpus refuses.
/// Every (device, inode) under the locked corpus. Blind pass 1 at `aeed70f9`
/// (A07): the write check resolved symlinks and pathname ancestry, and a hard
/// link is neither — it is a second name for the same inode, so writing
/// through one outside the corpus mutates corpus bytes under an innocent path.
/// Identity is read from directory metadata; no corpus file is opened.
fn locked_corpus_identities(root: &Utf8Path) -> Result<BTreeSet<(u64, u64)>> {
    use std::os::unix::fs::MetadataExt as _;
    let corpora = root.join("conformance/corpora");
    let mut identities = BTreeSet::new();
    if !corpora.is_dir() {
        return Ok(identities);
    }
    let mut stack = vec![corpora];
    while let Some(dir) = stack.pop() {
        // Review of `bba803d4`: skipping an unreadable directory returned a
        // partial set, and a hard link to a file inside it would then pass. A
        // corpus this cannot enumerate is a corpus it cannot protect.
        let entries = fs::read_dir(dir.as_std_path())
            .with_context(|| format!("enumerate the locked corpus at {dir}"))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .with_context(|| format!("stat {} in the locked corpus", path.display()))?;
            if metadata.is_dir() {
                if let Ok(dir) = Utf8PathBuf::from_path_buf(path) {
                    stack.push(dir);
                }
            } else if metadata.is_file() {
                identities.insert((metadata.dev(), metadata.ino()));
            }
        }
    }
    Ok(identities)
}

fn locked_corpus_write(
    root: &Utf8Path,
    candidates: &[String],
    label: &str,
) -> Result<Option<String>> {
    let canonical_root = fs::canonicalize(root.as_std_path())
        .with_context(|| format!("{label}: canonicalize repository root"))?;
    let identities = locked_corpus_identities(root)?;
    for lexical in candidates {
        let path = Path::new(lexical);
        // Blind pass 1 at 78c8f9b (A12): a relative write from a subdirectory,
        // `../conformance/corpora/heldout/x`, joined onto the root climbed out
        // of it and passed. The cwd is not receipted, so a relative write that
        // names the locked corpus at all is refused, before any resolution.
        let lower = lexical.to_ascii_lowercase();
        if !path.is_absolute()
            && (lower.contains("conformance/corpora/") || lower.ends_with("conformance/corpora"))
        {
            return Ok(Some(lexical.clone()));
        }
        let local = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.as_std_path().join(path)
        };
        // A path that no longer resolves (deleted after the trace) is judged
        // lexically: the conservative direction, since a lexical
        // conformance/corpora prefix still refuses.
        let canonical = canonicalize_trace_path(&local).unwrap_or(local);
        if let Ok(relative) = canonical.strip_prefix(&canonical_root)
            && relative
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with("conformance/corpora")
        {
            return Ok(Some(lexical.clone()));
        }
        // A07: a hard link is a second name for the same inode, so no amount
        // of path reasoning sees it. Identity does.
        if let Ok(metadata) = fs::symlink_metadata(&canonical) {
            use std::os::unix::fs::MetadataExt as _;
            if identities.contains(&(metadata.dev(), metadata.ino())) {
                return Ok(Some(format!(
                    "{lexical} (hard link into the locked corpus)"
                )));
            }
        }
    }
    Ok(None)
}

/// Whether a resolved repository path inside the locked corpus refuses the
/// whole trace (the carved fuzz binaries) or is a read the ruling permits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LockedCorpus {
    AnyTouch,
    WritesOnly,
}

fn scan_scope_trace_file(path: &Utf8Path, label: &str) -> Result<ScopeTraceScan> {
    let reader = open_evidence_reader(path)
        .with_context(|| format!("{label}: captured scope trace is required"))?;
    scan_scope_trace(reader).with_context(|| format!("{label}: {path}: scope trace unreadable"))
}

/// Scan a scope's stored remainder and every carved part, binding each part's
/// bytes to the digest the row declares. Returns the remainder's scan and the
/// union of open paths; a locked-corpus fragment in ANY file is the row's.
/// Whether any of `paths` names a file the locked corpus also names. A hard
/// link is a second name for one inode, so no amount of path reasoning sees it.
fn corpus_identity_touched(root: &Utf8Path, paths: &BTreeSet<String>) -> Result<bool> {
    use std::os::unix::fs::MetadataExt as _;
    let identities = locked_corpus_identities(root)?;
    if identities.is_empty() {
        return Ok(false);
    }
    for path in paths {
        let local = if Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            root.as_std_path().join(path)
        };
        let resolved = canonicalize_trace_path(&local).unwrap_or(local);
        if let Ok(metadata) = fs::symlink_metadata(&resolved)
            && identities.contains(&(metadata.dev(), metadata.ino()))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn scan_scope_trace_union(
    root: &Utf8Path,
    row: &CorpusScopeTrace,
) -> Result<(ScopeTraceScan, BTreeSet<String>, Option<&'static str>)> {
    let label = format!("{} corpus scope", row.scope);
    let remainder_path = safe_repo_path(root, &row.trace, "corpus scope trace")?;
    let remainder = scan_scope_trace_file(&remainder_path, &label)?;
    let mut open = remainder.open_paths.clone();
    // The remainder is a stage: reads permitted, writes judged by the caller
    // (F-44). Parts are the fuzz binaries: any fragment refuses.
    let mut forbidden = None;
    for part in &row.parts {
        let part_path = safe_repo_path(root, &part.trace, "corpus scope trace part")?;
        let scan = scan_scope_trace_file(&part_path, &label)?;
        require_eq(
            &format!(
                "{} corpus scope part {} trace_blake3",
                row.scope, part.trace
            ),
            &part.trace_blake3,
            &scan.raw_blake3,
        )?;
        forbidden = forbidden.or(scan.forbidden);
        // Blind pass 1 at `0a0b4b4b` (A07): the any-touch rule fired on the
        // lexical fragments `heldout` and `conformance/corpora`, so a hard
        // link under any other name was invisible — and F-64's inode
        // comparison guarded writes only, while the carved fuzz binaries are
        // the one family for which a READ is the leak. Identity applies to
        // every path a fuzz part opened.
        if forbidden.is_none() && corpus_identity_touched(root, &scan.open_paths)? {
            forbidden = Some("locked corpus reached by inode identity");
        }
        open.extend(scan.open_paths);
    }
    Ok((remainder, open, forbidden))
}

/// The fuzz scope's parts are exactly the campaign's per-target traces: every
/// binary the stage tracer saw is judged, none twice, and no other scope carves.
fn verify_scope_trace_parts(
    row: &CorpusScopeTrace,
    targets: &[CorpusAccessAuditTarget],
) -> Result<()> {
    let declared = row
        .parts
        .iter()
        .map(|part| (part.trace.as_str(), part.trace_blake3.as_str()))
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        declared.len() == row.parts.len(),
        "{} corpus scope declares a trace part twice",
        row.scope
    );
    if row.scope == "fuzz" {
        let expected = targets
            .iter()
            .map(|target| (target.trace.as_str(), target.trace_blake3.as_str()))
            .collect::<BTreeSet<_>>();
        anyhow::ensure!(
            declared == expected,
            "fuzz corpus scope parts differ from the campaign's per-target traces: \
             declared={declared:?}, expected={expected:?}"
        );
    } else {
        anyhow::ensure!(
            row.parts.is_empty(),
            "{} corpus scope declares trace parts, but only fuzz carves its binaries out",
            row.scope
        );
    }
    Ok(())
}

fn normalize_scope_trace_path(path: &str) -> String {
    for marker in ["/fuzz/", "/conformance/", "/target/"] {
        if let Some(index) = path.find(marker) {
            return path[index + 1..].to_owned();
        }
    }
    path.to_owned()
}

/// Resolve corpus paths from raw strace metadata. Lexical path filtering alone
/// is insufficient: a benign `fuzz/corpus/<target>` symlink can resolve into
/// locked `conformance/corpora/heldout` data. The campaign records only a
/// digest of `(lexical, canonical repository-relative)` pairs, so no path
/// receipt can disclose corpus names; verification recomputes it and fails
/// closed on missing, escaping, or forbidden resolutions.
/// Whether a trace is REQUIRED to have opened a fuzz corpus path.
///
/// The forbidden-path refusal below is unconditional for every trace. What
/// varies is whether an EMPTY resolved set is a defect: for a fuzz campaign it
/// is (a run that never read its corpus fuzzed nothing), while `just ci` or
/// `haq-canaries` legitimately never open one. Before this distinction the
/// corpus-scope subsystem could not be satisfied by six of its eight scopes,
/// which is one reason it had never produced a row (ruling 2026-09-04).
#[derive(Clone, Copy, PartialEq, Eq)]
enum CorpusAccess {
    Required,
    IfPresent,
}

/// Compare a trace's resolved corpus paths against the recorded digest.
fn verify_trace_corpus_resolution(
    root: &Utf8Path,
    trace_root: &str,
    bytes: &[u8],
    expected_digest: &str,
    label: &str,
    access: CorpusAccess,
) -> Result<()> {
    require_hex_digest(&format!("{label} resolved_paths_blake3"), expected_digest)?;
    let actual = resolved_corpus_paths_digest(root, trace_root, bytes, label, access)?;
    require_eq(
        &format!("{label} resolved_paths_blake3"),
        &actual,
        expected_digest,
    )
}

fn resolved_corpus_paths_digest(
    root: &Utf8Path,
    trace_root: &str,
    bytes: &[u8],
    label: &str,
    access: CorpusAccess,
) -> Result<String> {
    resolved_corpus_paths_digest_from(
        root,
        trace_root,
        &scope_trace_open_paths(bytes),
        label,
        access,
        LockedCorpus::AnyTouch,
    )
}

/// Whether a relative traced path is left out of corpus resolution.
///
/// Relative paths are interpreted against the process cwd or a directory fd,
/// which the trace does not receipt; resolving one as root-relative can hide a
/// locked corpus behind a chdir or alias. A fuzz binary is invoked with an
/// absolute corpus path, so a relative one is the campaign's defect and refuses
/// (`AnyTouch`). A stage's helpers name seeds relative to their cwd (git's
/// index refresh, the campaign's seed manifest) and reviewer tooling opens
/// `../nvidia0` under /dev: unresolvable, and able to add no corpus entry, so
/// under `WritesOnly` every relative path is skipped rather than refused as an
/// escape (F-44). Writes were already judged root-relative by
/// `locked_corpus_write`.
fn relative_trace_path_is_skipped(
    lexical_path: &Path,
    lexical_lower: &str,
    locked: LockedCorpus,
    label: &str,
) -> Result<bool> {
    if lexical_path.is_absolute() {
        return Ok(false);
    }
    let names_corpus =
        lexical_lower.contains("/fuzz/corpus/") || lexical_lower.starts_with("fuzz/corpus/");
    match locked {
        LockedCorpus::AnyTouch if names_corpus => anyhow::bail!(
            "{label}: relative fuzz corpus path lacks authenticated cwd/dirfd binding"
        ),
        LockedCorpus::AnyTouch => Ok(false),
        LockedCorpus::WritesOnly => Ok(true),
    }
}

fn resolved_corpus_paths_digest_from(
    root: &Utf8Path,
    trace_root: &str,
    open_paths: &BTreeSet<String>,
    label: &str,
    access: CorpusAccess,
    locked: LockedCorpus,
) -> Result<String> {
    let canonical_root = fs::canonicalize(root.as_std_path())
        .with_context(|| format!("{label}: canonicalize repository root"))?;
    let trace_root = Path::new(trace_root);
    anyhow::ensure!(
        trace_root.is_absolute(),
        "{label}: trace_root must be absolute"
    );
    let mut resolved = BTreeSet::new();
    for lexical in open_paths {
        let lexical_path = Path::new(&lexical);
        let lexical_lower = lexical_path.to_string_lossy().to_ascii_lowercase();
        if relative_trace_path_is_skipped(lexical_path, &lexical_lower, locked, label)? {
            continue;
        }
        let under_trace_root =
            !lexical_path.is_absolute() || lexical_path.strip_prefix(trace_root).is_ok();
        if !under_trace_root {
            continue;
        }
        let local = trace_path_to_repo(root, trace_root, lexical)
            .with_context(|| format!("{label}: cannot map traced repository path"))?;
        let canonical = canonicalize_trace_path(local.as_std_path())
            .with_context(|| format!("{label}: traced repository path cannot be resolved"))?;
        let relative = canonical
            .strip_prefix(&canonical_root)
            .with_context(|| format!("{label}: traced repository path escapes repository root"))?;
        let relative = relative.to_str().context("non-UTF-8 traced corpus path")?;
        let lower = relative.to_ascii_lowercase();
        if locked == LockedCorpus::AnyTouch {
            for forbidden in FORBIDDEN_TRACE_FRAGMENTS {
                anyhow::ensure!(
                    !lower.contains(forbidden),
                    "{label}: traced repository path resolves into forbidden data"
                );
            }
        }
        if lexical_lower.contains("/fuzz/corpus/")
            || lexical_lower.starts_with("fuzz/corpus/")
            || lower.contains("/fuzz/corpus/")
            || lower.starts_with("fuzz/corpus/")
        {
            resolved.insert(format!("{lexical}\0{relative}"));
        }
    }
    if access == CorpusAccess::Required {
        anyhow::ensure!(
            !resolved.is_empty(),
            "{label}: trace contains no resolvable fuzz corpus path"
        );
    }
    let resolved = resolved.into_iter().collect::<Vec<_>>();
    let resolved_bytes = format!("{}\n", resolved.join("\n"));
    Ok(blake3::hash(resolved_bytes.as_bytes()).to_hex().to_string())
}

/// Canonicalize the existing prefix of a traced path, then append historical
/// leaf components that libFuzzer may have deleted after opening them. This
/// preserves symlink and root-boundary checks without requiring every path in
/// a long-running trace to remain present at receipt time.
fn canonicalize_trace_path(path: &Path) -> Result<std::path::PathBuf> {
    let mut missing = Vec::new();
    let mut cursor = path;
    loop {
        match fs::symlink_metadata(cursor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    cursor
                        .file_name()
                        .context("traced path has no recoverable leaf")?
                        .to_owned(),
                );
                let leaf = missing.last().expect("just pushed missing leaf");
                anyhow::ensure!(
                    leaf != std::ffi::OsStr::new(".") && leaf != std::ffi::OsStr::new(".."),
                    "traced path contains a parent-directory suffix"
                );
                cursor = cursor
                    .parent()
                    .context("traced path has no existing ancestor")?;
                anyhow::ensure!(
                    cursor.file_name() != Some(std::ffi::OsStr::new("corpus")),
                    "traced path is missing a corpus entry; symlink history is unavailable"
                );
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut canonical = fs::canonicalize(cursor)?;
    for component in missing.into_iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
}

fn trace_path_to_repo(root: &Utf8Path, trace_root: &Path, lexical: &str) -> Result<Utf8PathBuf> {
    let path = Path::new(lexical);
    if path.is_absolute() {
        if let Ok(relative) = path.strip_prefix(trace_root) {
            return Ok(
                root.join(Utf8Path::from_path(relative).context("non-UTF-8 traced relative path")?)
            );
        }
        anyhow::bail!("absolute traced path is outside recorded trace root")
    }
    Ok(root.join(Utf8Path::from_path(path).context("non-UTF-8 traced relative path")?))
}

#[allow(
    clippy::too_many_lines,
    reason = "corpus tracer gate keeps process binding and manifest checks together"
)]
fn verify_corpus_audit_campaign_binding(root: &Utf8Path, audit: &CorpusAccessAudit) -> Result<()> {
    let fuzz_path = root.join("conformance/haqp/evidence/fuzz.json");
    let fuzz: Vec<FuzzEvidence> =
        serde_json::from_slice(&fs::read(&fuzz_path).with_context(|| {
            format!("{fuzz_path}: fuzz evidence is required for trace binding")
        })?)
        .with_context(|| format!("parse {fuzz_path}"))?;
    let fuzz_by_target = fuzz
        .iter()
        .map(|row| (row.target.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    for row in &audit.targets {
        let fuzz_row = fuzz_by_target.get(row.target.as_str()).with_context(|| {
            format!(
                "{}: corpus audit target is absent from fuzz evidence",
                row.target
            )
        })?;
        anyhow::ensure!(
            row.trace_pid > 0,
            "{}: corpus audit has no traced process PID",
            row.target
        );
        anyhow::ensure!(
            row.trace_exit_code == 0,
            "{}: tracer exited {}",
            row.target,
            row.trace_exit_code
        );
        anyhow::ensure!(
            row.trace_complete,
            "{}: raw tracer output is incomplete",
            row.target
        );
        anyhow::ensure!(
            row.command.contains("strace") && row.command.contains("-f"),
            "{}: corpus audit command is not recursive strace",
            row.target
        );
        anyhow::ensure!(
            !row.tracer_binary.trim().is_empty()
                && !row.tracer_version.trim().is_empty()
                && !row.tracer_version_blake3.trim().is_empty(),
            "{} corpus audit lacks current tracer identity; regenerate corpus-access evidence",
            row.target
        );
        require_eq("corpus tracer binary", &row.tracer_binary, "strace")?;
        require_eq(
            "corpus tracer version",
            &row.tracer_version,
            "conformance/haqp/evidence/access/strace.version",
        )?;
        require_hex_digest(
            &format!("{} tracer_version_blake3", row.target),
            &row.tracer_version_blake3,
        )?;
        let tracer_version_path =
            safe_repo_path(root, &row.tracer_version, "corpus tracer version")?;
        let tracer_version_bytes = fs::read(&tracer_version_path)
            .with_context(|| format!("{tracer_version_path}: tracer version receipt required"))?;
        require_eq(
            &format!("{} tracer_version_blake3", row.target),
            &row.tracer_version_blake3,
            blake3::hash(&tracer_version_bytes).to_hex().as_ref(),
        )?;
        require_hex_digest(&format!("{} trace_blake3", row.target), &row.trace_blake3)?;
        let trace_path = safe_repo_path(root, &row.trace, "corpus trace")?;
        let trace_bytes = read_evidence_bytes(&trace_path)
            .with_context(|| format!("{trace_path}: raw access trace is required"))?;
        require_eq(
            "corpus access trace_blake3",
            &row.trace_blake3,
            blake3::hash(&trace_bytes).to_hex().as_ref(),
        )?;
        let process_binding = blake3::hash(
            format!(
                "{}\0{}\0{}\0{}",
                row.command, row.trace_pid, row.trace_exit_code, row.trace_blake3
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        require_eq(
            "corpus access process_binding",
            &row.process_binding,
            &process_binding,
        )?;
        anyhow::ensure!(
            row.seed == fuzz_row.seed,
            "{}: corpus audit seed differs from fuzz evidence",
            row.target
        );
        require_eq(
            "corpus audit sanitizer",
            &row.sanitizer,
            &fuzz_row.sanitizer,
        )?;
        anyhow::ensure!(
            row.exit_code == fuzz_row.exit_code,
            "{}: corpus audit exit code differs from fuzz evidence",
            row.target
        );
        require_eq(
            "corpus audit log_blake3",
            &row.log_blake3,
            &fuzz_row.log_blake3,
        )?;
        anyhow::ensure!(
            !row.command.trim().is_empty(),
            "{}: corpus audit has no traced command",
            row.target
        );
        let expected_manifest = format!("conformance/haqp/evidence/access/{}.paths", row.target);
        require_eq(
            "corpus access manifest path",
            &row.manifest,
            &expected_manifest,
        )?;
        let manifest = root.join(&expected_manifest);
        let metadata = fs::symlink_metadata(&manifest)
            .with_context(|| format!("{manifest}: corpus access manifest is required"))?;
        anyhow::ensure!(
            metadata.file_type().is_file(),
            "{manifest}: corpus access manifest must be a regular file"
        );
        let manifest_bytes = fs::read(&manifest).with_context(|| format!("read {manifest}"))?;
        let manifest_digest = blake3::hash(&manifest_bytes).to_hex();
        require_eq(
            "corpus access manifest_blake3",
            &row.manifest_blake3,
            manifest_digest.as_ref(),
        )?;
        let manifest_text = String::from_utf8(manifest_bytes)
            .with_context(|| format!("{manifest}: corpus access manifest is not UTF-8"))?;
        anyhow::ensure!(
            !manifest_text.trim().is_empty(),
            "{manifest}: corpus access manifest is empty"
        );
        let manifest_lower = manifest_text.to_ascii_lowercase();
        for forbidden in ["heldout", "conformance/corpora"] {
            anyhow::ensure!(
                !manifest_lower.contains(forbidden),
                "{manifest}: corpus access audit observed forbidden path fragment {forbidden:?}"
            );
        }
        let trace_text = String::from_utf8_lossy(&trace_bytes);
        let trace_lower = trace_text.to_ascii_lowercase();
        let pid_prefix = format!("{} ", row.trace_pid);
        anyhow::ensure!(
            trace_text.lines().any(|line| line.starts_with(&pid_prefix)),
            "{}: raw trace has no record emitted by traced process PID {}",
            row.target,
            row.trace_pid
        );
        anyhow::ensure!(
            trace_text
                .lines()
                .any(|line| line == format!("{} +++ exited with 0 +++", row.trace_pid)),
            "{}: raw trace has no successful exit record for tracer PID {}",
            row.target,
            row.trace_pid
        );
        let target_corpus_marker = format!("/fuzz/corpus/{}", row.target);
        anyhow::ensure!(
            trace_lower.contains(&target_corpus_marker),
            "{}: raw trace is not bound to target corpus {}",
            row.target,
            target_corpus_marker
        );
        let target_binary_marker = "/fuzz/target/";
        let target_binary_name = format!("/release/{}", row.target);
        anyhow::ensure!(
            trace_lower.contains(target_binary_marker) && trace_lower.contains(&target_binary_name),
            "{}: raw trace does not show target binary execution",
            row.target
        );
        // The corpus and binary opens must come from the fuzz child, not from
        // the strace launcher or a build helper. Bind that child to the
        // launcher's SIGCHLD receipt; a hand-written trace containing only
        // expected path strings cannot satisfy this relation.
        let corpus_child_pids = trace_text
            .lines()
            .filter(|line| {
                line.to_ascii_lowercase().contains(&target_corpus_marker)
                    && line.contains("openat(")
            })
            .filter_map(|line| line.split_whitespace().next())
            .filter_map(|pid| pid.parse::<u32>().ok())
            .collect::<BTreeSet<_>>();
        anyhow::ensure!(
            corpus_child_pids.len() == 1,
            "{}: raw trace must identify exactly one fuzz child reading its corpus, got {:?}",
            row.target,
            corpus_child_pids
        );
        let child_pid = *corpus_child_pids.iter().next().expect("one child checked");
        let child_exit_receipt = format!("si_pid={child_pid},");
        anyhow::ensure!(
            trace_text.lines().any(|line| line.starts_with(&pid_prefix)
                && line.contains("SIGCHLD")
                && line.contains(&child_exit_receipt)),
            "{}: raw trace lacks launcher SIGCHLD receipt for corpus-reading child {}",
            row.target,
            child_pid
        );
        anyhow::ensure!(
            trace_text.lines().any(|line| {
                line.starts_with(&format!("{child_pid} ")) && line.contains(&target_binary_name)
            }),
            "{}: target binary execution is not attributed to corpus-reading child {}",
            row.target,
            child_pid
        );
        for forbidden in ["heldout", "conformance/corpora"] {
            anyhow::ensure!(
                !trace_lower.contains(forbidden),
                "{trace_path}: raw access trace observed forbidden path fragment {forbidden:?}"
            );
        }
        verify_trace_corpus_resolution(
            root,
            &row.trace_root,
            &trace_bytes,
            &row.resolved_paths_blake3,
            &format!("{} corpus audit", row.target),
            CorpusAccess::Required,
        )?;
    }
    Ok(())
}

/// Check the bounded-campaign clock artifact. One clean breach is retained as
/// residual risk; two clean breaches block ratification per ADR-0020 §7.
/// Days from 1970-01-01 for a civil date (Howard Hinnant's algorithm). Used to
/// turn a provider's ISO timestamp into an epoch without taking a date
/// dependency for one calculation.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_shift = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_shift + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The epoch second in a codex session id, which opens
/// `YYYY-MM-DDTHH-MM-SS-<uuid>`. The provider's CLI writes it, not the
/// qualifier.
fn codex_session_epoch(session_id: &str) -> Option<i64> {
    let digits: Vec<i64> = session_id
        .split(['-', 'T'])
        .take(6)
        .map(|part| part.parse::<i64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let [year, month, day, hour, minute, second] = digits[..] else {
        return None;
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Every clock reading in the committed review receipts that the qualifier did
/// not author: the provider's `created` for a MiMo envelope, and the provider
/// CLI's timestamp in a codex session id.
fn provider_clock_readings(root: &Utf8Path, packet: &Packet) -> Result<Vec<(String, i64)>> {
    let mut readings = Vec::new();
    for review in &packet.reviews {
        let Some(evidence) = review.evidence.as_deref() else {
            continue;
        };
        let path = safe_repo_path(root, evidence, "campaign clock review")?;
        let record: ReviewRecord =
            serde_json::from_slice(&fs::read(&path).with_context(|| format!("read {path}"))?)
                .with_context(|| format!("parse {path}"))?;
        let receipt = &record.provider_receipt;
        if receipt.backend == "codex" {
            let epoch = codex_session_epoch(&receipt.session_id).with_context(|| {
                format!(
                    "{}: codex session id {:?} carries no provider timestamp",
                    review.reviewer, receipt.session_id
                )
            })?;
            readings.push((format!("{} codex session", review.reviewer), epoch));
        } else {
            anyhow::ensure!(
                receipt.created > 0,
                "{}: receipt carries no provider clock",
                review.reviewer
            );
            readings.push((
                format!("{} provider created", review.reviewer),
                receipt.created,
            ));
        }
    }
    Ok(readings)
}

/// The campaign window must contain every clock reading the qualifier did not
/// author. Blind pass 1 at `6b36bbb9` (A07): the only temporal check was
/// `finished - started == elapsed`, which a regenerated window satisfies
/// exactly, so an understated duration passed. The reviews happen inside the
/// campaign; an understated window pushes their provider timestamps out.
fn verify_campaign_window_holds_provider_clocks(
    root: &Utf8Path,
    packet: &Packet,
    clock: &CampaignClock,
) -> Result<()> {
    let readings = provider_clock_readings(root, packet)?;
    if readings.is_empty() {
        return Ok(());
    }
    let started = clock
        .runs
        .iter()
        .map(|run| i64::try_from(run.started_epoch).unwrap_or(i64::MAX))
        .min()
        .context("a campaign with runs has a start")?;
    let finished = clock
        .runs
        .iter()
        .map(|run| i64::try_from(run.finished_epoch).unwrap_or(i64::MAX))
        .max()
        .context("a campaign with runs has an end")?;
    for (who, epoch) in &readings {
        anyhow::ensure!(
            *epoch >= started && *epoch <= finished,
            "{who} reads {epoch}, outside the campaign window {started}..={finished} the clock \
             claims; a provider's clock is not the qualifier's to move"
        );
    }
    Ok(())
}

fn verify_campaign_clock(
    root: &Utf8Path,
    packet: &Packet,
    expected: Option<&Provenance>,
) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/campaign.json");
    let bytes = fs::read(&path)
        .with_context(|| format!("{path}: committed campaign clock evidence is required"))?;
    let clock: CampaignClock =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))?;
    require_eq(
        "campaign clock schema_version",
        &clock.schema_version,
        "haqp-campaign-clock-v1",
    )?;
    anyhow::ensure!(
        !clock.reference_machine.trim().is_empty(),
        "campaign clock has no reference machine"
    );
    require_unique(clock.runs.iter().map(|row| row.id.as_str()), "campaign run")?;
    anyhow::ensure!(!clock.runs.is_empty(), "campaign clock records no runs");
    for run in &clock.runs {
        anyhow::ensure!(!run.id.trim().is_empty(), "campaign run has empty id");
        require_git_object_id("campaign run commit", &run.commit)?;
        require_git_object_id("campaign run tree", &run.tree)?;
        anyhow::ensure!(
            !run.command.trim().is_empty(),
            "campaign run has no command"
        );
        if let Some(provenance) = expected {
            require_eq("campaign run commit", &run.commit, &provenance.fixed_commit)?;
            require_eq("campaign run tree", &run.tree, &provenance.fixed_tree)?;
        }
        anyhow::ensure!(
            run.elapsed_s > 0,
            "campaign run {} has zero elapsed seconds",
            run.id
        );
        anyhow::ensure!(
            run.finished_epoch >= run.started_epoch
                && run.finished_epoch - run.started_epoch == run.elapsed_s,
            "campaign run {} elapsed_s is not derived from recorded start/end epochs",
            run.id
        );
        require_eq("campaign run result", &run.result, "pass")?;
        anyhow::ensure!(run.clean, "campaign run {} was not clean", run.id);
        require_eq(
            "campaign wrapper",
            &run.wrapper,
            "scripts/haqp_campaign_clock.sh",
        )?;
        require_hex_digest("campaign wrapper_sha256", &run.wrapper_sha256)?;
        let wrapper_bytes = fs::read(root.join(&run.wrapper))
            .with_context(|| format!("{}: campaign wrapper is required", run.wrapper))?;
        let mut wrapper_hasher = Sha256::new();
        wrapper_hasher.update(wrapper_bytes);
        let wrapper_digest = format!("{:x}", wrapper_hasher.finalize());
        require_eq(
            "campaign wrapper_sha256",
            &run.wrapper_sha256,
            &wrapper_digest,
        )?;
        anyhow::ensure!(
            !run.receipt.trim().is_empty() && !run.receipt_blake3.trim().is_empty(),
            "campaign run {} lacks wrapper receipt; regenerate campaign clock evidence",
            run.id
        );
        let receipt_path = safe_repo_path(root, &run.receipt, "campaign receipt")?;
        let receipt_bytes = fs::read(&receipt_path)
            .with_context(|| format!("{receipt_path}: campaign receipt is required"))?;
        require_hex_digest("campaign receipt_blake3", &run.receipt_blake3)?;
        require_eq(
            "campaign receipt_blake3",
            &run.receipt_blake3,
            blake3::hash(&receipt_bytes).to_hex().as_ref(),
        )?;
        let receipt = String::from_utf8(receipt_bytes)
            .with_context(|| format!("{receipt_path}: campaign receipt is not UTF-8"))?;
        let expected_receipt = format!(
            "run_id={}\ncommit={}\ntree={}\ncommand={}\nstarted_epoch={}\nfinished_epoch={}\nelapsed_s={}\nclean={}\nresult={}\n",
            run.id,
            run.commit,
            run.tree,
            run.command,
            run.started_epoch,
            run.finished_epoch,
            run.elapsed_s,
            run.clean,
            run.result
        );
        require_eq("campaign receipt contents", &receipt, &expected_receipt)?;
    }
    // Blind pass 1 at `6b36bbb9` (A07): every number here is the qualifier's
    // own, and `finished - started == elapsed` is self-consistency — a
    // regenerated window satisfies it exactly. The provider receipts carry
    // clocks the qualifier does not author, and the reviews happen inside the
    // campaign, so those readings must fall inside the window the campaign
    // claims. An understated window pushes them out.
    verify_campaign_window_holds_provider_clocks(root, packet, &clock)?;
    verify_campaign_clock_budget(&clock)
}

fn verify_campaign_clock_budget(clock: &CampaignClock) -> Result<()> {
    let breaches = clock
        .runs
        .iter()
        .filter(|run| run.elapsed_s > 8 * 60 * 60)
        .count();
    anyhow::ensure!(
        breaches < 2,
        "campaign exceeded the eight-hour ceiling on {breaches} clean runs; ratification is blocked"
    );
    Ok(())
}

/// Every limitation carried into qualification needs an owner, trigger,
/// evidence coordinate, mitigation, and acceptance authority. Bare prose in a
/// residual-risk table is not auditable (ADR-0020 §7).
fn verify_residual_risks(root: &Utf8Path, packet: &Packet) -> Result<()> {
    anyhow::ensure!(
        !packet.residual_risks.is_empty(),
        "qualified packet must record residual-risk coordinates"
    );
    let requirement_ids = packet
        .requirements
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    require_unique(
        packet.residual_risks.iter().map(|row| row.id.as_str()),
        "residual risk",
    )?;
    for risk in &packet.residual_risks {
        for (label, value) in [
            ("owner", &risk.owner),
            ("severity", &risk.severity),
            ("trigger", &risk.trigger),
            ("requirement", &risk.requirement),
            ("evidence", &risk.evidence),
            ("planned_phase", &risk.planned_phase),
            ("mitigation", &risk.mitigation),
            ("authority", &risk.authority),
            ("state", &risk.state),
        ] {
            anyhow::ensure!(
                !value.trim().is_empty(),
                "risk {} has empty {label}",
                risk.id
            );
        }
        anyhow::ensure!(
            requirement_ids.contains(risk.requirement.as_str()),
            "risk {} names unknown requirement {}",
            risk.id,
            risk.requirement
        );
        if let Some((file, line)) = risk.evidence.rsplit_once(':') {
            let line: usize = line
                .parse()
                .with_context(|| format!("risk {} evidence line is not numeric", risk.id))?;
            anyhow::ensure!(
                line > 0
                    && !file.is_empty()
                    && !file.starts_with('/')
                    && !file.split('/').any(|part| part == ".."),
                "risk {} evidence coordinate is unsafe",
                risk.id
            );
            anyhow::ensure!(
                !is_locked_acceptance_path(file),
                "risk {} evidence coordinate names locked acceptance corpus",
                risk.id
            );
            let path = safe_repo_path(root, file, "residual-risk evidence")?;
            let source = fs::read_to_string(&path)
                .with_context(|| format!("risk {} evidence source is missing", risk.id))?;
            anyhow::ensure!(
                source.lines().nth(line - 1).is_some(),
                "risk {} evidence line {} is outside {}",
                risk.id,
                line,
                file
            );
        } else {
            anyhow::bail!("risk {} evidence must be an exact file:coordinate", risk.id);
        }
        anyhow::ensure!(
            matches!(risk.state.as_str(), "open" | "mitigated" | "accepted"),
            "risk {} has unknown state {:?}",
            risk.id,
            risk.state
        );
    }
    Ok(())
}

/// ADR-0020 §1: the qualification lane runs from ONE fixed clean commit and
/// tree. The packet is committed one commit after that fixed evidence commit;
/// requiring the packet's metadata commit to equal HEAD would be a
/// self-referential hash claim and cannot be satisfied by Git.
fn verify_provenance(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let Some(provenance) = &packet.provenance else {
        anyhow::bail!("packet has no provenance block; qualification is unbound to any tree");
    };
    let git = |args: &[&str]| -> Result<String> {
        let out = Command::new("git").current_dir(root).args(args).output()?;
        anyhow::ensure!(out.status.success(), "git {args:?} failed");
        Ok(String::from_utf8(out.stdout)?.trim().to_owned())
    };
    require_eq(
        "provenance.commit/fixed_commit",
        &provenance.commit,
        &provenance.fixed_commit,
    )?;
    let parent = git(&["rev-parse", "HEAD^"])?;
    let parents = git(&["rev-list", "--parents", "-n1", "HEAD"])?
        .split_whitespace()
        .count()
        .saturating_sub(1);
    anyhow::ensure!(
        parents == 1,
        "qualification metadata commit must have exactly one parent, found {parents}"
    );
    require_eq(
        "provenance.evidence_parent",
        &provenance.evidence_parent,
        &parent,
    )?;
    require_eq(
        "provenance.fixed_commit/evidence_parent",
        &provenance.fixed_commit,
        &provenance.evidence_parent,
    )?;
    // The metadata child may carry only qualification outputs. If source or
    // gate code changes after evidence was captured, the packet can still
    // point at the old tree while claiming the new behavior was tested.
    let changed = git_text(
        root,
        &[
            "diff",
            "--name-only",
            &format!("{}..HEAD", provenance.evidence_parent),
        ],
    )?;
    for path in changed.lines().filter(|path| !path.trim().is_empty()) {
        if path == PHASE0_GATE_FILE {
            verify_gate_unignore_only(root, &provenance.evidence_parent)?;
            continue;
        }
        anyhow::ensure!(
            qualification_metadata_path(path),
            "qualification metadata child changed non-metadata path {path:?}; source and gate code must remain at fixed commit"
        );
    }
    let fixed_tree = git(&[
        "rev-parse",
        &format!("{}^{{tree}}", provenance.fixed_commit),
    ])?;
    require_eq("provenance.fixed_tree", &provenance.fixed_tree, &fixed_tree)?;
    let dirty = git(&["status", "--porcelain"])?;
    anyhow::ensure!(
        dirty.is_empty(),
        "qualification requires a clean tree; {} path(s) are dirty",
        dirty.lines().count()
    );
    let lockfile = fs::read(root.join("Cargo.lock"))?;
    let digest = blake3::hash(&lockfile).to_hex().to_string();
    require_eq(
        "provenance.lockfile_blake3",
        &provenance.lockfile_blake3,
        &digest,
    )?;
    Ok(())
}

/// The one gate whose `#[ignore]` the metadata child may lift (AM-17.6).
const PHASE0_GATE_FILE: &str = "conformance/tests/phase0.rs";
const PHASE0_GATE_IGNORE: &str =
    r#"#[ignore = "Phase 0 M17: HAQP qualification evidence not yet complete"]"#;

/// The metadata child may lift EXACTLY ONE `#[ignore]`, and change nothing else
/// (AM-17.6).
///
/// `phase1_suite_packet_is_complete_and_unratified` is M17.5's stated exit gate,
/// and it asserts that the packet is complete. It therefore cannot be live
/// before the flip that completes the packet — un-ignoring it at the fixed base
/// commits a red tree, and un-ignoring it after the flip moves HEAD and breaks
/// `evidence_parent == HEAD^`. The child is the only place it can go.
///
/// Widening the path allowlist alone would let gate LOGIC change in the child,
/// which is the whole thing `verify_provenance` exists to prevent. So the diff
/// itself is checked: one removed line, no added lines, and the removed line is
/// that attribute. Anything else in this file is refused exactly as before.
fn verify_gate_unignore_only(root: &Utf8Path, evidence_parent: &str) -> Result<()> {
    let diff = git_text(
        root,
        &[
            "diff",
            "--unified=0",
            &format!("{evidence_parent}..HEAD"),
            "--",
            PHASE0_GATE_FILE,
        ],
    )?;
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if let Some(rest) = line.strip_prefix('-') {
            removed.push(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix('+') {
            added.push(rest.trim().to_owned());
        }
    }
    anyhow::ensure!(
        added.is_empty(),
        "the metadata child added {} line(s) to {PHASE0_GATE_FILE}; it may only lift the \
         qualification gate's #[ignore], never add gate code: {added:?}",
        added.len()
    );
    anyhow::ensure!(
        removed.len() == 1,
        "the metadata child removed {} line(s) from {PHASE0_GATE_FILE}; exactly one \
         #[ignore] may be lifted: {removed:?}",
        removed.len()
    );
    require_eq(
        "metadata child's lifted attribute",
        removed[0].as_str(),
        PHASE0_GATE_IGNORE,
    )
}

/// Working-tree paths that are neither qualification metadata nor evidence.
///
/// A blanket `git status --porcelain` check is wrong here and has now been wrong
/// four times: once each in haqp_flip_packet.py, haqp_blind_review.py and
/// haqp_campaign_clock.sh, and here. The lanes WRITE evidence as they run, so by
/// the time the mutant stage starts, `canaries.json`, `generated.json` and
/// `crash.json` are already modified. The blanket check then refused a tree that
/// was doing exactly what the lane asked of it -- and the qualify script's
/// "not-ready is tolerated" branch read the STALE file, so the failure passed
/// silently and mutants.json kept an August commit.
///
/// `scripts/haqp_paths.py` is the sibling rule for the shell lanes. They agree
/// except on `conformance/tests/phase0.rs`, deliberately: the Python side runs
/// at FLIP time, after AM-17.6 has lifted the gate's `#[ignore]`, so that file
/// is legitimately modified by then. Here the check runs mid-campaign, before
/// the flip, where a modified gate file means someone edited gate code while
/// evidence was being produced -- which must invalidate the run.
fn source_dirt(root: &Utf8Path) -> Result<Vec<String>> {
    Ok(git_text(root, &["status", "--porcelain=v1"])?
        .lines()
        .filter_map(porcelain_path)
        .filter(|path| !qualification_metadata_path(path))
        .collect())
}

/// The path out of one `git status --porcelain=v1` line.
///
/// Not `line[3..]`. `git_text` trims its output, so the FIRST line loses the
/// leading space of a " M path" status and a fixed three-character slice eats
/// the first character of its path -- which is how this function first reported
/// `onformance/haqp/...`. That is the third appearance of this exact off-by-one:
/// `haqp_flip_packet.py` had it, its local copy in `haqp_blind_review.py` had
/// it, and it came back here in new code written by someone who knew about both.
/// Trimming first and splitting on the status field is immune to it.
fn porcelain_path(line: &str) -> Option<String> {
    let (_status, rest) = line.trim_start().split_once(' ')?;
    let path = rest.trim();
    // Rename and copy entries read "old -> new"; the destination is what is
    // present in the working tree.
    let path = path.rsplit(" -> ").next().unwrap_or(path);
    // Paths containing spaces or non-ASCII come back quoted; the quotes are not
    // part of the path.
    let path = path.trim_matches('"');
    (!path.is_empty()).then(|| path.to_owned())
}

/// Read one evidence artifact, transparently decompressing `.zst`.
///
/// A full 30-minute ASan campaign emits gigabytes of strace output — the
/// 2026-09-02 run produced a 1.56 GB trace for `graph_interchange_codec`, which
/// no remote will accept. zstd takes that to roughly 55 MB, and `zstd` is
/// already a workspace dependency used for exactly this under AM-11.6.
///
/// The digest is taken over the DECOMPRESSED bytes, which diverges from
/// AM-11.6 deliberately. AM-11.6 freezes the compressed artifact; here
/// `trace_blake3` already means "digest of the trace this campaign produced",
/// and keeping that meaning leaves every existing check saying exactly what it
/// said before. It also survives a zstd upgrade: the attestation is over
/// content, not over an encoding.
fn read_evidence_bytes(path: &Utf8Path) -> Result<Vec<u8>> {
    if path.extension() == Some("zst") {
        let file = fs::File::open(path).with_context(|| format!("open {path}"))?;
        zstd::decode_all(std::io::BufReader::new(file))
            .with_context(|| format!("{path}: evidence is not valid zstd"))
    } else {
        fs::read(path).with_context(|| format!("read {path}"))
    }
}

fn qualification_metadata_path(path: &str) -> bool {
    matches!(
        path,
        "conformance/haqp/packet.json"
            | "docs/execution/phase1-suite-review.md"
            | "docs/execution/m17-5-adversarial-findings.md"
    ) || path.starts_with("conformance/haqp/evidence/")
}

/// Requirement coordinates must resolve to committed source files. The
/// coordinate suffix remains domain-specific (for example `D18.1`), but a
/// nonexistent file cannot be an auditable normative source.
fn verify_requirement_sources(root: &Utf8Path, packet: &Packet) -> Result<()> {
    for requirement in &packet.requirements {
        let (source_file, coordinate) = requirement
            .source
            .split_once(':')
            .with_context(|| format!("requirement {} source lacks coordinate", requirement.id))?;
        let lower = source_file.to_ascii_lowercase();
        anyhow::ensure!(
            !source_file.starts_with('/')
                && !source_file.split('/').any(|part| part == "..")
                && !lower.contains("heldout")
                && !lower.contains("conformance/corpora"),
            "requirement {} source path is unsafe or locked",
            requirement.id
        );
        let path = root.join(source_file);
        let metadata = fs::symlink_metadata(&path).with_context(|| {
            format!(
                "requirement {} source file is missing: {path}",
                requirement.id
            )
        })?;
        anyhow::ensure!(
            metadata.file_type().is_file(),
            "requirement {} source is not a regular file: {path}",
            requirement.id
        );
        let source = fs::read_to_string(&path)
            .with_context(|| format!("read requirement {} source: {path}", requirement.id))?;
        anyhow::ensure!(
            has_exact_coordinate_anchor(&source, coordinate),
            "requirement {} coordinate {:?} is absent from {path}",
            requirement.id,
            coordinate
        );
    }
    Ok(())
}

/// A requirement coordinate is an exact anchor, not a substring assertion.
///
/// Semantic anchors such as `D18.1` must not match `D18.10`; heading anchors
/// use their complete trimmed Markdown heading (`## Exit gate`) so a generic
/// word such as `exit` cannot accidentally satisfy a gate row.
fn has_exact_coordinate_anchor(source: &str, coordinate: &str) -> bool {
    let coordinate = coordinate.trim();
    if coordinate.is_empty() {
        return false;
    }
    if coordinate.starts_with("## ") {
        return source.lines().any(|line| line.trim() == coordinate);
    }
    source.lines().any(|line| {
        line.match_indices(coordinate).any(|(offset, _)| {
            let before = line[..offset].chars().next_back();
            let after = line[offset + coordinate.len()..].chars().next();
            before.is_none_or(|ch| !coordinate_anchor_char(ch))
                && after.is_none_or(|ch| !coordinate_anchor_char(ch))
        })
    })
}

fn coordinate_anchor_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')
}

fn read_packet(root: &Utf8Path) -> Result<Packet> {
    let path = root.join("conformance/haqp/packet.json");
    let bytes = fs::read(&path).with_context(|| format!("read {path}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {path}"))
}

#[allow(
    clippy::too_many_lines,
    reason = "packet shape gate keeps closed inventory and evidence checks together"
)]
fn verify_packet_shape(packet: &Packet) -> Result<()> {
    require_eq(
        "suite_version",
        &packet.suite_version,
        "phase1-haqp1-proposed-v2-seven-fuzz",
    )?;
    require_eq("status", &packet.status, "proposed")?;
    require_eq("ratification", &packet.ratification, "unratified")?;
    anyhow::ensure!(
        matches!(
            packet.concurrent_code.as_str(),
            "deterministic_schedule_exploration" | "not_applicable"
        ),
        "concurrent_code must be deterministic_schedule_exploration or not_applicable"
    );
    if packet.locked_acceptance_corpora_touched {
        anyhow::bail!("locked_acceptance_corpora_touched must be false");
    }
    require_exact_ids(
        packet.requirements.iter().map(|row| row.id.as_str()),
        (1..=REQUIREMENT_COUNT).map(|idx| format!("P1-R{idx:03}")),
        "requirement",
    )?;
    require_unique(
        packet.requirements.iter().map(|row| row.id.as_str()),
        "requirement",
    )?;
    let authoritative = authoritative_requirement_map();
    anyhow::ensure!(
        packet.requirements.len() == authoritative.len(),
        "requirement inventory cardinality is not authoritative: got {}, expected {}",
        packet.requirements.len(),
        authoritative.len()
    );
    let requirement_ids = packet
        .requirements
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    require_unique(packet.tests.iter().map(|row| row.id.as_str()), "test")?;
    // The inventory grows as evidence kinds are declared (M17.5 F-05): the IDs
    // must stay dense and sequential, but the COUNT is not frozen at 8 — that
    // ceiling was the mechanism that kept 36 requirements on 8 coarse tests.
    require_exact_ids(
        packet.tests.iter().map(|row| row.id.as_str()),
        (1..=packet.tests.len()).map(|idx| format!("P1-T{idx:02}")),
        "test",
    )?;
    let mut covered_requirements = BTreeSet::new();
    for test in &packet.tests {
        if test.requirements.is_empty() {
            anyhow::bail!("{} has no requirement mapping", test.id);
        }
        for requirement in &test.requirements {
            if !requirement_ids.contains(requirement.as_str()) {
                anyhow::bail!("{} references unknown requirement {}", test.id, requirement);
            }
            covered_requirements.insert(requirement.as_str());
        }
    }
    for requirement in &packet.requirements {
        // M17.5 pass-2 #12: the `critical` guard let a non-critical requirement
        // sit in the inventory forever with no test behind it, inflating the
        // requirement count without adding any evidence. Every declared
        // requirement must be mapped; criticality decides how MUCH evidence is
        // needed (see `verify_evidence_coverage`), not whether any is.
        if !covered_requirements.contains(requirement.id.as_str()) {
            anyhow::bail!(
                "requirement {} ({}) is not mapped to any test — an unmapped \
                 requirement counts toward the inventory while proving nothing",
                requirement.id,
                if requirement.critical {
                    "critical"
                } else {
                    "non-critical"
                }
            );
        }
        if requirement.kind.trim().is_empty() || requirement.source.trim().is_empty() {
            anyhow::bail!("requirement {} has empty kind/source", requirement.id);
        }
        if !REQUIREMENT_KINDS.contains(&requirement.kind.as_str()) {
            anyhow::bail!(
                "requirement {} has unknown kind {:?}; expected one of {:?}",
                requirement.id,
                requirement.kind,
                REQUIREMENT_KINDS
            );
        }
        if STATEFUL_REQUIREMENT_IDS.contains(&requirement.id.as_str()) && !requirement.stateful {
            anyhow::bail!(
                "requirement {} is stateful and must set stateful=true",
                requirement.id
            );
        }
        let (source_file, coordinate) = requirement
            .source
            .split_once(':')
            .with_context(|| format!("requirement {} source lacks coordinate", requirement.id))?;
        anyhow::ensure!(
            !source_file.trim().is_empty() && !coordinate.trim().is_empty(),
            "requirement {} source must contain file and coordinate",
            requirement.id
        );
        let expected = authoritative
            .get(requirement.id.as_str())
            .with_context(|| {
                format!(
                    "requirement {} is not in the closed authoritative registry",
                    requirement.id
                )
            })?;
        anyhow::ensure!(
            requirement.kind == expected.0
                && requirement.source == expected.1
                && requirement.critical == expected.2
                && requirement.stateful == expected.3,
            "requirement {} differs from closed authoritative registry",
            requirement.id
        );
    }
    verify_evidence_coverage(packet)?;
    verify_test_registry(packet)?;
    verify_mutant_inventory(packet)?;
    verify_kill_concentration(packet)?;
    verify_canary_inventory(packet)?;
    verify_generated_inventory(packet)?;
    verify_reviews_inventory(packet)?;
    verify_crash_boundary_inventory(packet)?;
    verify_crash_boundary_scope_shape(packet)?;
    Ok(())
}

fn authoritative_requirement_map()
-> BTreeMap<&'static str, (&'static str, &'static str, bool, bool)> {
    AUTHORITATIVE_REQUIREMENTS
        .iter()
        .map(|(id, kind, source, critical, stateful)| (*id, (*kind, *source, *critical, *stateful)))
        .collect()
}

/// Evidence kinds every applicable critical requirement must carry
/// (ADR-0020 §2: "positive, negative, malformed/adversarial, basis/provenance,
/// and deterministic-replay evidence").
const CORE_EVIDENCE: [&str; 5] = ["positive", "negative", "malformed", "basis", "replay"];
/// "Stateful laws also have injected-fault and idempotent-recovery evidence."
const STATEFUL_EVIDENCE: [&str; 2] = ["fault", "recovery"];
/// Requirement kinds are closed by the HAQP packet contract. An open kind
/// vocabulary lets a packet relabel a stateful requirement and evade its
/// fault/recovery evidence obligations (M17.5 P1-A04).
const REQUIREMENT_KINDS: [&str; 4] = ["law", "gate", "fault", "abuse"];
/// Current packet's stateful law is M20 D20.3. Keep the semantic coordinate
/// closed even if an attacker relabels its `kind` or drops its JSON marker.
const STATEFUL_REQUIREMENT_IDS: [&str; 1] = ["P1-R015"];
/// No single test may carry more than this share of the mutation denominator.
const MAX_KILL_SHARE: f64 = 0.25;

/// M17.5 F-05: a requirement mapped to one coarse end-to-end test is not
/// covered. Every critical requirement must carry all five core evidence
/// kinds, and fault-class requirements must additionally carry injected-fault
/// and idempotent-recovery evidence.
fn verify_evidence_coverage(packet: &Packet) -> Result<()> {
    let known = CORE_EVIDENCE
        .iter()
        .chain(STATEFUL_EVIDENCE.iter())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut covered = BTreeMap::<&str, BTreeSet<&str>>::new();
    for test in &packet.tests {
        if test.evidence.is_empty() {
            anyhow::bail!("{} declares no evidence kind", test.id);
        }
        for kind in &test.evidence {
            if !known.contains(kind.as_str()) {
                anyhow::bail!("{} declares unknown evidence kind {kind}", test.id);
            }
        }
        for requirement in &test.requirements {
            covered
                .entry(requirement.as_str())
                .or_default()
                .extend(test.evidence.iter().map(String::as_str));
        }
    }
    for requirement in &packet.requirements {
        if !requirement.critical {
            continue;
        }
        let have = covered
            .get(requirement.id.as_str())
            .cloned()
            .unwrap_or_default();
        let mut needed = CORE_EVIDENCE.to_vec();
        if requirement.stateful || requirement.kind == "fault" {
            needed.extend_from_slice(&STATEFUL_EVIDENCE);
        }
        let missing = needed
            .into_iter()
            .filter(|kind| !have.contains(kind))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            anyhow::bail!(
                "critical requirement {} ({}) is missing {} evidence",
                requirement.id,
                requirement.kind,
                missing.join(", ")
            );
        }
    }
    Ok(())
}

/// Closed digest over test identity, executable name, requirement mapping, and
/// evidence kinds. Existence checks alone let a packet remap broad, unrelated
/// test names to satisfy coverage; this registry makes the authoritative
/// mapping tamper-evident (P1-A04). Recompute with the canonical NUL/US stream
/// below and update this constant in the same commit as any test inventory edit.
const TEST_REGISTRY_SHA256: &str =
    "7a29add0e81657e17786934010638abf1ea99dfee4e162921a9ef2699197d447";

fn verify_test_registry(packet: &Packet) -> Result<()> {
    let mut canonical = String::new();
    for test in &packet.tests {
        canonical.push_str(&test.id);
        canonical.push('\0');
        canonical.push_str(&test.name);
        canonical.push('\0');
        canonical.push_str(&test.requirements.join("\x1f"));
        canonical.push('\0');
        canonical.push_str(&test.evidence.join("\x1f"));
        canonical.push('\0');
    }
    let digest = sha256_text(&canonical);
    anyhow::ensure!(
        digest == TEST_REGISTRY_SHA256,
        "test registry digest differs from closed authority: got {digest}, expected {TEST_REGISTRY_SHA256}"
    );
    Ok(())
}

fn contains_exact_coordinate(text: &str, coordinate: &str) -> bool {
    text.match_indices(coordinate).any(|(start, _)| {
        let end = start + coordinate.len();
        !text[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == ':')
            && !text[end..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == ':')
    })
}

/// M17.5 F-05: ADR-0020 §3 forbids mutation coverage clustering on
/// easy-to-kill paths. A 100% kill rate carried by a handful of coarse
/// end-to-end tests measures almost nothing, so cap any one test's share of
/// the denominator and require every named killer to exist.
fn verify_kill_concentration(packet: &Packet) -> Result<()> {
    let total = packet.mutants.len();
    if total == 0 {
        return Ok(());
    }
    let test_ids = packet
        .tests
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut kills = BTreeMap::<&str, usize>::new();
    for mutant in &packet.mutants {
        for test in &mutant.killing_tests {
            if !test_ids.contains(test.as_str()) {
                anyhow::bail!("{} names unknown killing test {test}", mutant.id);
            }
            *kills.entry(test.as_str()).or_default() += 1;
        }
    }
    for (test, count) in &kills {
        #[allow(clippy::cast_precision_loss, reason = "inventory-scale counts")]
        let share = *count as f64 / total as f64;
        // M17.5 F-30: `>` -> `>=` survives here and is EQUIVALENT within the
        // reachable domain. `share` is `count / total` over 65 declared
        // mutants, and 65/4 is not an integer, so `share == 0.25` exactly
        // cannot occur.
        if share > MAX_KILL_SHARE {
            anyhow::bail!(
                "{test} is the named killer for {count}/{total} mutants ({:.0}%), \
                 above the {:.0}% ceiling — mutation coverage is clustering on one \
                 coarse test",
                share * 100.0,
                MAX_KILL_SHARE * 100.0
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct PacketStatusExpectations {
    mutant: &'static str,
    canary: &'static str,
    generated: &'static str,
    crash: &'static str,
    review: &'static str,
}

fn verify_packet_statuses(packet: &Packet, expected: PacketStatusExpectations) -> Result<()> {
    for mutant in &packet.mutants {
        if expected.mutant == "resolved" {
            anyhow::ensure!(
                mutant.disposition != "predeclared",
                "mutant {} remains predeclared in a resolved packet",
                mutant.id
            );
        } else {
            require_eq("mutant.disposition", &mutant.disposition, expected.mutant)?;
        }
    }
    for canary in &packet.canaries {
        require_eq("canary.result", &canary.result, expected.canary)?;
    }
    for family in &packet.generated {
        require_eq("generated.result", &family.result, expected.generated)?;
    }
    for row in &packet.crash_boundaries {
        require_eq("crash.result", &row.result, expected.crash)?;
    }
    for review in &packet.reviews {
        require_eq("review.result", &review.result, expected.review)?;
    }
    Ok(())
}

/// The five ADR-0020 §4 evidence families. A mutant naming anything else is not
/// describing this suite.
const MUTANT_FAMILIES: [&str; 5] = [
    "source/CST/formatting",
    "graph/interchange codecs",
    "transforms/projections",
    "repair/ILRP/recovery",
    "Basis/revision/query invalidation",
];

const GENERATED_FAMILIES: [&str; 5] = MUTANT_FAMILIES;

/// Phase 1's M18-M24 inventory is closed. Packet fields are claims; this
/// registry is the authority they must match byte-for-byte. A count-only check
/// lets a doctored packet remove a law and replace it with an invented row
/// while preserving the same cardinality, and a packet-controlled `critical`
/// bit can demote evidence obligations.
const AUTHORITATIVE_REQUIREMENTS: [(&str, &str, &str, bool, bool); 55] = [
    ("P1-R001", "law", "docs/execution/M18.md:D18.1", true, false),
    ("P1-R002", "law", "docs/execution/M18.md:D18.2", true, false),
    ("P1-R003", "law", "docs/execution/M18.md:D18.3", true, false),
    ("P1-R004", "law", "docs/execution/M18.md:D18.4", true, false),
    ("P1-R005", "law", "docs/execution/M18.md:D18.5", true, false),
    ("P1-R006", "law", "docs/execution/M18.md:D18.6", true, false),
    (
        "P1-R007",
        "gate",
        "docs/execution/M18.md:## Exit gate",
        true,
        false,
    ),
    ("P1-R008", "law", "docs/execution/M19.md:D19.1", true, false),
    ("P1-R009", "law", "docs/execution/M19.md:D19.2", true, false),
    ("P1-R010", "law", "docs/execution/M19.md:D19.3", true, false),
    ("P1-R011", "law", "docs/execution/M19.md:D19.4", true, false),
    (
        "P1-R012",
        "gate",
        "docs/execution/M19.md:## Exit gate",
        true,
        false,
    ),
    ("P1-R013", "law", "docs/execution/M20.md:D20.1", true, false),
    ("P1-R014", "law", "docs/execution/M20.md:D20.2", true, false),
    (
        "P1-R015",
        "fault",
        "docs/execution/M20.md:D20.3",
        true,
        true,
    ),
    (
        "P1-R016",
        "abuse",
        "docs/execution/M20.md:D20.4",
        true,
        false,
    ),
    ("P1-R017", "law", "docs/execution/M21.md:D21.1", true, false),
    ("P1-R018", "law", "docs/execution/M21.md:D21.2", true, false),
    ("P1-R019", "law", "docs/execution/M21.md:D21.3", true, false),
    ("P1-R020", "law", "docs/execution/M22.md:D22.1", true, false),
    ("P1-R021", "law", "docs/execution/M22.md:D22.2", true, false),
    ("P1-R022", "law", "docs/execution/M23.md:D23.1", true, false),
    ("P1-R023", "law", "docs/execution/M23.md:D23.6", true, false),
    (
        "P1-R024",
        "gate",
        "docs/execution/M24.md:D24.1",
        true,
        false,
    ),
    ("P1-R025", "law", "docs/execution/M20.md:D20.5", true, false),
    ("P1-R026", "law", "docs/execution/M20.md:D20.6", true, false),
    ("P1-R027", "law", "docs/execution/M20.md:D20.7", true, false),
    ("P1-R028", "law", "docs/execution/M21.md:D21.4", true, false),
    ("P1-R029", "law", "docs/execution/M21.md:D21.5", true, false),
    ("P1-R030", "law", "docs/execution/M21.md:D21.6", true, false),
    ("P1-R031", "law", "docs/execution/M21.md:D21.7", true, false),
    ("P1-R032", "law", "docs/execution/M22.md:D22.3", true, false),
    ("P1-R033", "law", "docs/execution/M22.md:D22.4", true, false),
    ("P1-R034", "law", "docs/execution/M22.md:D22.5", true, false),
    ("P1-R035", "law", "docs/execution/M22.md:D22.6", true, false),
    ("P1-R036", "law", "docs/execution/M22.md:D22.7", true, false),
    ("P1-R037", "law", "docs/execution/M19.md:D19.0", true, false),
    ("P1-R038", "law", "docs/execution/M19.md:D19.5", true, false),
    ("P1-R039", "law", "docs/execution/M19.md:D19.6", true, false),
    ("P1-R040", "law", "docs/execution/M19.md:D19.7", true, false),
    ("P1-R041", "law", "docs/execution/M19.md:D19.8", true, false),
    ("P1-R042", "law", "docs/execution/M20.md:D20.0", true, false),
    (
        "P1-R043",
        "gate",
        "docs/execution/M20.md:## Exit gate",
        true,
        false,
    ),
    (
        "P1-R044",
        "gate",
        "docs/execution/M21.md:## Exit gate",
        true,
        false,
    ),
    (
        "P1-R045",
        "gate",
        "docs/execution/M22.md:## Exit gate",
        true,
        false,
    ),
    ("P1-R046", "law", "docs/execution/M23.md:D23.2", true, false),
    ("P1-R047", "law", "docs/execution/M23.md:D23.3", true, false),
    ("P1-R048", "law", "docs/execution/M23.md:D23.4", true, false),
    ("P1-R049", "law", "docs/execution/M23.md:D23.5", true, false),
    (
        "P1-R050",
        "gate",
        "docs/execution/M23.md:## Exit gate",
        true,
        false,
    ),
    ("P1-R051", "law", "docs/execution/M24.md:D24.2", true, false),
    ("P1-R052", "law", "docs/execution/M24.md:D24.3", true, false),
    ("P1-R053", "law", "docs/execution/M24.md:D24.4", true, false),
    (
        "P1-R054",
        "gate",
        "docs/execution/M24.md:D24.5",
        true,
        false,
    ),
    (
        "P1-R055",
        "gate",
        "docs/execution/M24.md:## Exit gate",
        true,
        false,
    ),
];

const REQUIREMENT_COUNT: usize = AUTHORITATIVE_REQUIREMENTS.len();

/// The declared mutation operators. Closed by design (M17.5 pass-2 #13): an
/// open vocabulary lets a fabricated inventory invent an operator per mutant
/// and sail through the per-operator concentration ceiling, since a label used
/// once can never exceed it.
const MUTANT_OPERATORS: [&str; 13] = [
    "predicate-deletion",
    "predicate-inversion",
    "threshold-plus-one",
    "threshold-minus-one",
    "missing-enum-dispatch",
    "success-error-substitution",
    "oracle-short-circuit",
    "ordering-nondeterminism",
    "stale-basis-acceptance",
    "skipped-durable-transition",
    "disabled-crash-point",
    "broadened-allow-list",
    "wrong-holder-selection",
];

/// Dispositions a mutant row may carry. `equivalent` and `duplicate` are the
/// only ADR-0020 §3 escapes from "one survivor blocks eligibility", and both
/// demand a written proof, so neither may be spelled freehand.
const MUTANT_DISPOSITIONS: [&str; 4] = ["predeclared", "killed", "equivalent", "duplicate"];

fn verify_mutant_inventory(packet: &Packet) -> Result<()> {
    if packet.mutants.len() != 65 {
        anyhow::bail!("mutant count must be 65, got {}", packet.mutants.len());
    }
    require_unique(packet.mutants.iter().map(|row| row.id.as_str()), "mutant")?;
    let requirement_ids = packet
        .requirements
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut by_family = BTreeMap::<&str, usize>::new();
    let mut by_operator = BTreeMap::<&str, usize>::new();
    for mutant in &packet.mutants {
        // M17.5 pass-2 #13: family/operator/source/disposition were free text,
        // so a fabricated inventory with invented labels was accepted whole.
        // Each must now name something this suite actually declares.
        if !MUTANT_FAMILIES.contains(&mutant.family.as_str()) {
            anyhow::bail!(
                "{} declares unknown mutation family {:?}; expected one of {:?}",
                mutant.id,
                mutant.family,
                MUTANT_FAMILIES
            );
        }
        if !MUTANT_OPERATORS.contains(&mutant.operator.as_str()) {
            anyhow::bail!(
                "{} declares unknown mutation operator {:?}; expected one of {:?}",
                mutant.id,
                mutant.operator,
                MUTANT_OPERATORS
            );
        }
        if !MUTANT_DISPOSITIONS.contains(&mutant.disposition.as_str()) {
            anyhow::bail!(
                "{} declares unknown disposition {:?}; expected one of {:?}",
                mutant.id,
                mutant.disposition,
                MUTANT_DISPOSITIONS
            );
        }
        let requirement = if mutant.requirement.trim().is_empty() {
            // Proposed inventories from before the coordinate contract used
            // source for requirement ID. Preserve that plan shape, but never
            // treat it as an evaluated coordinate (qualified gate below is
            // strict).
            mutant.source.as_str()
        } else {
            mutant.requirement.as_str()
        };
        if !requirement_ids.contains(requirement) {
            anyhow::bail!(
                "{} names requirement {:?}, which is not a declared requirement — a mutant must \
                 attack something the inventory claims to require",
                mutant.id,
                requirement
            );
        }
        if !mutant.source.starts_with("P1-R") && mutant.requirement.trim().is_empty() {
            anyhow::bail!(
                "{} source {:?} is a coordinate but requirement ID is missing",
                mutant.id,
                mutant.source
            );
        }
        if !mutant.requirement.trim().is_empty() {
            let (file, line) = mutant.source.rsplit_once(':').with_context(|| {
                format!(
                    "{} source must be exact file:line; source is not a declared requirement",
                    mutant.id
                )
            })?;
            let line: usize = line
                .parse()
                .with_context(|| format!("{} source line is not numeric", mutant.id))?;
            anyhow::ensure!(
                line > 0
                    && !file.is_empty()
                    && !file.starts_with('/')
                    && !file.split('/').any(|part| part == ".."),
                "{} source must be safe file:line",
                mutant.id
            );
        }
        if mutant.defect.trim().is_empty() {
            anyhow::bail!("{} describes no defect", mutant.id);
        }
        validate_disposition(mutant)?;
        *by_family.entry(mutant.family.as_str()).or_default() += 1;
        *by_operator.entry(mutant.operator.as_str()).or_default() += 1;
        if mutant.disposition == "predeclared" && mutant.killing_tests.is_empty() {
            anyhow::bail!("{} has no killing test", mutant.id);
        }
    }
    verify_mutation_plan_balance(&by_family, &by_operator)?;
    Ok(())
}

/// ADR-0020 §3's shape rules for the plan: thirteen mutants per family, no
/// operator over a quarter of the denominator, and every named operator
/// present (blind pass 1 at 5fb1b57, A04).
fn verify_mutation_plan_balance(
    by_family: &BTreeMap<&str, usize>,
    by_operator: &BTreeMap<&str, usize>,
) -> Result<()> {
    const REQUIRED_OPERATORS: [&str; 13] = [
        "predicate-deletion",
        "predicate-inversion",
        "threshold-plus-one",
        "threshold-minus-one",
        "missing-enum-dispatch",
        "success-error-substitution",
        "stale-basis-acceptance",
        "wrong-holder-selection",
        "skipped-durable-transition",
        "disabled-crash-point",
        "ordering-nondeterminism",
        "oracle-short-circuit",
        "broadened-allow-list",
    ];
    for (family, count) in by_family {
        if *count != 13 {
            anyhow::bail!("family {family} mutant count must be 13, got {count}");
        }
    }
    for (operator, count) in by_operator {
        if *count > 16 {
            anyhow::bail!("operator {operator} supplies {count} mutants; max 16");
        }
    }
    // Blind pass 1 at 5fb1b57 (A04): the tally only bounded operators that were
    // PRESENT, so dropping one entirely — broadened-allow-list has a single
    // mutant — left the plan short an operator ADR-0020 §3 names and nothing
    // said so. The list is the ADR's, closed.
    let missing = REQUIRED_OPERATORS
        .into_iter()
        .filter(|operator| !by_operator.contains_key(operator))
        .collect::<Vec<_>>();
    anyhow::ensure!(
        missing.is_empty(),
        "the mutation plan declares no {missing:?}; ADR-0020 §3 names every operator the plan \
         must exercise, and an absent one cannot be killed or counted"
    );
    Ok(())
}

#[allow(clippy::too_many_lines)]
/// Whether a declared anchor line contains anything a mutation can change
/// (M17.5 F-33 / blind pass 1 A04, A06, A10).
///
/// Operator rules already existed, but only over a mutation's before/after text
/// — i.e. only at stage 1b, where evidence exists. At 1a the packet's declared
/// anchors were checked for POSITION and never for whether the line held any
/// behaviour. Blind pass 1 walked in and found operators anchored to function
/// signatures and enum variants.
///
/// This rejects exactly one thing: a line that DECLARES rather than does. A
/// signature has no predicate to invert, no threshold to shift, no crash point
/// to disable, and no ordering to perturb; whatever the operator, the mutation
/// cannot be applied there, and a mutant that cannot be applied can never be
/// killed while still inflating the denominator §3 counts.
///
/// It deliberately does NOT try to match operators to line contents. A first
/// version did, and rejected three legitimate anchors: `NodeFlags(self.0 |
/// other.0)` is a real inversion site, `if rev == inner.state.head` is a real
/// staleness comparison, and `for entry in entries.flatten()` is a real
/// ordering site. Guessing which token an operator needs produces false
/// accusations against a mutation plan; "is this a declaration" does not.
/// Whether a mutant is GOOD is the 1b runner's question.
/// The aliases a `use` line introduces for the symbols it imports:
/// `use std::fs::{rename as mv, remove_file};` yields `("rename", "mv")`.
/// Blind-review follow-up (review of c455cdf): splitting the whole line on
/// `" as "` mangled a brace group into an alias like `mv, remove_file}`, which
/// matched nothing and reopened the evasion the alias scan closed.
/// `line` with a leading item visibility removed: `pub`, `pub(crate)`,
/// `pub(super)`, `pub(in crate::x)`. Unchanged when it carries none, and
/// unchanged for an identifier that merely starts with those letters.
fn without_visibility(line: &str) -> &str {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix("pub") else {
        return trimmed;
    };
    let rest = if let Some(open) = rest.strip_prefix('(') {
        match open.find(')') {
            Some(close) => &open[close + 1..],
            None => return trimmed,
        }
    } else {
        rest
    };
    if rest.starts_with(char::is_whitespace) {
        rest.trim_start()
    } else {
        trimmed
    }
}

fn use_aliases(line: &str) -> Vec<(String, String)> {
    // Blind pass 1 at ee646f8c (A10, A11): `pub use x::y as z;` is a `use`
    // with a visibility in front, and stripping only `use ` meant the alias
    // bound nothing — in the oracle's independence scan and in the durable
    // transition census, which share this parser.
    let Some(rest) = without_visibility(line).strip_prefix("use ") else {
        return Vec::new();
    };
    // Braces are dropped rather than parsed, so a nested group
    // (`use a::{b::{c as d}};`) reads the same as a flat one: what matters is
    // the trailing `symbol as alias`, and the path before it is only used for
    // its last segment (review of d09978c).
    let flattened = rest.trim_end_matches(';').replace(['{', '}'], "");
    let items = flattened.split(',').map(str::trim).collect::<Vec<_>>();
    items
        .iter()
        .filter_map(|item| {
            let (path, alias) = item.split_once(" as ")?;
            // `rsplit` always yields at least one element.
            let symbol = path
                .rsplit("::")
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            let alias = alias.trim().to_owned();
            (alias != "_").then_some((symbol, alias))
        })
        .collect()
}

fn anchor_is_mutable_line(line: &str) -> bool {
    let trimmed = line.trim();
    let declares_item = [
        "fn ",
        "pub fn ",
        "pub(crate) fn ",
        "async fn ",
        "pub async fn ",
        "struct ",
        "pub struct ",
        "enum ",
        "pub enum ",
        "trait ",
        "pub trait ",
        "impl ",
        // Blind pass 1 at 9eca4f0 (A05): `impl<C: CrashInjector> CrashInjector
        // for &C {` starts with `impl<`, not `impl `, so a generic impl header
        // read as behaviour.
        "impl<",
        "mod ",
        "pub mod ",
        "type ",
        "pub type ",
        "use ",
        "pub use ",
        "pub(crate) use ",
        // Blind pass 1 at aa00d41 (A04): P1-M046 was anchored on
        // `pub const REPAIR_STALE_BASIS: &str = "JUR053";`. Deleting a
        // diagnostic code's name changes compilation, not staleness semantics.
        // A const is not always inert, though — see `binds_a_threshold`.
        "const ",
        "pub const ",
        "pub(crate) const ",
        "static ",
        "pub static ",
    ]
    .iter()
    .any(|kw| trimmed.starts_with(kw));
    // A bare `Variant {` / `Variant,` inside an enum body.
    let declares_variant = trimmed.chars().next().is_some_and(char::is_uppercase)
        && (trimmed.ends_with('{') || trimmed.ends_with(','))
        && !trimmed.contains('=')
        && !trimmed.contains('(');
    // A constant that binds a NUMBER is the canonical threshold site:
    // `pub const MAX_NESTING: u16 = 256;` is exactly what threshold ±1 moves,
    // and refusing it would push the plan off the very lines §3 asks for. A
    // constant binding a string, or an array whose length is the only number,
    // stays a declaration: 256 can become 257, `[IntentState; 7]` cannot
    // become 8 and still compile.
    let binds_a_threshold = declares_item
        && trimmed.contains("const ")
        && trimmed.split_once('=').is_some_and(|(_, value)| {
            // The VALUE must be a number, not merely contain a digit:
            // `pub const TAG: &str = "v2";` is still a declaration (review of
            // c455cdf). A newtype around a number counts: `RelationFlags(1)`.
            let value = value.trim().trim_end_matches(';').trim();
            let inner = value
                .split_once('(')
                .map_or(value, |(_, rest)| rest.trim_end_matches(')'));
            // Strip a trailing type suffix (`256u16`) before asking whether
            // what remains is a number.
            !value.starts_with('"')
                && !inner.is_empty()
                && inner
                    .trim_end_matches(|ch: char| ch.is_ascii_alphabetic() || ch == '_')
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ch == '_')
        });
    // Blind pass 1 at 9eca4f0 (A11): a multi-line signature's continuation —
    // `) -> Result<WorkspaceBasis, PerspectiveError> {` — begins with none of
    // the keywords above and so read as behaviour. A line that closes a
    // parameter list and opens a body is still a declaration.
    // `) -> Result<..> {` and `) {` continue a signature. A bare `);` does NOT:
    // it terminates any multi-line call, which is behaviour, and refusing it
    // would push legitimate anchors off real code (review of `e490671`).
    let continues_signature =
        trimmed.starts_with(')') && (trimmed.contains("->") || trimmed.ends_with('{'));
    !(declares_item || declares_variant || continues_signature) || binds_a_threshold
}

/// Whether `line` in `text` falls inside a `#[cfg(test)]` item.
///
/// Blind pass 1 at 9eca4f0 (A12): P1-M053 was anchored on
/// `assert!(!deps.invalidated_by(&b));` inside `mod tests`. Deleting that
/// predicate mutates the ORACLE, so killing it measures the test's own
/// assertion rather than the product's invalidation behaviour — the shared-
/// oracle defect this campaign exists to catch, declared as a mutant.
fn line_is_inside_test_code(text: &str, line: usize) -> bool {
    let mut depth = 0i32;
    let mut in_test = false;
    let mut test_depth = 0i32;
    let mut scanner = SourceScanner::default();
    for (index, source) in text.lines().enumerate() {
        let trimmed = source.trim();
        if index + 1 == line {
            return in_test;
        }
        if !in_test && trimmed.replace(' ', "") == "#[cfg(test)]" {
            in_test = true;
            test_depth = depth;
            scanner.structural(source);
            continue;
        }
        // Braces inside a comment or a string literal are text, not structure:
        // one `// weird }` would close the test module early and let a
        // test-code anchor read as production (review of `e490671`).
        let structural = scanner.structural(source);
        depth += i32::try_from(structural.matches('{').count()).unwrap_or(0);
        depth -= i32::try_from(structural.matches('}').count()).unwrap_or(0);
        if in_test && depth <= test_depth && structural.contains('}') {
            in_test = false;
        }
    }
    false
}

/// Brace counting that survives a line ending. Blind pass 1 at `ec045588`
/// (A07): the old per-line reader could not see a raw string or block comment
/// that spans lines, so their braces counted as structure — enough to hold a
/// `#[cfg(test)]` skip open past the module and hide a production spawn after
/// it. The state carries across lines, which is what a multi-line literal is.
///
/// A `'` opens a character literal only when it closes within one escape
/// sequence. Otherwise it introduces a lifetime, and reading `Formatter<'_>`
/// as an unterminated literal would swallow the `{` that follows it (F-56).
#[derive(Default)]
struct SourceScanner {
    in_block_comment: bool,
    /// Hashes of the raw string currently open, if one is.
    in_raw_string: Option<usize>,
}

impl SourceScanner {
    /// `line` with its comments and literals removed, continuing whatever the
    /// previous line left open, so only braces that structure code remain.
    fn structural(&mut self, line: &str) -> String {
        let chars: Vec<char> = line.chars().collect();
        let mut out = String::with_capacity(line.len());
        let mut i = 0;
        while i < chars.len() {
            if self.in_block_comment {
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    self.in_block_comment = false;
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            if let Some(hashes) = self.in_raw_string {
                let closes = chars[i] == '"'
                    && chars[i + 1..]
                        .iter()
                        .take(hashes)
                        .filter(|c| **c == '#')
                        .count()
                        == hashes;
                if closes {
                    self.in_raw_string = None;
                    i += 1 + hashes;
                } else {
                    i += 1;
                }
                continue;
            }
            if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                self.in_block_comment = true;
                i += 2;
                continue;
            }
            if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
                break;
            }
            if let Some((hashes, after)) = raw_string_opening(&chars, i) {
                if let Some(end) = raw_string_close(&chars, after, hashes) {
                    i = end;
                    continue;
                }
                self.in_raw_string = Some(hashes);
                break;
            }
            if chars[i] == '"' {
                i = skip_string(&chars, i).max(i + 1);
                continue;
            }
            if let Some(after) = skip_char_literal(&chars, i) {
                i = after;
                continue;
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }
}

/// The hash count and the index just past the opening quote of a raw string
/// starting at `at`, or `None` if one does not start there.
fn raw_string_opening(chars: &[char], at: usize) -> Option<(usize, usize)> {
    let mut i = at;
    if chars.get(i) == Some(&'b') {
        i += 1;
    }
    if chars.get(i) != Some(&'r') {
        return None;
    }
    i += 1;
    let hashes = chars[i..].iter().take_while(|c| **c == '#').count();
    i += hashes;
    if chars.get(i) != Some(&'"') {
        return None;
    }
    Some((hashes, i + 1))
}

/// The index past a raw string's terminator, searching from `from`, or `None`
/// when the line ends inside it. Counted rather than `all`: on a tail shorter
/// than `hashes` an `all` is vacuously true, which would close the literal on
/// a quote that does not terminate it.
fn raw_string_close(chars: &[char], from: usize, hashes: usize) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == '"'
            && chars[i + 1..]
                .iter()
                .take(hashes)
                .filter(|c| **c == '#')
                .count()
                == hashes
        {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    None
}

/// The index past the string literal opening at `at`, or the end of the line.
fn skip_string(chars: &[char], at: usize) -> usize {
    let mut i = at + 1;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '"' => return i + 1,
            _ => i += 1,
        }
    }
    chars.len()
}

/// The index past the character literal opening at `at`, or `None` when the
/// quote introduces a lifetime rather than a literal.
fn skip_char_literal(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at) != Some(&'\'') {
        return None;
    }
    if chars.get(at + 1) == Some(&'\\') {
        // Past the backslash and the character it escapes: in `'\''` the quote
        // at `at + 2` is the escaped character, not the terminator.
        let close = chars.get(at + 3..)?.iter().position(|c| *c == '\'')?;
        return Some(at + 4 + close);
    }
    if chars.get(at + 2) == Some(&'\'') {
        return Some(at + 3);
    }
    None
}

#[allow(
    clippy::too_many_lines,
    reason = "one mutant's coordinate contract is one contract; the length is the field count"
)]
fn verify_mutant_source_coordinates(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let mut seen = BTreeSet::new();
    for mutant in &packet.mutants {
        // Legacy predeclared rows are an inventory plan; M24 must replace
        // these IDs with exact file:line coordinates before qualification.
        if mutant.source.starts_with("P1-R") && mutant.requirement.trim().is_empty() {
            anyhow::bail!(
                "{} still uses legacy requirement ID in mutant source; qualified evidence requires file:line plus requirement",
                mutant.id
            );
        }
        let (file, line) = mutant
            .source
            .rsplit_once(':')
            .with_context(|| format!("{} mutant source must be exact file:line", mutant.id))?;
        let line: usize = line
            .parse()
            .with_context(|| format!("{} mutant source line is not numeric", mutant.id))?;
        anyhow::ensure!(
            line > 0,
            "{} mutant source line must be positive",
            mutant.id
        );
        let (expected_file, expected_line, expected_anchor) = mutant_source_coordinate(&mutant.id)
            .with_context(|| format!("{} has no closed source registry entry", mutant.id))?;
        anyhow::ensure!(
            file == expected_file && line == expected_line,
            "{} source {}:{} does not match closed registry {}:{}",
            mutant.id,
            file,
            line,
            expected_file,
            expected_line
        );
        anyhow::ensure!(
            seen.insert((file.to_owned(), line)),
            "{} reuses source coordinate {}:{}",
            mutant.id,
            file,
            line
        );
        let path = safe_repo_path(root, file, "mutant source")?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("{} mutant source file is missing", mutant.id))?;
        anyhow::ensure!(
            source.lines().nth(line - 1).is_some(),
            "{} mutant source coordinate {}:{} is outside file",
            mutant.id,
            file,
            line
        );
        let anchor = source
            .lines()
            .nth(line - 1)
            .map(str::trim_start)
            .unwrap_or_default();
        anyhow::ensure!(
            anchor.starts_with(expected_anchor),
            "{} mutant source coordinate {}:{} does not name family anchor {:?}",
            mutant.id,
            file,
            line,
            expected_anchor
        );
        anyhow::ensure!(
            !mutant.requirement.trim().is_empty()
                && packet
                    .requirements
                    .iter()
                    .any(|requirement| requirement.id == mutant.requirement),
            "{} mutant coordinate lacks a declared requirement ID",
            mutant.id
        );
        if let Some(patch) = &mutant.patch {
            anyhow::ensure!(
                patch.file == file,
                "{} patch file {:?} does not match mutant source file {:?}",
                mutant.id,
                patch.file,
                file
            );
            let source_line = source.lines().nth(line - 1).unwrap_or_default();
            let patch_anchor = patch
                .before
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default();
            anyhow::ensure!(
                !patch_anchor.is_empty() && source_line.trim().contains(patch_anchor),
                "{} patch before text is not anchored at declared source {}:{}",
                mutant.id,
                file,
                line
            );
            anyhow::ensure!(
                patch
                    .before
                    .lines()
                    .filter(|text| !text.trim().is_empty())
                    .count()
                    == 1
                    && source_line.trim() == patch_anchor,
                "{} patch coordinate {}:{} is not an exact single-line source anchor",
                mutant.id,
                file,
                line
            );
            verify_mutant_operator_patch(&mutant.operator, patch)?;
        }
    }
    Ok(())
}

/// Every declared mutant's anchor line must be able to CARRY its operator
/// (M17.5 F-33). Runs at every layer, including stage 1a, because an
/// inapplicable mutant is a defect in the inventory, not in the evidence.
/// The locked corpus must be reachable by exactly one name (M17.5 F-34 /
/// blind pass 1 A07).
///
/// The corpus-access audit refuses a traced path containing `heldout` or
/// `conformance/corpora`. That is substring matching on what the process SAW,
/// so it catches direct access and nothing else: a symlink or a rename gives
/// the same bytes a path the filter does not recognise, and the audit reports
/// clean while the locked corpus was read.
///
/// Rather than try to canonicalize strace output after the fact — the symlink
/// may not exist by verification time — the aliasing itself is made
/// impossible. If the locked corpus has exactly one name, then matching that
/// name is sufficient, and the existing filter becomes sound rather than
/// lucky.
fn verify_locked_corpus_has_no_aliases(root: &Utf8Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let locked = root.join("conformance/corpora/heldout");
    // M17.5 F-65: this returned Ok when the corpus was absent, and nothing
    // else in the gate required it to exist — so a tree with the corpus
    // deleted passed every corpus check vacuously while the packet went on
    // declaring `locked_acceptance_corpora_touched: false`. Twenty-three files
    // are tracked; absence is a broken tree, not an empty obligation.
    anyhow::ensure!(
        locked.is_dir(),
        "{locked} is missing: the locked corpus cannot be shown untouched when it is not there"
    );
    let identities = locked_corpus_identities(root)?;
    let canonical = fs::canonicalize(&locked)
        .with_context(|| format!("{locked}: locked corpus must resolve"))?;
    let mut aliases = Vec::new();
    let mut stack = vec![root.to_owned()];
    while let Some(dir) = stack.pop() {
        // A directory this cannot read is a directory it cannot clear of
        // aliases; the same silent skip was a hard-link bypass in F-64.
        let entries = fs::read_dir(&dir)
            .with_context(|| format!("{dir}: scan the tree for corpus aliases"))?;
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue;
            };
            let name = path.file_name().unwrap_or_default();
            if name == ".git" || name == "target" {
                continue;
            }
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            // Blind pass 1 at `ec045588` (A08): only symlinks counted. A hard
            // link is the alias no path reasoning sees, and one created
            // outside a traced process leaves the trace naming an innocent
            // path; if it is then removed, neither the lexical fragments nor
            // the inode check can resolve it afterwards. An alias that still
            // exists in the tree is found here, which is where it can be.
            if meta.is_file() && identities.contains(&(meta.dev(), meta.ino())) {
                // The identity set covers the whole corpora tree, so a file
                // inside it is the corpus rather than an alias to it.
                let corpora = root.join("conformance/corpora");
                let inside = Utf8PathBuf::from_path_buf(entry.path())
                    .is_ok_and(|path| path.starts_with(&corpora));
                if !inside {
                    aliases.push(entry.path().display().to_string());
                }
            }
            if meta.file_type().is_symlink() {
                // A link INTO the locked tree gives its bytes a second name.
                if let Ok(resolved) = fs::canonicalize(entry.path())
                    && (resolved == canonical || resolved.starts_with(&canonical))
                    && path != locked
                {
                    aliases.push(path.to_string());
                }
            } else if meta.is_dir() {
                stack.push(path);
            }
        }
    }
    anyhow::ensure!(
        aliases.is_empty(),
        "the locked acceptance corpus is reachable under {} alias(es), so a traced \
         path can read it without naming it: {aliases:?}",
        aliases.len()
    );
    Ok(())
}

/// The calls that carry a durable transition. Shared by the operator contract
/// and the anchor precondition so the two cannot drift. Blind pass 1 at
/// `aeed70f9` (A05): `fs::rename` publishes a staged file atomically and was
/// on no list, so P1-M035's anchor — the rename itself — read as a line with
/// no durable transition on it.
const DURABLE_CALL_MARKERS: [&str; 10] = [
    "commit_intent(",
    "prepare(",
    "finalize(",
    "persist(",
    "ack(",
    ".commit(",
    ".commit_if(",
    ".append(",
    "write(",
    "rename(",
];

/// Whether `anchor` could be the `before` text of `operator`'s prescribed
/// mutation. `verify_mutant_operator_patch` states the lexical contract over
/// both sides of a patch, but no patch exists at stage 1a, so that contract
/// had never been applied to an anchor: six mutants declared operators their
/// anchor could not support, including a `predicate-deletion` on a line with
/// no negation and a `missing-enum-dispatch` on a match ARM rather than the
/// match. This is the half of the contract the anchor alone can satisfy.
///
/// `<` and `>` are required to be spaced. Rustfmt writes comparisons that way
/// and generic parameters the other, and without the distinction every struct
/// field holding a `BTreeMap<K, V>` reads as a comparison.
/// Whether `trimmed` uses `||` as a disjunction rather than as a closure's
/// empty parameter list. Blind pass 1 at `ec045588` (A04): `ok_or_else(|| {`
/// carries a `||` that no short-circuit mutation can act on, and the
/// precondition counted it — the same mistake as a pattern's `|`, one token
/// wider. A disjunction has an operand to its left.
fn has_disjunction(trimmed: &str) -> bool {
    trimmed.match_indices("||").any(|(at, _)| {
        trimmed[..at]
            .trim_end()
            .chars()
            .next_back()
            .is_some_and(|previous| previous.is_alphanumeric() || matches!(previous, '_' | ')'))
    })
}

/// The lines a wrong-holder mutation can redirect. Blind pass 1 at `ec045588`
/// (A05): `self.store.begin()?` reads a member off a receiver exactly as
/// `inner.state.nodes.get(&id)` does, and one is the only store there is while
/// the other has a sibling to be redirected to. Nothing in the text says
/// which, so this operator's reach is not lexically decidable, and pretending
/// otherwise is how P1-M047 came to declare a mutation no patch could
/// implement. The set is closed and shared with
/// `verify_mutant_operator_patch`: one statement of where the operator
/// applies, not two that can drift.
const WRONG_HOLDER_ANCHORS: [&str; 2] = [
    "&self.basis",
    "return Ok(inner.state.nodes.get(&id).cloned());",
];

fn anchor_supports_operator(operator: &str, anchor: &str) -> bool {
    let trimmed = anchor.trim();
    let lower = trimmed.to_ascii_lowercase();
    let chars: Vec<char> = trimmed.chars().collect();
    let negation = chars.iter().enumerate().any(|(index, ch)| {
        *ch == '!'
            && chars.get(index + 1) != Some(&'=')
            && !index
                .checked_sub(1)
                .and_then(|prev| chars.get(prev))
                .is_some_and(|prev| prev.is_alphanumeric() || *prev == '_')
    });
    let integer = chars.iter().enumerate().any(|(index, ch)| {
        ch.is_ascii_digit()
            && !index
                .checked_sub(1)
                .and_then(|prev| chars.get(prev))
                .is_some_and(|prev| prev.is_alphanumeric() || *prev == '_')
    });
    let word = |needle: &str| {
        trimmed
            .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .any(|token| token == needle)
    };
    match operator {
        // `|` is one of the contract's inversion pairs and must stay, but only
        // when it occurs once: a closure's `|m| ...` has two, and a generic's
        // `<` is excluded by requiring spaces, because inverting the `<` of
        // `Vec<String>` yields `Vec>=String>`, which is not a program.
        "predicate-inversion" => {
            // Blind pass 1 at `0a0b4b4b` (A04): a `|` in a PATTERN is an
            // alternative, not an operator — inverting the bar of
            // `A | B => ...` yields `A & B`, which is not a pattern and does
            // not compile, so the mutant could never be applied let alone
            // killed. A bar counts only where it can be bitwise or.
            let pattern_alternative = trimmed.contains("=>") || trimmed.starts_with('|');
            [
                "==", "!=", " <= ", " >= ", " < ", " > ", "&&", " true", " false",
            ]
            .iter()
            .any(|token| trimmed.matches(token).count() == 1)
                || has_disjunction(trimmed)
                || (!pattern_alternative && trimmed.matches('|').count() == 1)
        }
        "predicate-deletion" => negation,
        "threshold-plus-one" | "threshold-minus-one" => integer,
        "missing-enum-dispatch" => word("match"),
        "success-error-substitution" => trimmed.contains("Ok(") || trimmed.contains("Err("),
        "oracle-short-circuit" => {
            trimmed.contains("&&") || has_disjunction(trimmed) || trimmed == "if id.is_some() {"
        }
        "ordering-nondeterminism" => {
            trimmed.contains("sort")
                || trimmed.contains("BTree")
                || trimmed.starts_with("let lines: Vec<")
        }
        "stale-basis-acceptance" => {
            lower.contains("basis") || trimmed == "if rev == inner.state.head {"
        }
        "skipped-durable-transition" => DURABLE_CALL_MARKERS
            .iter()
            .any(|call| trimmed.contains(call)),
        "disabled-crash-point" => {
            lower.contains("crash")
                || trimmed == "if fail_after.is_some_and(|limit| committed >= limit) {"
        }
        "broadened-allow-list" => {
            word("match")
                || trimmed.contains("matches!")
                || trimmed.contains("ensure!")
                || trimmed.contains("=>")
        }
        "wrong-holder-selection" => WRONG_HOLDER_ANCHORS.contains(&trimmed),
        _ => false,
    }
}

fn verify_mutant_anchors_support_operators(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let mut inapplicable = Vec::new();
    for mutant in &packet.mutants {
        let Some((file, line)) = mutant.source.rsplit_once(':') else {
            anyhow::bail!("{} source {:?} is not file:line", mutant.id, mutant.source);
        };
        let line: usize = line
            .parse()
            .with_context(|| format!("{} source line {line:?} is not a number", mutant.id))?;
        let text = fs::read_to_string(root.join(file))
            .with_context(|| format!("{}: read anchor file {file}", mutant.id))?;
        let anchor = text
            .lines()
            .nth(line.saturating_sub(1))
            .with_context(|| format!("{} anchors {file}:{line}, past end of file", mutant.id))?;
        if line_is_inside_test_code(&text, line) {
            inapplicable.push(format!(
                "{} declares {:?} at {file}:{line}, which is inside test code: mutating an \
                 assertion measures the ORACLE, not the product",
                mutant.id, mutant.operator
            ));
            continue;
        }
        if !anchor_supports_operator(&mutant.operator, anchor) {
            inapplicable.push(format!(
                "{} declares {:?} at {file}:{line}, whose text cannot be the BEFORE side of \
                 that operator's mutation: {:?}",
                mutant.id,
                mutant.operator,
                anchor.trim()
            ));
            continue;
        }
        if !anchor_is_mutable_line(anchor) {
            inapplicable.push(format!(
                "{} declares {:?} at {file}:{line}, which is a declaration, not \
                 behaviour: {:?}",
                mutant.id,
                mutant.operator,
                anchor.trim()
            ));
        }
    }
    anyhow::ensure!(
        inapplicable.is_empty(),
        "{} declared mutant(s) cannot be applied, so they can never be killed and \
         inflate the denominator ADR-0020 §3 counts:\n  - {}",
        inapplicable.len(),
        inapplicable.join("\n  - ")
    );
    Ok(())
}

/// Require an evaluated patch to exhibit the lexical mutation prescribed by
/// its closed operator.  Anchoring and compilation alone allow an unrelated
/// replacement to masquerade as a predicate inversion.
fn verify_mutant_operator_patch(operator: &str, patch: &MutantPatch) -> Result<()> {
    let before = patch.before.as_str();
    let after = patch.after.as_str();
    let inversion_pairs = [
        ("==", "!="),
        ("!=", "=="),
        ("<=", ">"),
        (">=", "<"),
        ("<", ">="),
        (">", "<="),
        ("&&", "||"),
        ("||", "&&"),
        ("|", "&"),
        ("true", "false"),
        ("false", "true"),
    ];
    let recognized = match operator {
        "predicate-inversion" => inversion_pairs.iter().any(|(from, to)| {
            before.contains(from)
                && after == before.replacen(from, to, 1)
                && before.matches(from).count() == 1
                && after.matches(to).count() >= 1
        }),
        "predicate-deletion" => before.contains('!') && after == before.replacen('!', "", 1),
        "threshold-plus-one" => integer_delta(before, after) == Some(1),
        "threshold-minus-one" => integer_delta(before, after) == Some(-1),
        "missing-enum-dispatch" => before.contains("match") && !after.contains("match"),
        "success-error-substitution" => {
            (before.contains("Ok(") && after.contains("Err("))
                || (before.contains("Err(") && after.contains("Ok("))
        }
        "oracle-short-circuit" => {
            ((before.contains("&&") || before.contains("||"))
                && (after.contains("return") || after.contains("Ok(")))
                // P1-M012's closed anchor; exact replacements only.
                || (before == "if id.is_some() {"
                    && matches!(after, "if true {" | "if false {"))
        }
        "ordering-nondeterminism" => {
            ((before.contains("sort") || before.contains("BTree"))
                && (!after.contains("sort") || after.contains("Hash")))
                // P1-M011's closed anchor; require a Hash collection.
                || (before.starts_with("let lines: Vec<")
                    && before != after
                    && after.contains("Hash")
                    && !after.contains("let lines: Vec<"))
        }
        "stale-basis-acceptance" => {
            (before.to_ascii_lowercase().contains("basis")
                && !after.to_ascii_lowercase().contains("basis"))
                || (before == "if rev == inner.state.head {"
                    && matches!(after, "if true {" | "if false {"))
        }
        // M17.5 F-34 / blind pass 1 A11: this matched bare WORDS, so deleting
        // the token from a label satisfied it. The anchored site is
        // `self.commit_intent(id, intent, &format!("ack:{step_id}"), origin)?;`
        // — renaming the label from `ack:` to anything else "skipped a durable
        // transition" while commit_intent still ran and persistence was intact.
        //
        // Skipping a transition means the CALL stops happening, so the markers
        // are call-shaped and the call must be gone from the after-text.
        // Blind pass 1 (2026-09-05) A10/A12: P1-M022 deletes `log.append(`
        // and P1-M035 deletes `.commit_if(`; neither call was on this list,
        // so both mutants were unevaluable while counting toward §3.
        "skipped-durable-transition" => DURABLE_CALL_MARKERS
            .iter()
            .any(|call| before.contains(call) && !after.contains(call)),
        "disabled-crash-point" => {
            (before.to_ascii_lowercase().contains("crash")
                && !after.to_ascii_lowercase().contains("crash"))
                // P1-M010 disables only its injected branch.
                || (before == "if fail_after.is_some_and(|limit| committed >= limit) {"
                    && after == "if false {")
        }
        "broadened-allow-list" => {
            (!before.contains('*') && after.contains('*'))
                // P1-M013: `matches!(byte, b'_' | b'-')` gains `| _`, admitting every byte.
                || (!before.contains("| _") && after.contains("| _"))
                || (!before.contains("|| true") && after.contains("|| true"))
                || (before.contains("ensure!") && !after.contains("ensure!"))
                // P1-M013 and P1-M026 are admitted by exact anchors below.
                || (before == "if id_str.is_empty()" && after == "if false")
                || (before == "match &aux.value {"
                    && before != after
                    && !after.contains("match")
                    && after.contains("if let"))
        }
        "wrong-holder-selection" => {
            // P1-M008: basis getter redirected to text; no generic self.*
            // escape. The anchors are `WRONG_HOLDER_ANCHORS`, which
            // `anchor_supports_operator` holds to as well, so a mutant cannot
            // declare this operator at a line no patch here could implement.
            WRONG_HOLDER_ANCHORS.contains(&before)
                && ((before == "&self.basis" && after == "&self.text")
                    || (before == "return Ok(inner.state.nodes.get(&id).cloned());"
                        && after == "return Ok(inner.state.relations.get(&id).cloned());"))
        }
        _ => false,
    };
    anyhow::ensure!(
        recognized,
        "mutation patch does not implement declared operator {operator:?}"
    );
    Ok(())
}

fn integer_delta(before: &str, after: &str) -> Option<i64> {
    fn scan(text: &str) -> Option<(Vec<String>, Vec<i64>)> {
        let mut chars = text.char_indices().peekable();
        let mut parts = Vec::new();
        let mut values = Vec::new();
        let mut cursor = 0;
        while let Some((index, ch)) = chars.next() {
            let starts_integer = ch.is_ascii_digit()
                || (ch == '-' && chars.peek().is_some_and(|(_, next)| next.is_ascii_digit()));
            if !starts_integer {
                continue;
            }
            let mut end = index + ch.len_utf8();
            while let Some((next_index, next)) = chars.peek().copied() {
                if !next.is_ascii_digit() {
                    break;
                }
                chars.next();
                end = next_index + next.len_utf8();
            }
            parts.push(text[cursor..index].to_owned());
            values.push(text[index..end].parse().ok()?);
            cursor = end;
        }
        parts.push(text[cursor..].to_owned());
        Some((parts, values))
    }

    if let Some(suffix) = after.strip_prefix(before).map(str::trim) {
        if suffix == "+ 1" || suffix == "+1" {
            return Some(1);
        }
        if suffix == "- 1" || suffix == "-1" {
            return Some(-1);
        }
    }
    let (before_parts, before_values) = scan(before)?;
    let (after_parts, after_values) = scan(after)?;
    if before_parts != after_parts || before_values.len() != after_values.len() {
        return None;
    }
    let mut delta = None;
    for (before, after) in before_values.into_iter().zip(after_values) {
        let change = after - before;
        if change == 0 {
            continue;
        }
        if delta.replace(change).is_some() || !matches!(change, 1 | -1) {
            return None;
        }
    }
    delta
}

/// Closed M24 mutation-source registry. Each predeclared mutant owns one
/// exact implementation coordinate; generated harness helpers are excluded.
#[allow(
    clippy::too_many_lines,
    reason = "closed mutation registry keeps every exact coordinate auditable in one place"
)]
fn mutant_source_coordinate(id: &str) -> Option<(&'static str, usize, &'static str)> {
    const SOURCE: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-format/src/lib.rs",
            122,
            "if !author_keyed {",
        ),
        (
            "crates/liminal-cst/src/parser.rs",
            221,
            "if delimiter_depth > MAX_NESTING && !nesting_reported {",
        ),
        (
            "crates/liminal-cst/src/parser.rs",
            206,
            "let suffix = &line[marker + 2..];",
        ),
        (
            "crates/liminal-cst/src/parser.rs",
            209,
            ".map_or(marker..line.len(), |close| marker..marker + 2 + close + 1)",
        ),
        ("crates/liminal-cst/src/parser.rs", 64, "match raw.0 {"),
        (
            "crates/liminal-source/src/view.rs",
            32,
            "return Err(SourceLoadError::TooLarge {",
        ),
        ("crates/liminal-source/src/view.rs", 51, "basis,"),
        ("crates/liminal-source/src/view.rs", 59, "&self.basis"),
        (
            "crates/liminal-cli/src/format.rs",
            73,
            "if let Err(error) = pending.commit_if(Some(ContentHash::of(&plan.original))) {",
        ),
        (
            "crates/liminal-cli/src/format.rs",
            55,
            "return Err(error).with_context(|| format!(\"stage {}\", plan.path));",
        ),
        (
            "crates/liminal-source/src/paragraph.rs",
            33,
            "let lines: Vec<(usize, &str)> = input.lines().enumerate().collect();",
        ),
        (
            "crates/liminal-source/src/paragraph.rs",
            76,
            "if id.is_some() {",
        ),
        (
            "crates/liminal-format/src/lib.rs",
            373,
            ".all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))",
        ),
    ];
    const GRAPH: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-graph/src/node.rs",
            58,
            "self.0 & other.0 == other.0",
        ),
        (
            "crates/liminal-graph/src/node.rs",
            64,
            "NodeFlags(self.0 | other.0)",
        ),
        (
            "crates/liminal-graph/src/relation.rs",
            76,
            "pub const TOMBSTONE: RelationFlags = RelationFlags(1);",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            100,
            "node.revision.0 += 1;",
        ),
        ("crates/liminal-graph/src/store/mod.rs", 81, "match op {"),
        (
            "crates/liminal-graph/src/store/mod.rs",
            302,
            "Ok(self.lock()?.state.head)",
        ),
        (
            "crates/liminal-graph/src/store/log.rs",
            42,
            "found.sort_unstable_by_key(|(n, _)| *n);",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            320,
            "return Ok(inner.state.nodes.get(&id).cloned());",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            475,
            "inner.log.append(&record)?;",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            122,
            "rel.revision.0 += 1;",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            160,
            "n.revision.0 += 1;",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            186,
            "match &aux.value {",
        ),
        (
            "crates/liminal-graph/src/store/mod.rs",
            84,
            "return Err(StoreError::Conflict(format!(\"node exists: {}\", node.id)));",
        ),
    ];
    const TRANSFORM: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-source/src/merge.rs",
            203,
            "if !ours_changed.is_disjoint(&theirs_changed) {",
        ),
        (
            "crates/liminal-source/src/merge.rs",
            70,
            "let ours_changed = ours_t != base_t;",
        ),
        ("crates/liminal-source/src/merge.rs", 158, "anon += 1;"),
        ("crates/liminal-source/src/merge.rs", 175, "anon += 1;"),
        (
            "crates/liminal-source/src/merge.rs",
            73,
            "let resolved: Option<&String> = match (ours_changed, theirs_changed) {",
        ),
        (
            "crates/liminal-source/src/file.rs",
            30,
            "Ok(bytes) => Ok(Some(FileObservation {",
        ),
        (
            "crates/liminal-source/src/view.rs",
            38,
            "if actual_hash != basis.content_hash {",
        ),
        (
            "crates/liminal-source/src/file.rs",
            147,
            "let hash = match fs::read(&path) {",
        ),
        (
            "crates/liminal-source/src/file.rs",
            92,
            "fs::rename(&self.staged, &self.target)?;",
        ),
        (
            "crates/liminal-source/src/paragraph.rs",
            141,
            "if !after_close.trim().is_empty() {",
        ),
        (
            "crates/liminal-source/src/merge.rs",
            45,
            "let mut seen: BTreeSet<String> = BTreeSet::new();",
        ),
        (
            "crates/liminal-source/src/merge.rs",
            131,
            "if c != b && m != c {",
        ),
        ("crates/liminal-source/src/paragraph.rs", 132, "|| !id_str"),
    ];
    const REPAIR: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-jurisdiction/src/repair.rs",
            219,
            "if !plan.steps.contains_key(&dep.before) || !plan.steps.contains_key(&dep.after) {",
        ),
        (
            "crates/liminal-jurisdiction/src/repair.rs",
            253,
            "if order.len() == plan.steps.len() {",
        ),
        (
            "crates/liminal-jurisdiction/src/checker.rs",
            567,
            "} else if evidences.len() == 1 {",
        ),
        (
            "crates/liminal-jurisdiction/src/checker.rs",
            426,
            "if aliases.len() > 1",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            178,
            "match self {",
        ),
        ("crates/liminal-jurisdiction/src/ilrp.rs", 278, "Ok(())"),
        (
            "crates/liminal-jurisdiction/src/checker.rs",
            288,
            "if !basis_ok && !chain_ok {",
        ),
        (
            "crates/liminal-jurisdiction/src/checker.rs",
            186,
            "if !node_grade.satisfies(req.minimum) {",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            291,
            "self.commit_intent(id, intent, &format!(\"ack:{step_id}\"), origin)?;",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            290,
            "self.crash.crash_if_armed(CrashPoint::BeforeAcknowledge);",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            310,
            "let graph_steps: std::collections::BTreeSet<RepairStepId> = plan",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            318,
            "if graph_steps.contains(&dep.before) && after_is_external {",
        ),
        (
            "crates/liminal-jurisdiction/src/ilrp.rs",
            226,
            "(*self).crash_if_armed(at);",
        ),
    ];
    const BASIS: [(&str, usize, &str); 13] = [
        (
            "crates/liminal-revision/src/inputs.rs",
            87,
            "return Err(PerspectiveError::NoFederationFrontier);",
        ),
        (
            "crates/liminal-revision/src/durability.rs",
            30,
            "BasisComponent::ObjectContent { .. } | BasisComponent::GitCommit { .. } => {",
        ),
        (
            "crates/liminal-revision/src/inputs.rs",
            58,
            "match selection.and_then(|m| m.get(key)) {",
        ),
        (
            "crates/liminal-revision/src/durability.rs",
            33,
            "BasisComponent::FileContent { .. } | BasisComponent::GraphSnapshot { .. } => {",
        ),
        (
            "crates/liminal-revision/src/durability.rs",
            29,
            "match component {",
        ),
        (
            "crates/liminal-revision/src/inputs.rs",
            70,
            "return Err(PerspectiveError::AmbiguousWorkingHolder {",
        ),
        (
            "crates/liminal-revision/src/durability.rs",
            36,
            "BasisComponent::BufferGeneration { .. }",
        ),
        (
            "crates/liminal-revision/src/durability.rs",
            37,
            "| BasisComponent::Observation { .. }",
        ),
        (
            "crates/liminal-revision/src/inputs.rs",
            83,
            "BasisPerspective::Published { .. } => {",
        ),
        (
            "crates/liminal-revision/src/inputs.rs",
            43,
            "let components = match perspective {",
        ),
        (
            "crates/liminal-revision/src/inputs.rs",
            84,
            "return Err(PerspectiveError::PublishedUnavailable);",
        ),
        (
            "crates/liminal-revision/src/inputs.rs",
            103,
            "BasisComponent::BufferGeneration { buffer, .. } if *buffer == selected",
        ),
        (
            "crates/liminal-revision/src/inputs.rs",
            46,
            "BasisPerspective::DurableOnly => inputs.durable.clone(),",
        ),
    ];
    let number = id.strip_prefix("P1-M")?.parse::<usize>().ok()?;
    let (table, offset) = match number {
        1..=13 => (&SOURCE, 1),
        14..=26 => (&GRAPH, 14),
        27..=39 => (&TRANSFORM, 27),
        40..=52 => (&REPAIR, 40),
        53..=65 => (&BASIS, 53),
        _ => return None,
    };
    table.get(number - offset).copied()
}

fn verify_canary_inventory(packet: &Packet) -> Result<()> {
    require_exact_ids(
        packet.canaries.iter().map(|row| row.id.as_str()),
        (1..=33).map(|idx| format!("C{idx:02}")),
        "canary",
    )?;
    for canary in &packet.canaries {
        if canary.expected_failure.trim().is_empty() {
            anyhow::bail!("{} has empty expected failure", canary.id);
        }
        let expected_gate = CANARY_GATES
            .iter()
            .find_map(|(id, gate)| (*id == canary.id.as_str()).then_some(*gate))
            .with_context(|| format!("{} has no closed gate mapping", canary.id))?;
        require_eq("canary gate", &canary.gate, expected_gate)?;
    }
    Ok(())
}

/// Closed canary-to-gate map. The 33 rows are not merely a count: each
/// ratification gate has one named, executable violation (M17.5 P1-A04).
const CANARY_GATES: [(&str, &str); 33] = [
    ("C01", "status"),
    ("C02", "ratification"),
    ("C03", "locked corpus"),
    ("C04", "requirements"),
    ("C05", "tests"),
    ("C06", "tests"),
    ("C07", "mutants"),
    ("C08", "mutants"),
    ("C09", "mutants"),
    ("C10", "canaries"),
    ("C11", "generated"),
    ("C12", "generated"),
    ("C13", "fuzz"),
    ("C14", "crash"),
    ("C15", "reviews"),
    ("C16", "markdown"),
    ("C17", "requirements"),
    ("C18", "evidence coverage"),
    ("C19", "stateful registry"),
    ("C20", "mutants"),
    ("C21", "mutants"),
    ("C22", "generated"),
    ("C23", "generated"),
    ("C24", "crash"),
    ("C25", "reviews"),
    ("C26", "provenance"),
    ("C27", "sanitizer"),
    ("C28", "oracle independence"),
    ("C29", "campaign clock"),
    ("C30", "residual risk"),
    ("C31", "corpus access"),
    ("C32", "concurrency"),
    ("C33", "crash boundary scope"),
];

#[allow(
    clippy::too_many_lines,
    reason = "generated inventory gate keeps closed family and relation checks together"
)]
fn verify_generated_inventory(packet: &Packet) -> Result<()> {
    if packet.generated.len() != 5 {
        anyhow::bail!(
            "generated family count must be 5, got {}",
            packet.generated.len()
        );
    }
    require_exact_ids(
        packet.generated.iter().map(|row| row.family.as_str()),
        GENERATED_FAMILIES.iter().map(|family| (*family).to_owned()),
        "generated family",
    )?;
    for family in &packet.generated {
        if family.accepted < 100_000 {
            anyhow::bail!("{} accepted cases below 100000", family.family);
        }
        // M17.5 F-30: `>` -> `>=` survives and is EQUIVALENT within the
        // reachable domain. Exactly 1% needs `accepted == 99 * discards`, and
        // with the >=100,000 accepted floor the nearest such point is not
        // expressible alongside the packet's own declared counts. The ceiling
        // is checked from both sides at the nearest reachable pair.
        let discard_percent = family
            .discards
            .checked_mul(100)
            .with_context(|| format!("{} discard rate arithmetic overflow", family.family))?;
        if discard_percent > family.attempts {
            anyhow::bail!("{} discard rate exceeds 1%", family.family);
        }
        if family.seed_categories.len() < 16 {
            anyhow::bail!("{} has fewer than 16 seed categories", family.family);
        }
        let mut categories = BTreeSet::new();
        for category in &family.seed_categories {
            anyhow::ensure!(
                !category.trim().is_empty(),
                "{} has an empty seed category",
                family.family
            );
            anyhow::ensure!(
                categories.insert(category),
                "{} repeats seed category {:?}",
                family.family,
                category
            );
        }
        let category_classes = family
            .seed_categories
            .iter()
            .map(|category| generated_category_class(category))
            .collect::<BTreeSet<_>>();
        let required_classes =
            BTreeSet::from(["valid", "boundary", "truncated", "malformed", "hostile"]);
        let missing_classes = required_classes
            .difference(&category_classes)
            .copied()
            .collect::<Vec<_>>();
        anyhow::ensure!(
            missing_classes.is_empty(),
            "{} seed categories miss typed classes {missing_classes:?}",
            family.family
        );
        require_exact_ids(
            family.seed_categories.iter().map(String::as_str),
            generated_seed_categories(family.family.as_str())
                .iter()
                .copied()
                .map(str::to_owned),
            "generated seed category registry",
        )?;
        if !family.relations.is_empty() {
            require_exact_ids(
                family.relations.iter().map(String::as_str),
                generated_relations(family.family.as_str())
                    .iter()
                    .copied()
                    .map(str::to_owned),
                "generated packet relation",
            )?;
            let oracle = family
                .oracle
                .as_ref()
                .with_context(|| format!("{} declares relations without oracle", family.family))?;
            anyhow::ensure!(
                !oracle.id.trim().is_empty() && !oracle.source.trim().is_empty(),
                "{} generated oracle declaration is incomplete",
                family.family
            );
        }
        // Internal consistency: the three counts must describe one run.
        let total = family
            .accepted
            .checked_add(family.discards)
            .and_then(|total| total.checked_add(family.negatives))
            .with_context(|| format!("{} count arithmetic overflow", family.family))?;
        if total != family.attempts {
            anyhow::bail!(
                "{}: accepted {} + discards {} + negatives {} != attempts {} — the counts do not \
                 describe a single run",
                family.family,
                family.accepted,
                family.discards,
                family.negatives,
                family.attempts
            );
        }
        // M17.5 F-04/F-07: a `pass` must be backed by a reproducible run, not
        // by numbers typed into the packet.
        if family.result == "pass" {
            if family.seed.is_none() {
                anyhow::bail!("{} claims pass without a recorded seed", family.family);
            }
            // M17.5 F-27 / P1-A04: this checked LENGTH only, so any 64
            // characters passed — including 64 spaces. A hash that is not
            // hexadecimal cannot be the digest of anything.
            if family
                .evidence_hash
                .as_ref()
                .is_none_or(|hash| hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()))
            {
                anyhow::bail!(
                    "{} claims pass without a 64-hex evidence hash",
                    family.family
                );
            }
        }
    }
    // The fuzz budget is no longer checkable here: it is DERIVED from the
    // committed campaign by `verify_fuzz_budget` at the qualified layer. What
    // the inventory layer can still check is the mapping's shape.
    verify_fuzz_target_mapping(&packet.generated)
}

/// The family→target mapping must be a partition, not an overlap
/// (M17.5 F-17, F-29).
///
/// ADR-0020 §4 gives each family its own 30-minute campaign. If one target
/// evidenced two families, a single campaign would satisfy two budgets and both
/// would pass on half the work — so a target may be claimed at most once.
///
/// Empty is allowed HERE and refused at the qualified layer. The inventory
/// layer describes a plan, and a family whose fuzz target has not been written
/// yet is a legitimate plan; claiming `pass` for it is not.
fn verify_fuzz_target_mapping(declared: &[Generated]) -> Result<()> {
    let mut claimed: BTreeMap<&str, &str> = BTreeMap::new();
    for family in declared {
        for target in &family.fuzz_targets {
            if let Some(other) = claimed.insert(target.as_str(), family.family.as_str()) {
                anyhow::bail!(
                    "fuzz target {target:?} is claimed by both {other:?} and {:?}; \
                     one campaign cannot satisfy two families' budgets",
                    family.family
                );
            }
        }
    }
    Ok(())
}

/// Derive each family's fuzz minutes from the committed campaign and enforce
/// ADR-0020 §4's floors against the DERIVED value (M17.5 F-17).
///
/// The packet says which targets belong to each family; the artifact supplies
/// their seconds. Summing those rows is the sole family-budget calculation.
fn verify_fuzz_budget(declared: &[Generated], recorded: &[FuzzEvidence]) -> Result<()> {
    let elapsed_seconds: BTreeMap<&str, u64> = recorded
        .iter()
        .map(|row| (row.target.as_str(), row.elapsed_s))
        .collect();
    let mut total_minutes = 0_u64;
    for family in declared {
        if family.result != "pass" {
            continue;
        }
        if family.fuzz_targets.is_empty() {
            anyhow::bail!(
                "{} claims pass but names no fuzz target; ADR-0020 §4 gives every \
                 family its own sanitizer campaign, and a family with no target \
                 has had none",
                family.family
            );
        }
        let mut family_minutes = 0_u64;
        for target in &family.fuzz_targets {
            let recorded_seconds = elapsed_seconds.get(target.as_str()).with_context(|| {
                format!(
                    "{} names fuzz target {target:?}, which the committed campaign did not run",
                    family.family
                )
            })?;
            family_minutes += recorded_seconds / 60;
        }
        // ADR-0020 line 90: one 30-minute sanitizer campaign per family.
        if family_minutes < 30 {
            anyhow::bail!(
                "{}: its targets {:?} recorded {family_minutes} minutes, below the 30 \
                 ADR-0020 §4 requires per family",
                family.family,
                family.fuzz_targets
            );
        }
        total_minutes += family_minutes;
    }
    // ADR-0020 line 92: "Total fuzz budget is at least 150 target-minutes."
    if total_minutes < 150 {
        anyhow::bail!("derived fuzz budget {total_minutes} target-minutes is below 150");
    }
    Ok(())
}

fn verify_reviews_inventory(packet: &Packet) -> Result<()> {
    if packet.reviews.len() != 2 {
        anyhow::bail!(
            "review record count must be 2, got {}",
            packet.reviews.len()
        );
    }
    require_unique(
        packet.reviews.iter().map(|row| row.reviewer.as_str()),
        "reviewer",
    )?;
    for review in &packet.reviews {
        if review.attempts < 12 {
            anyhow::bail!("{} has fewer than 12 attempts", review.reviewer);
        }
        if review.unresolved_verified_findings != 0 {
            anyhow::bail!("{} has unresolved verified findings", review.reviewer);
        }
    }
    Ok(())
}

fn verify_markdown_surface(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("docs/execution/phase1-suite-review.md");
    let text = fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
    verify_markdown_surface_text(&text, packet)
}

/// Read one `key: value` line from the packet markdown's frontmatter.
///
/// M17.5 F-33 / blind pass 1 A08: the status was checked with
/// `text.contains("status: proposed")`, so the frontmatter could read
/// `status: ratified` and the check still passed as long as the literal
/// survived ANYWHERE in the file — one sentence of prose is enough. The header
/// is a declaration, not a substring, so it is parsed as one.
fn markdown_header<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .take_while(|line| !line.starts_with('#') || line.starts_with("# "))
        .find_map(|line| line.strip_prefix(&format!("{key}: ")))
        .map(str::trim)
}

#[allow(
    clippy::too_many_lines,
    reason = "one check per declared header and table claim, in document order"
)]
fn verify_markdown_surface_text(text: &str, packet: &Packet) -> Result<()> {
    let status = markdown_header(text, "status")
        .context("review packet markdown has no `status:` header")?;
    require_eq("review packet markdown status", status, "proposed")?;
    // Blind pass 1 at `0a0b4b4b` (A09): this searched the WHOLE document for
    // the row, so the phrase in a paragraph, a second authority table, or a
    // row under any other header satisfied it. The authority table is one
    // table and its rows are read from it.
    // `| Field | Value |` is a generic header this document spells five times,
    // so the authority table is found by its section instead.
    let heading = "## Packet authority and bounds";
    let section = text
        .find(heading)
        .with_context(|| format!("review packet markdown has no {heading:?} section"))?;
    let authority = markdown_tables(&text[section..], "| Field | Value |");
    let authority = authority
        .first()
        .with_context(|| format!("{heading:?} renders no table"))?;
    let row_value = |field: &str| -> Option<String> {
        authority
            .1
            .iter()
            .find(|row| row.first().is_some_and(|cell| cell.trim() == field))
            .and_then(|row| row.get(1))
            .map(|cell| (*cell).trim().to_owned())
    };
    let ratification = row_value("ratification decision")
        .context("the authority table has no ratification decision row")?;
    anyhow::ensure!(
        ratification.starts_with("unratified"),
        "the authority table records ratification {ratification:?}; the packet is unratified"
    );
    // Blind pass 1 at `ec045588` (A10): the qualification lane's table renders
    // the commands, hashes and toolchain a run would record, and no rendered
    // table or authority row bound it — a fabricated command inventory sat
    // there through the packet digest and every status check. None of it is a
    // verdict word, so the document-wide verdict scan never saw it either.
    // While the packet is unqualified every one of those cells is a value the
    // lane has not produced.
    let lane_heading = "### Qualification lane";
    let lane = text
        .find(lane_heading)
        .with_context(|| format!("review packet markdown has no {lane_heading:?} section"))?;
    let lane_table = markdown_tables(&text[lane..], "| Field | Value |");
    let lane_table = lane_table
        .first()
        .with_context(|| format!("{lane_heading:?} renders no table"))?;
    anyhow::ensure!(
        !lane_table.1.is_empty(),
        "{lane_heading:?} renders an empty table"
    );
    for row in &lane_table.1 {
        for cell in row.iter().skip(1) {
            let value = cell.trim().trim_matches('`').trim();
            anyhow::ensure!(
                cell_claims_nothing(value),
                "the qualification lane records {value:?} while the packet is unqualified; only \
                 the flip may write what a run produced"
            );
        }
    }
    let canary_claim = format!(
        "Target: exactly **{} predeclared canaries**",
        packet.canaries.len()
    );
    anyhow::ensure!(
        text.contains(&canary_claim),
        "review packet markdown canary target disagrees with packet count {}",
        packet.canaries.len()
    );
    let expected_state = packet
        .qualification_state
        .replace('-', "_")
        .to_ascii_uppercase();
    let state_marker = format!("| qualification state | {expected_state} |");
    if !text.contains(&state_marker) {
        anyhow::bail!(
            "review packet markdown qualification state disagrees with packet {:?}",
            packet.qualification_state
        );
    }
    // M17.5 F-33 / blind pass 1 A09: `fixed_review_base` appeared ONLY in docs
    // — no Rust file read it. The markdown declared which tree was reviewed and
    // nothing bound that declaration to the tree the packet points at, so the
    // two could name different commits and every check still passed.
    let declared_base = markdown_header(text, "fixed_review_base")
        .context("review packet markdown has no `fixed_review_base:` header")?;
    match packet.provenance.as_ref() {
        Some(provenance) => require_eq(
            "review packet markdown fixed_review_base",
            declared_base,
            &provenance.fixed_commit,
        )?,
        // Before qualification there is no provenance to bind to, and the
        // placeholder is the honest value — but it may not be a plausible
        // commit, or a reader cannot tell a predeclared packet from a
        // qualified one.
        None => anyhow::ensure!(
            declared_base == "NOT_RUN",
            "review packet markdown declares fixed_review_base {declared_base:?} \
             while the packet carries no provenance; an unqualified packet must \
             say NOT_RUN"
        ),
    }

    if packet.qualification_state != "not-run" && text.contains("| NOT_RUN |") {
        anyhow::bail!(
            "review packet markdown retains NOT_RUN placeholders for qualification state {:?}",
            packet.qualification_state
        );
    }
    let conclusion = text
        .lines()
        .find(|line| line.starts_with("Current conclusion:"))
        .unwrap_or_default();
    match packet.qualification_state.as_str() {
        "not-run" => anyhow::ensure!(
            conclusion == "Current conclusion: `NOT_RUN`. Packet remains proposed and unqualified.",
            "review packet markdown conclusion disagrees with qualification state not-run: {conclusion:?}"
        ),
        "complete" => anyhow::ensure!(
            conclusion.contains("QUALIFIED"),
            "review packet markdown conclusion disagrees with qualification state complete: {conclusion:?}"
        ),
        state => anyhow::bail!(
            "review packet markdown conclusion has no closed qualification state {state:?}"
        ),
    }
    let digest = blake3::hash(
        serde_json::to_vec(packet)
            .context("serialize packet for markdown digest")?
            .as_slice(),
    )
    .to_hex()
    .to_string();
    if !text.contains(&digest) {
        anyhow::bail!("review packet markdown missing packet digest {digest}");
    }
    let status_tuple = format!(
        "| machine status tuple | qualification_state={}; qualification_stage={}; requirements={}; tests={}; mutants={}; canaries={}; generated={}; crash_boundaries={}; reviews={} |",
        packet.qualification_state,
        packet.qualification_stage,
        packet.requirements.len(),
        packet.tests.len(),
        packet.mutants.len(),
        packet.canaries.len(),
        packet.generated.len(),
        packet.crash_boundaries.len(),
        packet.reviews.len(),
    );
    anyhow::ensure!(
        text.contains(&status_tuple),
        "review packet markdown machine status tuple disagrees with packet"
    );
    let markdown_canaries = text
        .lines()
        .filter_map(|line| line.split('|').nth(1).map(str::trim))
        .filter(|id| id.starts_with('C') && id[1..].chars().all(|ch| ch.is_ascii_digit()))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let packet_canaries = packet
        .canaries
        .iter()
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        markdown_canaries == packet_canaries,
        "review packet markdown canary rows differ: markdown={markdown_canaries:?}, packet={packet_canaries:?}"
    );
    Ok(())
}

/// Concurrent implementations require a committed schedule matrix. The
/// packet's implementation label is not a schedule result; each sequence must
/// carry an independent oracle digest and a passing replay outcome.
fn verify_concurrency_evidence(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/concurrency.json");
    let evidence: ConcurrentEvidence = serde_json::from_slice(
        &fs::read(&path)
            .with_context(|| format!("{path}: concurrent schedule evidence required"))?,
    )
    .with_context(|| format!("parse {path}"))?;
    verify_concurrency_evidence_record(root, packet, &evidence)
}

/// A05: the not-applicable label is checked against the implementation, not
/// trusted from the packet. Concurrent execution refuses; the recorded
/// synchronization primitives must be the ones the tree has.
fn verify_not_applicable_against_implementation(
    root: &Utf8Path,
    evidence: &ConcurrentEvidence,
) -> Result<()> {
    let scan = scan_concurrency_primitives(root)?;
    anyhow::ensure!(
        scan.execution.is_empty(),
        "concurrent_code is not_applicable, but the implementation runs concurrently: {:?}",
        scan.execution
    );
    require_eq(
        "concurrency sync_primitives",
        &evidence.sync_primitives.join("\n"),
        &scan.sync.join("\n"),
    )
}

/// The not-applicable branch: no schedules, the reserved-phase reason, the
/// declared anchor, and a coordinate bound to this qualification's commit.
fn verify_not_applicable_concurrency(
    root: &Utf8Path,
    packet: &Packet,
    evidence: &ConcurrentEvidence,
) -> Result<()> {
    require_eq("concurrency mode", &evidence.mode, "not_applicable")?;
    require_eq(
        "concurrency source_anchor",
        &evidence.source_anchor,
        "docs/execution/M21.md:13",
    )?;
    anyhow::ensure!(
        evidence.schedules.is_empty(),
        "not-applicable concurrency evidence must contain no schedules"
    );
    verify_not_applicable_against_implementation(root, evidence)?;
    anyhow::ensure!(
        evidence.reason.contains("reserved for Phase 6")
            && evidence.reason.contains("no concurrent implementation"),
        "not-applicable concurrency evidence must state why schedule exploration is absent"
    );
    verify_git_coordinate(
        root,
        &evidence.source_commit,
        &evidence.source_tree,
        &evidence.source_anchor,
        "concurrency",
    )?;
    // Blind pass 1 at ee646f8c (A12): the coordinate was checked to exist
    // and never checked to be THIS qualification's. The applicable branch
    // below binds it to the packet; not-applicable evidence returned
    // first, which is how F-40's stale August source_commit rode into
    // every later lane.
    if let Some(provenance) = &packet.provenance {
        require_eq(
            "concurrency source_commit",
            &evidence.source_commit,
            &provenance.fixed_commit,
        )?;
    }
    Ok(())
}

fn verify_concurrency_evidence_record(
    root: &Utf8Path,
    packet: &Packet,
    evidence: &ConcurrentEvidence,
) -> Result<()> {
    let applicable = packet.concurrent_code == "deterministic_schedule_exploration";
    require_eq(
        "concurrency schema_version",
        &evidence.schema_version,
        "haqp-concurrency-v1",
    )?;
    require_eq(
        "concurrency implementation",
        &evidence.implementation,
        &packet.concurrent_code,
    )?;
    if !applicable {
        return verify_not_applicable_concurrency(root, packet, evidence);
    }
    require_eq("concurrency mode", &evidence.mode, "executed")?;
    anyhow::ensure!(
        !evidence.runner.trim().is_empty(),
        "concurrency evidence has no executable runner"
    );
    require_eq(
        "concurrency oracle",
        &evidence.oracle_id,
        "independent_schedule_oracle_v1",
    )?;
    require_git_object_id("concurrency source_commit", &evidence.source_commit)?;
    require_git_object_id("concurrency source_tree", &evidence.source_tree)?;
    if let Some(provenance) = &packet.provenance {
        require_eq(
            "concurrency source_commit/provenance",
            &evidence.source_commit,
            &provenance.fixed_commit,
        )?;
        require_eq(
            "concurrency source_tree/provenance",
            &evidence.source_tree,
            &provenance.fixed_tree,
        )?;
    }
    anyhow::ensure!(
        !evidence.schedules.is_empty(),
        "concurrency evidence has no schedules"
    );
    require_unique(
        evidence.schedules.iter().map(|row| row.id.as_str()),
        "concurrency schedule",
    )?;
    for schedule in &evidence.schedules {
        anyhow::ensure!(
            schedule.sequence.len() >= 2,
            "concurrency schedule {} has fewer than two operations",
            schedule.id
        );
        anyhow::ensure!(
            schedule.sequence.iter().all(|step| !step.trim().is_empty()),
            "concurrency schedule {} has empty operation",
            schedule.id
        );
        require_eq("concurrency schedule result", &schedule.result, "pass")?;
        let expected = blake3::hash(
            serde_json::to_vec(&(
                schedule.id.as_str(),
                &schedule.sequence,
                schedule.result.as_str(),
            ))?
            .as_slice(),
        )
        .to_hex()
        .to_string();
        require_eq(
            "concurrency schedule oracle_digest",
            &schedule.oracle_digest,
            &expected,
        )?;
    }
    Ok(())
}

/// Bind evidence source coordinates to an actual committed Git tree. Shape
/// checks alone let not-applicable rows carry arbitrary commit/tree claims.
fn verify_git_coordinate(
    root: &Utf8Path,
    commit: &str,
    tree: &str,
    coordinate: &str,
    label: &str,
) -> Result<()> {
    require_git_object_id(&format!("{label} source_commit"), commit)?;
    require_git_object_id(&format!("{label} source_tree"), tree)?;
    let expected_tree = git_text(root, &["rev-parse", &format!("{commit}^{{tree}}")])?;
    anyhow::ensure!(
        tree == expected_tree,
        "{label} source provenance does not match committed tree"
    );
    let (file, line) = coordinate
        .rsplit_once(':')
        .with_context(|| format!("{label} source_anchor must be file:line"))?;
    let line: usize = line
        .parse()
        .with_context(|| format!("{label} source_anchor line is not numeric"))?;
    anyhow::ensure!(
        line > 0
            && !file.is_empty()
            && !file.starts_with('/')
            && !file.split('/').any(|part| part == "..")
            && !is_locked_acceptance_path(file),
        "{label} source_anchor is unsafe"
    );
    let source = git_text(root, &["show", &format!("{commit}:{file}")])?;
    anyhow::ensure!(
        source.lines().nth(line - 1).is_some(),
        "{label} source_anchor line {line} is absent from committed source"
    );
    Ok(())
}

fn verify_crash_boundary_inventory(packet: &Packet) -> Result<()> {
    // Closed authority is intentional: runtime and packet declarations must
    // both be updated when a new durable transition is introduced.
    const AUTHORITATIVE_CRASH_BOUNDARIES: [&str; 8] = [
        "ilrp/before_intent_commit",
        "ilrp/after_intent_commit",
        "ilrp/before_external_apply",
        "ilrp/after_external_apply",
        "ilrp/before_ack",
        "ilrp/after_ack",
        "ilrp/before_finalize",
        "ilrp/after_finalize_before_notify",
    ];
    let authoritative = AUTHORITATIVE_CRASH_BOUNDARIES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let registered = liminal_jurisdiction::CrashPoint::all()
        .iter()
        .map(|point| point.name().to_owned())
        .collect::<BTreeSet<_>>();
    let declared = packet
        .crash_boundaries
        .iter()
        .map(|row| row.boundary.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    if declared != registered {
        anyhow::bail!(
            "crash-boundary inventory mismatch: declared={declared:?}, registered={registered:?}"
        );
    }
    anyhow::ensure!(
        registered == authoritative,
        "runtime crash-boundary registry differs from closed authoritative registry: runtime={registered:?}, authoritative={authoritative:?}"
    );
    for row in &packet.crash_boundaries {
        if !row.before || !row.after {
            anyhow::bail!(
                "{} missing before/after injection declaration",
                row.boundary
            );
        }
    }
    Ok(())
}

/// Durable-transition call sites in tracked runtime source, by path.
///
/// Markers are call-shaped for the reason F-34 established for the
/// `skipped-durable-transition` operator: a durable transition is a CALL, and
/// matching bare words lets a rename satisfy the check.
///
/// Enumerated from `git ls-files`, never the filesystem (F-32): a scan that
/// reads the working directory reports a different durable surface on a dirty
/// tree than a fresh clone does, and the clone is the one that has to reproduce
/// this.
///
/// `liminal-xtask` is excluded — the verifier is not runtime code, and its
/// operator table contains the marker as a string literal, so including it
/// would make this count self-referential. `tests/` is excluded: an integration
/// test's commit is not a transition the product performs.
///
/// This is a TRIPWIRE, not a semantic census. It counts call sites textually,
/// including any inside `#[cfg(test)]` modules, and its only job is to fail
/// loudly when the durable surface moves under a disclosure that claims to have
/// surveyed it.
///
/// Known imprecision, and its direction: line comments (`//`, and `///` with
/// them) are skipped, but a marker inside a block comment or a string literal
/// still counts. That errs toward REJECTION — a spurious match fails a packet
/// that was honest, which a human then corrects. The opposite error, silently
/// undercounting, would let a real durable transition hide, so the bias is the
/// one worth having.
/// How many times `line` names one of `symbols` as a durable transition.
///
/// Blind pass 1 at 5fb1b57 (A05): matching `symbol(` missed a turbofish
/// (`rename::<T>(`) and a function pointer (`let f = fs::rename;`). The symbol
/// is matched on a word boundary and accepted when what follows is a call, a
/// path or turbofish, or the end of a value — never when it is more
/// identifier, so `rename_all` and `append_str` are not renames and appends.
/// Counting occurrences rather than marker spellings keeps one call site worth
/// one site however many spellings would match it.
fn durable_symbol_hits(line: &str, symbols: &[String]) -> usize {
    let identifier = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut hits = 0;
    for symbol in symbols {
        for (offset, _) in line.match_indices(symbol.as_str()) {
            if line[..offset].chars().next_back().is_some_and(identifier) {
                continue;
            }
            let rest = &line[offset + symbol.len()..];
            let follows = rest.starts_with('(')
                || rest.starts_with("::")
                || rest.starts_with(';')
                || rest.starts_with(',')
                || rest.starts_with(')');
            if follows {
                hits += 1;
            }
        }
    }
    hits
}

fn durable_transition_sites(root: &Utf8Path) -> Result<BTreeMap<String, usize>> {
    // Blind pass 1 at 612cbcc (A05): `inner.log.append(` is the store's durable
    // append (P1-M022 deletes it) and was outside the census, so an append-based
    // transition could move without the scope disclosure noticing.
    // Blind pass 1 at f360e90 (A05): a direct `fs::rename` or `sync_all` is a
    // durable transition with no marker call around it, so it escaped.
    // Blind pass 1 at aa00d41 (A05): `use std::fs::rename as mv; mv(a, b)`
    // named no marker. The durable symbols are matched by name, and a `use`
    // that renames one adds its alias for that file.
    const DURABLE_SYMBOLS: [&str; 6] = [
        "commit",
        "commit_if",
        "append",
        "sync_all",
        "sync_data",
        "rename",
    ];
    let listed = git_text(root, &["ls-files", "-z", "--", "crates"])?;
    let mut sites = BTreeMap::new();
    for name in listed.split('\0').filter(|name| !name.trim().is_empty()) {
        if Utf8Path::new(name).extension() != Some("rs")
            || name.starts_with("crates/liminal-xtask/")
            || name.contains("/tests/")
        {
            continue;
        }
        let path = root.join(name);
        // Never skip a tracked file we cannot read. Dropping it would delete
        // its transitions from the measured surface, and the disclosure would
        // then pass by describing a tree nobody managed to look at — the exact
        // vacuity this check exists to prevent.
        let text = fs::read_to_string(&path).with_context(|| {
            format!(
                "{name} is tracked but unreadable; the durable surface cannot be measured, and \
                 a scope claim over a surface that was not read is worth nothing"
            )
        })?;
        // A `use` that renames a durable symbol makes the alias a marker in
        // this file: `use std::fs::rename as mv;` means `mv` is a rename.
        let mut markers = DURABLE_SYMBOLS
            .iter()
            .map(|symbol| (*symbol).to_owned())
            .collect::<Vec<_>>();
        for line in text.lines() {
            for (symbol, alias) in use_aliases(line) {
                if DURABLE_SYMBOLS.contains(&symbol.as_str()) {
                    markers.push(alias);
                }
            }
        }
        let count = text
            .lines()
            // An import declares a name; it performs no transition. Without
            // this the aliasing `use` line counts as a durable call itself.
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && !trimmed.starts_with("use ")
            })
            .map(|line| durable_symbol_hits(line, &markers))
            .sum::<usize>();
        if count > 0 {
            sites.insert(name.to_owned(), count);
        }
    }
    Ok(sites)
}

/// The half of the scope claim that reads only the packet, returning the
/// declared surface keyed by path.
///
/// Split from the source-measured half deliberately: `verify_inventory_repo` is
/// exercised against synthetic trees that hold a packet and nothing else, and a
/// check must read only inputs its caller actually has. Measuring source there
/// would either fail on a tree that is not at fault or — the worse outcome —
/// invite a "skip when there is no source" branch, which is how a gate silently
/// degrades into passing everything.
fn verify_crash_boundary_scope_shape(packet: &Packet) -> Result<BTreeMap<&str, usize>> {
    let scope = &packet.crash_boundary_scope;
    anyhow::ensure!(
        !scope.in_scope.is_empty(),
        "crash_boundary_scope.in_scope is empty: §5 exhaustiveness must be claimed over at \
         least one subsystem, or the packet claims nothing"
    );
    anyhow::ensure!(
        !scope.deferred_to.trim().is_empty(),
        "crash_boundary_scope.deferred_to must name who closes the deferral"
    );
    anyhow::ensure!(
        !scope.rationale.trim().is_empty(),
        "crash_boundary_scope.rationale must state why the claim is bounded"
    );

    // A registered boundary outside every declared subsystem means the packet
    // registers boundaries it has not claimed scope over.
    for row in &packet.crash_boundaries {
        let subsystem = row.boundary.split('/').next().unwrap_or_default();
        anyhow::ensure!(
            scope
                .in_scope
                .iter()
                .any(|entry| entry.subsystem == subsystem),
            "crash boundary {} is outside every declared in-scope subsystem",
            row.boundary
        );
    }

    let mut declared: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in &scope.in_scope {
        // A blank subsystem would match no boundary while still reading as a
        // claim; a blank path would name no surface. Checked on this side too,
        // not only on `deferred`, so the two halves cannot drift apart.
        anyhow::ensure!(
            !entry.subsystem.trim().is_empty() && !entry.path.trim().is_empty(),
            "an in-scope subsystem must name both a boundary prefix and a path"
        );
        anyhow::ensure!(
            declared.insert(entry.path.as_str(), entry.sites).is_none(),
            "{} is declared twice in crash_boundary_scope",
            entry.path
        );
    }
    for entry in &scope.deferred {
        anyhow::ensure!(
            !entry.reason.trim().is_empty(),
            "{}: a deferred durable surface must say why it is deferred",
            entry.path
        );
        anyhow::ensure!(
            declared.insert(entry.path.as_str(), entry.sites).is_none(),
            "{} is declared both in scope and deferred",
            entry.path
        );
    }
    Ok(declared)
}

/// Hold the §5 exhaustiveness claim to the surface it actually covers.
///
/// Ruling of 2026-08-26 (M17.5 A05): for HAQP-1a, exhaustiveness is claimed
/// over ILRP alone, every other durable transition is deferred to M17.9, and
/// the split is declared in the packet rather than left implicit. Deciding
/// which of the runtime's transitions are *registrable* boundaries is runtime
/// design work; doing it here would either block 1a on Phase 1 or pass by
/// picking a convenient definition, and ADR-0020 §3 forbids improvising
/// qualification semantics.
///
/// What is enforced instead is that the disclosure stays true. A prose scope
/// note rots the first time somebody adds a commit site — which is exactly the
/// defect A05 found in the original unscoped claim — so every durable
/// transition in tracked source must fall inside a declared in-scope subsystem
/// or a declared deferred surface, with counts matching exactly. A new
/// `txn.commit()` anywhere turns the packet red until it is either registered
/// or disclosed.
fn verify_crash_boundary_scope(root: &Utf8Path, packet: &Packet) -> Result<()> {
    let declared = verify_crash_boundary_scope_shape(packet)?;
    let measured = durable_transition_sites(root)?;
    for (path, count) in &measured {
        let Some(expected) = declared.get(path.as_str()) else {
            anyhow::bail!(
                "{path} performs {count} durable transition(s) and is in neither the in-scope \
                 nor the deferred surface: ADR-0020 §5 exhaustiveness cannot be claimed over a \
                 surface the packet has not surveyed. Register its boundaries or disclose it."
            );
        };
        anyhow::ensure!(
            expected == count,
            "{path}: crash_boundary_scope declares {expected} durable transition(s), source has \
             {count}. The disclosure is stale — a durable surface moved under a claim that says \
             it was surveyed."
        );
    }
    for path in declared.keys() {
        anyhow::ensure!(
            measured.contains_key(*path),
            "{path}: crash_boundary_scope declares durable transitions that no longer exist in \
             tracked source"
        );
    }
    Ok(())
}

fn require_eq(field: &str, actual: &str, expected: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        anyhow::bail!("{field}: expected {expected:?}, got {actual:?}")
    }
}

fn require_unique<'a>(ids: impl Iterator<Item = &'a str>, label: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.to_owned()) {
            anyhow::bail!("duplicate {label} id {id}");
        }
    }
    Ok(())
}

fn require_exact_ids<'a>(
    actual: impl Iterator<Item = &'a str>,
    expected: impl Iterator<Item = String>,
    label: &str,
) -> Result<()> {
    let actual = actual.map(ToOwned::to_owned).collect::<BTreeSet<_>>();
    let expected = expected.collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        anyhow::bail!("{label} ids differ: actual={actual:?}, expected={expected:?}")
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    suite_version: String,
    status: String,
    ratification: String,
    qualification_state: String,
    /// Which HAQP stage this packet claims (ADR-0021).
    ///
    /// `1a` is everything Phase 0 can prove; `1b` adds ADR-0020 §3's mutation
    /// requirement and runs at M24, when M18–M23 have written the tests that
    /// witness a kill. Required, not defaulted: a packet that did not say which
    /// stage it completed would let a reader assume the stronger one.
    qualification_stage: String,
    /// ADR-0020 §4 requires deterministic schedule evidence when concurrent
    /// code exists, and an explicit N/A record otherwise. This is required
    /// rather than defaulted so omission cannot masquerade as N/A.
    concurrent_code: String,
    locked_acceptance_corpora_touched: bool,
    requirements: Vec<Requirement>,
    tests: Vec<Test>,
    mutants: Vec<Mutant>,
    canaries: Vec<Canary>,
    generated: Vec<Generated>,
    crash_boundaries: Vec<CrashBoundary>,
    /// What ADR-0020 §5's "exhaustive" is exhaustive OVER (M17.5 A05).
    ///
    /// Required rather than defaulted, for the same reason as
    /// `qualification_stage`: a packet that did not say which durable surface
    /// it claims would let a reader assume all of them.
    crash_boundary_scope: CrashBoundaryScope,
    reviews: Vec<Review>,
    /// Residual-risk coordinates are populated at qualification time; the
    /// proposed inventory may leave this empty while no campaign has run.
    #[serde(default)]
    residual_risks: Vec<ResidualRisk>,
    /// ADR-0020 §1 fixed-base binding. Absent until the qualification lane
    /// runs from one clean tree.
    #[serde(default)]
    provenance: Option<Provenance>,
}

/// The fixed base a qualification run was taken from.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    /// Fixed clean source/evidence commit. The metadata packet is committed as
    /// its child, avoiding impossible self-referential commit hashing.
    commit: String,
    lockfile_blake3: String,
    /// Clean source commit the evidence ran against; must be HEAD's parent.
    fixed_commit: String,
    fixed_tree: String,
    evidence_parent: String,
}

/// One target's recorded fuzz campaign (committed artifact).
///
/// `deny_unknown_fields` is load-bearing (M17.5 pass-2 #17). The committed
/// artifact already carried `elapsed_s`, `seed` and `log`, written by
/// `scripts/haqp_fuzz_campaign.sh`, and this struct silently dropped all three:
/// the campaign recorded its own reproduction seed and the verifier threw it
/// away. Anything the campaign records must now either be checked here or fail
/// to parse.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct FuzzEvidence {
    target: String,
    /// Budget the campaign was asked for (`-max_total_time`).
    seconds: u64,
    /// Wall-clock the campaign actually took. A target that exits early spent
    /// less than its budget, which is how a crash shows up in the timings.
    elapsed_s: u64,
    exit_code: i32,
    execs: u64,
    artifacts: u64,
    /// Fixed so the campaign is reproducible (ADR-0020 §1).
    seed: u64,
    /// Number of committed, non-locked seed inputs supplied to this target.
    /// A scalar libFuzzer seed does not prove D23.2's declared seed set.
    #[serde(default)]
    seed_count: u64,
    /// BLAKE3 over sorted seed names and per-input SHA-256 digests, binding
    /// the campaign row to the exact committed input set rather than only to
    /// one RNG seed.
    #[serde(default)]
    seed_manifest_blake3: String,
    /// Sanitizer the campaign was built with (ADR-0020 §4 requires a
    /// sanitizer-enabled campaign; M17.5 F-27 / P1-A09).
    ///
    /// This binds the packet to the campaign's CONFIGURATION, not to proof the
    /// binary was instrumented — a clean libFuzzer run emits no sanitizer
    /// marker to grep for, so there is nothing stronger available from the
    /// artifact. Recorded as a known limit rather than dressed up.
    sanitizer: String,
    /// Path the campaign wrote its libFuzzer log to.
    log: String,
    /// BLAKE3 of that log (M17.5 F-17).
    ///
    /// Defaulted so a pre-F-17 artifact still parses at the inventory layer;
    /// the qualified layer requires 64 hex, so an empty digest cannot reach a
    /// `complete` packet. Populating it needs a fresh campaign — the digest of
    /// a log nobody kept cannot be reconstructed.
    ///
    /// `log` names a file under `target/`, which is gitignored and therefore
    /// guaranteed absent by the time anyone verifies — so execs, timings and
    /// exit codes were unfalsifiable in the committed state. The digest makes a
    /// later-produced log checkable without committing megabytes of libFuzzer
    /// output; `--keep-logs` commits them when someone actually wants them.
    #[serde(default)]
    log_blake3: String,
    /// Build and runtime proof that the recorded campaign used an
    /// instrumented binary, not merely a sanitizer label in JSON.
    #[serde(default)]
    sanitizer_proof: Option<SanitizerProof>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SanitizerProof {
    build_command: String,
    // `runtime_probe` / `runtime_probe_blake3` bind the COMMITTED probe log by
    // digest (checked below). The replay re-runs the probe only as a runtime
    // identity check: its output carries ASLR addresses ("Loaded 1 modules ...
    // [0x5633...]") and so cannot be compared byte-for-byte to the recorded log
    // without inventing a normalization -- which would be a new check that
    // means less than it says (HEAD review, F-42).
    /// The canonical absolute path the binary was built at (M17.5 F-42). cargo's
    /// `-C metadata` hash includes a path dependency's absolute manifest path
    /// and ASan embeds the resulting codegen-unit name in .rodata, so a rebuild
    /// only reproduces the committed bytes at the SAME path.
    #[serde(default)]
    build_root: String,
    #[serde(default)]
    build_rustflags: String,
    binary: String,
    binary_blake3: String,
    runtime_probe: String,
    runtime_probe_blake3: String,
    runtime_probe_exit_code: i32,
    instrumentation_flags: Vec<String>,
    #[serde(default)]
    build_log: String,
    #[serde(default)]
    build_log_blake3: String,
    #[serde(default)]
    source_commit: String,
    #[serde(default)]
    source_tree: String,
    #[serde(default)]
    build_result: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusAccessAudit {
    schema_version: String,
    tracer: String,
    source_commit: String,
    source_tree: String,
    targets: Vec<CorpusAccessAuditTarget>,
    /// Qualification-wide tracer receipts. Inventory-only audits may omit
    /// these while the packet remains unrun; qualified packets may not.
    #[serde(default)]
    scope_traces: Vec<CorpusScopeTrace>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusAccessAuditTarget {
    target: String,
    manifest: String,
    manifest_blake3: String,
    seed: u64,
    sanitizer: String,
    exit_code: i32,
    log_blake3: String,
    command: String,
    /// Raw recursive trace, retained separately from normalized path list.
    #[serde(default)]
    trace: String,
    #[serde(default)]
    trace_blake3: String,
    #[serde(default)]
    trace_pid: u32,
    #[serde(default)]
    trace_exit_code: i32,
    #[serde(default)]
    trace_complete: bool,
    #[serde(default)]
    process_binding: String,
    #[serde(default)]
    tracer_binary: String,
    #[serde(default)]
    tracer_version: String,
    #[serde(default)]
    tracer_version_blake3: String,
    #[serde(default)]
    trace_root: String,
    #[serde(default)]
    resolved_paths_blake3: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusScopeTrace {
    scope: String,
    command: String,
    trace: String,
    trace_blake3: String,
    exit_code: i32,
    result: String,
    #[serde(default)]
    trace_pid: u32,
    #[serde(default)]
    trace_exit_code: i32,
    #[serde(default)]
    trace_complete: bool,
    #[serde(default)]
    process_binding: String,
    #[serde(default)]
    tracer_binary: String,
    #[serde(default)]
    tracer_version: String,
    #[serde(default)]
    tracer_version_blake3: String,
    #[serde(default)]
    observed_paths_blake3: String,
    #[serde(default)]
    trace_root: String,
    #[serde(default)]
    resolved_paths_blake3: String,
    /// Carved per-target traces this scope's stage trace also covered (F-43).
    #[serde(default)]
    parts: Vec<ScopeTracePart>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ConcurrentEvidence {
    schema_version: String,
    implementation: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    source_anchor: String,
    #[serde(default)]
    runner: String,
    source_commit: String,
    source_tree: String,
    oracle_id: String,
    schedules: Vec<ConcurrentSchedule>,
    /// Blind pass 1 (2026-09-05) A05: the not-applicable label used to be
    /// trusted from the packet. The generator now scans the implementation:
    /// concurrent EXECUTION (spawned threads, async runtimes) contradicts the
    /// label and refuses; synchronization primitives are recorded here, and
    /// the gate recomputes the scan so the record describes the tree.
    #[serde(default)]
    sync_primitives: Vec<String>,
}

/// Crates outside the qualified surfaces: the gate itself and the scratch
/// test scaffold. Closed list, so an addition is a visible decision.
const CONCURRENCY_SCAN_EXCLUDED_CRATES: [&str; 2] = ["liminal-xtask", "liminal-scratch"];
/// Blind pass 1 at aecb2ec7 (A11): a lexical scan cannot see inside a macro
/// that expands to a spawn. Today none can. The scanned crates depend on
/// exactly these external crates, neither of which exports a macro that
/// spawns, and a macro defined inside a scanned crate carries the primitive
/// in its own body, where the scan reads it. The list is closed so a new
/// dependency asks the question again instead of answering it silently.
///
/// What each is: `serde`, `serde_json`, `thiserror`, `anyhow`, `clap` and
/// `uuid` export derive and formatting macros that expand to no execution;
/// `tracing`'s macros record spans in the calling thread. `blake3` is taken at
/// default features, so its optional `rayon` thread pool is not compiled in.
/// `ropey`, `rowan`, `camino`, `crc32fast`, `fs4` and `toml` export no macro
/// that spawns.
const CONCURRENCY_SCAN_ALLOWED_DEPENDENCIES: [&str; 14] = [
    "anyhow",
    "blake3",
    "camino",
    "clap",
    "crc32fast",
    "fs4",
    "ropey",
    "rowan",
    "serde",
    "serde_json",
    "thiserror",
    "toml",
    "tracing",
    "uuid",
];

/// The external crates `manifest` depends on at run time, workspace members
/// excluded. Dev-dependencies are excluded too: the scan reads `src/` only,
/// which a dev-dependency cannot reach.
fn runtime_external_dependencies(manifest: &Utf8Path) -> Result<BTreeSet<String>> {
    let text = fs::read_to_string(manifest).with_context(|| format!("read {manifest}"))?;
    let mut section = String::new();
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            section.clear();
            section.push_str(name);
            continue;
        }
        let last = section.rsplit('.').next().unwrap_or_default();
        if last != "dependencies" || section.contains("dev-") || section.contains("build-") {
            continue;
        }
        let Some((key, _)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        // `serde.workspace = true` names the crate before the dot.
        let key = key.split('.').next().unwrap_or(key);
        if key.is_empty() || key.starts_with('#') || key.starts_with("liminal") {
            continue;
        }
        out.insert(key.to_owned());
    }
    Ok(out)
}

/// Tokens that mean code runs concurrently.
// Blind pass 1 at f360e90 (A06): `use std::thread as th; th::spawn(..)` named
// no listed token. The spawn and scope calls are matched by their call shape,
// whatever path prefix reaches them; `.spawn(` also catches a builder. The
// product crates spawn no processes today, so the over-match is a refusal
// that names its line, never a miss.
const CONCURRENCY_EXECUTION_PRIMITIVES: [&str; 12] = [
    // Blind pass 1 at 5fb1b57 (A06): the list was Rust-shaped, so a thread
    // created through libc named nothing.
    "pthread_create",
    "libc::clone",
    "clone3",
    // Blind pass 1 at aa00d41 (A06): the trailing parenthesis meant
    // `let f = thread::spawn;` named no token while spawning all the same.
    // A mention of the symbol is the signal; calling it is not required.
    "::spawn",
    ".spawn",
    "::scope",
    "thread::Builder",
    "tokio::",
    "rayon::",
    "async fn",
    "crossbeam",
    "mpsc::",
];
/// Tokens that mean code is written to be shared: recorded, not refused.
const CONCURRENCY_SYNC_PRIMITIVES: [&str; 3] = ["Mutex<", "RwLock<", "Atomic"];

struct ConcurrencyScan {
    execution: Vec<String>,
    sync: Vec<String>,
}

/// Scan the implementation crates' non-test sources for concurrency
/// primitives. A file's trailing `#[cfg(test)]` module is skipped; comment
/// lines are skipped. Deterministic order: crates, files and lines sorted.
fn scan_concurrency_primitives(root: &Utf8Path) -> Result<ConcurrencyScan> {
    fn walk(dir: &Utf8Path, out: &mut Vec<Utf8PathBuf>) -> Result<()> {
        let mut entries = fs::read_dir(dir.as_std_path())
            .with_context(|| format!("read {dir}"))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        for path in entries {
            let path = Utf8PathBuf::from_path_buf(path)
                .map_err(|p| anyhow::anyhow!("non-UTF-8 path {}", p.display()))?;
            if path.is_dir() {
                walk(&path, out)?;
            } else if path.extension() == Some("rs") {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut scan = ConcurrencyScan {
        execution: Vec::new(),
        sync: Vec::new(),
    };
    let crates_dir = root.join("crates");
    let mut crate_dirs = fs::read_dir(crates_dir.as_std_path())
        .with_context(|| format!("read {crates_dir}"))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    crate_dirs.sort();
    for member in crate_dirs {
        let member_dir = Utf8PathBuf::from_path_buf(member)
            .map_err(|p| anyhow::anyhow!("non-UTF-8 path {}", p.display()))?;
        let Some(name) = member_dir.file_name() else {
            continue;
        };
        if CONCURRENCY_SCAN_EXCLUDED_CRATES.contains(&name) || !member_dir.join("src").is_dir() {
            continue;
        }
        let manifest = member_dir.join("Cargo.toml");
        if manifest.is_file() {
            let external = runtime_external_dependencies(&manifest)?;
            let unknown = external
                .iter()
                .filter(|dep| !CONCURRENCY_SCAN_ALLOWED_DEPENDENCIES.contains(&dep.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            anyhow::ensure!(
                unknown.is_empty(),
                "{name} depends on {unknown:?}, outside the closed list the \
                 concurrency scan reasons about; a dependency may export a \
                 macro that spawns, which a lexical scan cannot see. Decide, \
                 then widen CONCURRENCY_SCAN_ALLOWED_DEPENDENCIES"
            );
        }
        let mut files = Vec::new();
        walk(&member_dir.join("src"), &mut files)?;
        for file in files {
            let text = fs::read_to_string(&file).with_context(|| format!("read {file}"))?;
            let relative = file.strip_prefix(root).unwrap_or(&file);
            // Blind pass 1 at 612cbcc (A06): this used to stop at the first
            // `#[cfg(test)]` line, so production code after an earlier test
            // module escaped the scan. The attributed item is skipped as a
            // block (brace depth) or, for a brace-less item, through its `;`,
            // and scanning resumes after it. F-56: braces used to be
            // counted textually, strings and comments included, so an
            // unbalanced brace inside a test module could end the skip late
            // and hide a production spawn. Only structural braces count now,
            // the same scanner the mutant anchor screen uses.
            let mut skipping: Option<(usize, bool)> = None;
            let mut scanner = SourceScanner::default();
            for (index, line) in text.lines().enumerate() {
                let trimmed = line.trim();
                let structural = scanner.structural(line);
                if let Some((depth, entered)) = skipping.as_mut() {
                    *depth += structural.matches('{').count();
                    let closes = structural.matches('}').count();
                    if *depth > 0 {
                        *entered = true;
                    }
                    *depth = depth.saturating_sub(closes);
                    if (*entered && *depth == 0) || (!*entered && trimmed.ends_with(';')) {
                        skipping = None;
                    }
                    continue;
                }
                if trimmed.replace(' ', "") == "#[cfg(test)]" {
                    skipping = Some((0, false));
                    continue;
                }
                if trimmed.starts_with("//") {
                    continue;
                }
                for token in CONCURRENCY_EXECUTION_PRIMITIVES {
                    if line.contains(token) {
                        scan.execution
                            .push(format!("{relative}:{}:{token}", index + 1));
                    }
                }
                for token in CONCURRENCY_SYNC_PRIMITIVES {
                    if line.contains(token) {
                        scan.sync.push(format!("{relative}:{}:{token}", index + 1));
                    }
                }
            }
        }
    }
    Ok(scan)
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ConcurrentSchedule {
    id: String,
    sequence: Vec<String>,
    result: String,
    oracle_digest: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CampaignClock {
    schema_version: String,
    reference_machine: String,
    runs: Vec<CampaignRun>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CampaignRun {
    id: String,
    commit: String,
    tree: String,
    command: String,
    started_epoch: u64,
    finished_epoch: u64,
    elapsed_s: u64,
    clean: bool,
    result: String,
    wrapper: String,
    wrapper_sha256: String,
    #[serde(default)]
    receipt: String,
    #[serde(default)]
    receipt_blake3: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ResidualRisk {
    id: String,
    owner: String,
    severity: String,
    trigger: String,
    requirement: String,
    evidence: String,
    planned_phase: String,
    mitigation: String,
    authority: String,
    state: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
    id: String,
    kind: String,
    source: String,
    critical: bool,
    /// Explicit stateful-law marker. The qualified layer requires fault and
    /// recovery evidence for this marker, independently of `kind` spelling.
    #[serde(default)]
    stateful: bool,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Test {
    id: String,
    name: String,
    requirements: Vec<String>,
    /// Evidence kinds this test supplies for the requirements it maps to
    /// (ADR-0020 §2). One of `positive`, `negative`, `malformed`, `basis`,
    /// `replay`, `fault`, `recovery`.
    evidence: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Mutant {
    id: String,
    family: String,
    operator: String,
    source: String,
    /// Requirement attacked by exact source coordinate. Legacy proposed
    /// packets may omit this until M24 coordinate migration.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    requirement: String,
    defect: String,
    killing_tests: Vec<String>,
    disposition: String,
    /// Exact source replacement used by the mutation runner. Predeclared
    /// inventory rows may omit this until their owning milestone supplies the
    /// implementation coordinate; every evaluated disposition must carry it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    patch: Option<MutantPatch>,
    /// Required proof for an equivalent or duplicate disposition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    disposition_proof: Option<String>,
    /// Reviewer names that concurred with an equivalent/duplicate proof.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    disposition_concurrence: Vec<DispositionConcurrence>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct DispositionConcurrence {
    reviewer: String,
    record: String,
    finding: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MutantPatch {
    file: String,
    before: String,
    after: String,
}

/// Durable result of one mutation-run invocation. Raw test output is never
/// persisted; digests and exit states are enough to audit what ran without
/// turning model/test output into a secret-bearing artifact.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct MutantEvidence {
    schema_version: String,
    source_commit: String,
    lockfile_blake3: String,
    runner: String,
    rows: Vec<MutantEvidenceRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct MutantEvidenceRow {
    id: String,
    disposition: String,
    status: String,
    patch: Option<MutantPatch>,
    runnable_tests: Vec<String>,
    baseline_exit_codes: Vec<i32>,
    failed_tests: Vec<String>,
    exit_codes: Vec<i32>,
    stdout_blake3: Vec<String>,
    stderr_blake3: Vec<String>,
    reason: Option<String>,
    #[serde(default)]
    failure_class: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Canary {
    id: String,
    gate: String,
    violation: String,
    expected_failure: String,
    result: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Generated {
    family: String,
    accepted: u64,
    attempts: u64,
    discards: u64,
    /// Exercised out-of-domain negative cases, distinct from generator discards.
    #[serde(default)]
    negatives: u64,
    /// Fuzz targets that evidence THIS family (ADR-0020 §4; M17.5 F-29).
    ///
    /// Replaces a per-family `fuzz_minutes` number. Minutes are derived by
    /// summing these targets' recorded seconds, so family budget is reconciled
    /// to one committed campaign artifact rather than asserted independently.
    ///
    /// May be empty at the inventory layer, which describes a plan; the
    /// qualified layer refuses a family claiming `pass` with no target, the
    /// same way it refuses a test name that does not exist.
    #[serde(default)]
    fuzz_targets: Vec<String>,
    seed_categories: Vec<String>,
    result: String,
    /// Seed the recorded run used; required once `result` is `pass` so the
    /// campaign is reproducible (ADR-0020 §1) and its numbers cannot be
    /// asserted without a run behind them (M17.5 F-04/F-07).
    #[serde(default)]
    seed: Option<u64>,
    /// BLAKE3 of the recorded run's evidence stream; required once `pass`.
    #[serde(default)]
    evidence_hash: Option<String>,
    /// Closed relation matrix required at qualification time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    relations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oracle: Option<GeneratedOracleDeclaration>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct GeneratedOracleDeclaration {
    id: String,
    source: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashBoundary {
    boundary: String,
    before: bool,
    after: bool,
    result: String,
}

/// The declared reach of ADR-0020 §5's crash-boundary exhaustiveness claim.
///
/// §5 says "exhaustive registered crash boundaries" without saying exhaustive
/// over what. M17.5 A05 measured it and the honest answer is: over ILRP, and
/// nothing else. The registry holds eight `ilrp/*` boundaries while
/// `txn.commit()` — which `liminal-graph/src/store/mod.rs` documents as
/// "Durable boundary: the record is fsynced before the commit returns" — occurs
/// 35 more times in runtime source with no boundary at all.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashBoundaryScope {
    in_scope: Vec<ScopedSubsystem>,
    deferred: Vec<DeferredDurableSurface>,
    /// Who closes the deferral, so the disclosure names an owner.
    deferred_to: String,
    rationale: String,
}

/// One subsystem whose durable transitions §5's claim covers.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ScopedSubsystem {
    /// Boundary-name prefix, e.g. `ilrp` for `ilrp/before_ack`.
    subsystem: String,
    path: String,
    sites: usize,
}

/// One durable surface deliberately left outside §5 for this stage.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct DeferredDurableSurface {
    path: String,
    sites: usize,
    reason: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Review {
    reviewer: String,
    attempts: u64,
    /// Finding ids this reviewer raised.
    #[serde(default)]
    findings: Vec<String>,
    /// The subset of `findings` a SECOND party reproduced independently.
    ///
    /// ADR-0020 §6 exists because one model asserting a defect is not evidence
    /// of a defect. Without this field a review row was four self-asserted
    /// numbers (M17.5 pass-2 #20): a reviewer could claim twelve attempts and
    /// zero unresolved findings having reproduced nothing at all.
    #[serde(default)]
    independently_reproduced: Vec<String>,
    unresolved_verified_findings: u64,
    /// Committed record of the run; required once `result` is `pass`.
    #[serde(default)]
    evidence: Option<String>,
    result: String,
}

/// A blind-review run's committed record, as `scripts/haqp_blind_review.py`
/// writes it. Typed rather than probed as a `serde_json::Value` so a field the
/// verifier depends on cannot quietly go missing.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewRecord {
    schema_version: String,
    pass: u8,
    reviewer: ReviewRecordReviewer,
    attempts: Vec<ReviewRecordAttempt>,
    #[serde(default)]
    findings: Vec<serde_json::Value>,
    independently_reproduced: Vec<String>,
    unresolved_verified_findings: u64,
    result: String,
    #[serde(default)]
    raw_response_sha256: String,
    /// How many off-schema answers preceded this record (F-46).
    #[serde(default)]
    schema_retries: u32,
    /// The provider's own evidence that the exchange happened (F-49).
    provider_receipt: ReviewProviderReceipt,
    #[serde(default)]
    integrity_binding_sha256: String,
    isolated_session_hash: String,
    sanitized_prompt_hash: String,
    prompt_binding_sha256: String,
    fixed_base: ReviewFixedBase,
    blindness_proof: ReviewRecordBlindness,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewRecordReviewer {
    model_family: String,
    identity_hash: String,
    backend: String,
}

/// Blind pass 1 at aa00d41 (A09): every independence field in a review record
/// was computed by the runner, so nothing outside this machine showed that two
/// distinct reviewers had answered. A receipt is the provider's own artifact —
/// a Codex rollout transcript, or MiMo's response envelope and billed usage.
/// It is not unforgeable by a hostile runner; it is externally originated and
/// checkable against the vendor, which is the evidence a non-hostile
/// qualification can actually produce (Brian's ruling, 2026-09-06: AI reviews
/// are inherently hostile to perfect idempotency, so the receipt attests that
/// the exchange OCCURRED, never that it would recur).
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewProviderReceipt {
    backend: String,
    #[serde(default)]
    session_file: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    transcript_sha256: String,
    #[serde(default)]
    transcript_bytes: u64,
    #[serde(default)]
    response_id: String,
    #[serde(default)]
    provider_model: String,
    #[serde(default)]
    created: i64,
    #[serde(default)]
    usage: BTreeMap<String, serde_json::Value>,
}

/// Findings carry model-chosen key names, so they stay untyped; ATTEMPTS are
/// the runner's own contract and are pinned.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewRecordAttempt {
    id: String,
    attack_class: String,
    target: String,
    attempt: String,
    observed_result: String,
    independently_reproduced: bool,
    classification: String,
    resolved: bool,
    #[serde(default)]
    resolution: Option<ReviewResolution>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewResolution {
    commit: String,
    coordinate: String,
    evidence_path: String,
    evidence_sha256: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewFixedBase {
    commit: String,
    tree: String,
    clean: bool,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewRecordBlindness {
    ephemeral_session_requested: bool,
    session_state: String,
    prior_pass_artifact_supplied: bool,
    #[serde(default)]
    pass_two_original_spec_only: bool,
}

/// One scenario's fault-matrix outcome (M17.5 F-31 / P1-A08).
///
/// The artifact has always carried this block and the verifier has always
/// ignored it — the field was typed `Vec<serde_json::Value>` and read by
/// nothing, so a scenario that injected no faults, or reported a non-pass
/// result, satisfied every check.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashScenario {
    scenario: String,
    faults_injected: u64,
    boundaries: BTreeSet<String>,
    result: String,
}

/// One boundary's recorded fault-matrix outcome (committed artifact).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashEvidence {
    source_commit: String,
    source_tree: String,
    lockfile_blake3: String,
    registered: usize,
    exercised: usize,
    /// Per-scenario rows; carried so `deny_unknown_fields` cannot be defeated
    /// by the lane emitting a field the verifier silently drops (the F-17
    /// lesson applied before it bites).
    scenarios: Vec<CrashScenario>,
    boundaries: Vec<CrashEvidenceBoundary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CrashEvidenceBoundary {
    boundary: String,
    /// Receipt that runtime fault injection reached this exact boundary.
    injected: bool,
    occurrences_exercised: u64,
    recovery_pairs: Vec<RecoveryPair>,
    staged_residue: String,
    result: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RecoveryPair {
    scenario: String,
    occurrence: u64,
    /// The terminal states recovery actually reached, by name.
    first_terminals: Vec<String>,
    first_recovery_digest: String,
    second_recovery_digest: String,
    #[serde(default)]
    first_terminal_digest: String,
    #[serde(default)]
    second_terminal_digest: String,
    #[serde(default)]
    first_basis_digest: String,
    #[serde(default)]
    second_basis_digest: String,
    #[serde(default)]
    first_effect_digest: String,
    #[serde(default)]
    second_effect_digest: String,
}

/// Execute every declared disposable gate canary without touching the working
/// tree.
pub fn run_canaries_repo(root: &Utf8Path) -> Result<()> {
    let baseline = read_packet(root)?;
    verify_inventory_repo(root)?;
    let markdown_path = root.join("docs/execution/phase1-suite-review.md");
    let markdown = fs::read_to_string(&markdown_path)?;
    let records = run_canary_suite(root, &baseline, &markdown)?;
    let bytes = serde_json::to_vec_pretty(&records)?;
    let path = root.join("conformance/haqp/evidence/canaries.json");
    fs::create_dir_all(path.parent().expect("evidence parent"))?;
    fs::write(&path, &bytes)?;
    println!(
        "all {} canaries caught; evidence {path}; blake3={}",
        records.len(),
        blake3::hash(&bytes).to_hex()
    );
    Ok(())
}

/// Run one canary per DECLARED row, and require each row's prose to describe
/// the mutation that was actually performed (M17.5 pass-2 #22).
///
/// The runner used to iterate a hardcoded canary range and read only
/// `expected_failure`. `Canary.gate` and `Canary.violation` were therefore
/// decorative: the packet could say C09 relabels a mutation family while the
/// executed arm did something entirely different, and nothing would notice.
/// That matters because the canary table is the artifact a reader trusts to
/// learn WHAT the canaries prove — unverified prose in that position is
/// worse than no prose, because it reads as evidence.
///
/// Driving the loop from `packet.canaries` rather than a hardcoded `1..=16`
/// closes a drift hazard rather than a live hole: `verify_canary_inventory`
/// currently pins the id set to exactly C01–C25, so an extra row is rejected
/// before it reaches this function. But that rule is the only thing keeping the
/// hardcoded range honest, and the natural future edit — relaxing it to a
/// minimum when canary 17 is written — would have left the new canaries
/// declared, counted, and never executed. The loop now cannot disagree with the
/// table it reports on.
fn run_canary_suite(
    root: &Utf8Path,
    baseline: &Packet,
    markdown: &str,
) -> Result<Vec<CanaryEvidence>> {
    let baseline_digest = packet_digest(baseline)?;
    let mut records = Vec::new();
    for row in &baseline.canaries {
        let mut packet = baseline.clone();
        let mut altered_markdown = markdown.to_owned();
        let performed = mutate_canary(
            &row.id,
            &mut packet,
            &mut altered_markdown,
            &baseline_digest,
        )?;
        require_eq(
            &format!("{}.violation (packet prose vs executed mutation)", row.id),
            &row.violation,
            performed,
        )?;
        let observed = if is_qualification_canary(&row.id) {
            run_qualification_canary(root, baseline, &row.id)
                .expect_err("qualification canary must fail closed")
                .to_string()
        } else {
            verify_packet_shape(&packet)
                .and_then(|()| verify_packet_statuses(&packet, inventory_statuses()))
                .and_then(|()| verify_markdown_surface_text(&altered_markdown, &packet))
                .expect_err("mutated canary must fail closed")
                .to_string()
        };
        if !canary_failure_matches(&row.expected_failure, &observed) {
            anyhow::bail!(
                "{}: expected failure {:?}, observed {observed:?}",
                row.id,
                row.expected_failure
            );
        }
        let canonical = canary_expected_prefix(&row.id)?;
        if let Some((_, phrase)) = CANARY_REQUIRED_PHRASE
            .iter()
            .find(|(id, _)| *id == row.id.as_str())
        {
            anyhow::ensure!(
                row.expected_failure.contains(phrase),
                "{}: expected failure {:?} must name the violation this canary performs \
                 ({phrase:?}); its prefix is shared by every refusal of its kind",
                row.id,
                row.expected_failure
            );
        }
        anyhow::ensure!(
            row.expected_failure.starts_with(canonical),
            "{}: expected failure {:?} is outside closed canary failure registry {:?}",
            row.id,
            row.expected_failure,
            canonical
        );
        records.push(CanaryEvidence {
            id: row.id.clone(),
            gate: row.gate.clone(),
            violation: row.violation.clone(),
            expected_failure: row.expected_failure.clone(),
            observed_failure: observed,
            caught: true,
            mutation_semantics: canary_mutation_semantics(&row.id)?.to_owned(),
        });
    }
    Ok(records)
}

fn is_qualification_canary(id: &str) -> bool {
    matches!(
        id,
        "C26" | "C27" | "C28" | "C29" | "C30" | "C31" | "C32" | "C33"
    )
}

/// A scratch repository in which a provenance block is ACCEPTED: the fixed
/// base commit (lockfile and unbound packet), then a metadata child whose
/// packet is bound to it. C26 strips the block from that state (A01).
fn provenance_canary_repo(
    root: &Utf8Path,
    packet: &Packet,
) -> Result<(liminal_scratch::ScratchDir, Utf8PathBuf, Packet)> {
    let scratch = liminal_scratch::ScratchDir::new("haq-c26").context("C26 scratch")?;
    let scratch_root = Utf8PathBuf::from_path_buf(fs::canonicalize(scratch.path().as_std_path())?)
        .map_err(|p| anyhow::anyhow!("non-UTF-8 scratch path {}", p.display()))?;
    let git = |args: &[&str]| -> Result<String> {
        let output = Command::new("git")
            .current_dir(&scratch_root)
            .args(["-c", "user.email=haq@liminal", "-c", "user.name=haq"])
            .args(args)
            .output()
            .with_context(|| format!("C26 git {args:?}"))?;
        anyhow::ensure!(
            output.status.success(),
            "C26 git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    git(&["init", "-q"])?;
    fs::create_dir_all(scratch_root.join("conformance/haqp"))?;
    let lockfile = fs::read(root.join("Cargo.lock")).context("C26: Cargo.lock")?;
    fs::write(scratch_root.join("Cargo.lock"), &lockfile)?;
    let mut unbound = packet.clone();
    unbound.provenance = None;
    fs::write(
        scratch_root.join("conformance/haqp/packet.json"),
        serde_json::to_vec_pretty(&unbound)?,
    )?;
    git(&["add", "-A"])?;
    git(&["commit", "-q", "-m", "fixed base"])?;
    let base = git(&["rev-parse", "HEAD"])?;
    let tree = git(&["rev-parse", "HEAD^{tree}"])?;
    let mut qualified = unbound;
    qualified.provenance = Some(Provenance {
        commit: base.clone(),
        lockfile_blake3: blake3::hash(&lockfile).to_hex().to_string(),
        fixed_commit: base.clone(),
        fixed_tree: tree,
        evidence_parent: base,
    });
    fs::write(
        scratch_root.join("conformance/haqp/packet.json"),
        serde_json::to_vec_pretty(&qualified)?,
    )?;
    git(&["commit", "-q", "-am", "qualification metadata"])?;
    Ok((scratch, scratch_root, qualified))
}

/// Execute deliberate failures for qualified-only gates whose evidence does
/// not exist in the proposed inventory packet. Each arm mutates a valid
/// qualification state, then invokes the closed failure contract.
#[allow(
    clippy::too_many_lines,
    reason = "one match arm per canary; the length is the canary count"
)]
fn run_qualification_canary(root: &Utf8Path, packet: &Packet, id: &str) -> Result<()> {
    if id == "C27" {
        return run_sanitizer_canary(root);
    }
    if id == "C33" {
        // A05: understating the deferred surface by ONE site is the realistic
        // way this disclosure goes wrong — someone adds a `txn.commit()` and
        // does not revisit the packet. The canary proves the tripwire fires on
        // exactly that, rather than only on a wholesale deletion.
        let mut candidate = packet.clone();
        let surface = candidate
            .crash_boundary_scope
            .deferred
            .first_mut()
            .context("C33 needs at least one deferred durable surface to understate")?;
        surface.sites -= 1;
        let refusal = verify_crash_boundary_scope(root, &candidate)
            .expect_err("crash-boundary scope canary must fail closed");
        // Blind pass 1 at a53f6a4d (A12): a canary that accepts any error
        // proves the gate refused something, not that it refused THIS.
        anyhow::ensure!(
            refusal
                .to_string()
                .contains("durable transition(s), source has"),
            "C33 refused for another reason: {refusal}"
        );
        anyhow::bail!(
            "crash_boundary_scope declares a durable surface smaller than tracked source"
        );
    }
    if id == "C32" {
        let path = root.join("conformance/haqp/evidence/concurrency.json");
        let mut evidence: ConcurrentEvidence = serde_json::from_slice(&fs::read(&path)?)?;
        evidence.source_tree = "0".repeat(40);
        let refusal = verify_concurrency_evidence_record(root, packet, &evidence)
            .expect_err("concurrency provenance canary must fail closed");
        anyhow::ensure!(
            refusal
                .to_string()
                .contains("concurrency source provenance does not match committed tree"),
            "C32 refused for another reason: {refusal}"
        );
        anyhow::bail!("concurrency provenance does not match committed tree");
    }
    match id {
        "C26" => {
            // Blind pass 1 (2026-09-05) A01: at the fixed base the packet has
            // no provenance, so stripping it changed nothing and the canary
            // "caught" the baseline's own refusal. Removal is only a violation
            // of an ACCEPTED state: build one in a scratch repository (fixed
            // base, then a metadata child carrying a bound provenance block),
            // require the verifier to accept it, then strip it.
            let (_scratch, scratch_root, qualified) = provenance_canary_repo(root, packet)?;
            verify_provenance(&scratch_root, &qualified).context(
                "C26: a bound provenance block must verify before its removal can be caught",
            )?;
            let mut candidate = qualified;
            candidate.provenance = None;
            let error = verify_provenance(&scratch_root, &candidate)
                .expect_err("provenance canary must exercise provenance verifier");
            anyhow::bail!("{error}");
        }
        "C28" => {
            let path = root.join("conformance/haqp/evidence/generated.json");
            let artifact: GeneratedEvidenceArtifact = serde_json::from_slice(
                &fs::read(&path)
                    .with_context(|| "C28 canary requires committed generated evidence")?,
            )?;
            let family = packet
                .generated
                .iter()
                .find(|family| family.family == "source/CST/formatting")
                .context("C28 canary requires source/CST/formatting family")?;
            let row = artifact
                .rows
                .iter()
                .find(|row| row.family == family.family)
                .context("oracle canary requires generated evidence")?;
            let mut row = row.clone();
            let oracle = row
                .oracle
                .as_mut()
                .context("oracle canary requires generated oracle evidence")?;
            oracle.independent = false;
            let error = verify_generated_contract(root, family, &row)
                .expect_err("oracle canary must exercise independent-oracle verifier");
            anyhow::bail!("generated oracle is not independent: {error}");
        }
        "C29" => {
            let clock = CampaignClock {
                schema_version: "haqp-campaign-clock-v1".to_owned(),
                reference_machine: "canary".to_owned(),
                runs: vec![
                    CampaignRun {
                        id: "C29-1".to_owned(),
                        // Budget-only helper intentionally skips provenance;
                        // keep object IDs syntactically valid for expansion.
                        commit: "a".repeat(40),
                        tree: "b".repeat(40),
                        command: "canary".to_owned(),
                        started_epoch: 1,
                        finished_epoch: 1 + 8 * 60 * 60 + 1,
                        elapsed_s: 8 * 60 * 60 + 1,
                        clean: true,
                        result: "pass".to_owned(),
                        wrapper: "canary".to_owned(),
                        wrapper_sha256: "0".repeat(64),
                        receipt: String::new(),
                        receipt_blake3: String::new(),
                    },
                    CampaignRun {
                        id: "C29-2".to_owned(),
                        commit: "a".repeat(40),
                        tree: "b".repeat(40),
                        command: "canary".to_owned(),
                        started_epoch: 1,
                        finished_epoch: 1 + 8 * 60 * 60 + 1,
                        elapsed_s: 8 * 60 * 60 + 1,
                        clean: true,
                        result: "pass".to_owned(),
                        wrapper: "canary".to_owned(),
                        wrapper_sha256: "0".repeat(64),
                        receipt: String::new(),
                        receipt_blake3: String::new(),
                    },
                ],
            };
            let error = verify_campaign_clock_budget(&clock)
                .expect_err("campaign clock canary must exercise breach ceiling");
            anyhow::bail!("{error}");
        }
        "C30" => {
            let mut candidate = packet.clone();
            candidate.residual_risks = vec![ResidualRisk {
                id: "RISK-001".to_owned(),
                owner: "canary".to_owned(),
                severity: "high".to_owned(),
                trigger: "canary".to_owned(),
                requirement: "P1-R001".to_owned(),
                evidence: String::new(),
                planned_phase: "M24".to_owned(),
                mitigation: "canary".to_owned(),
                authority: "ADR-0020".to_owned(),
                state: "open".to_owned(),
            }];
            let error = verify_residual_risks(root, &candidate)
                .expect_err("residual-risk canary must exercise coordinate verifier");
            anyhow::bail!("{error}");
        }
        "C31" => {
            let path = root.join("conformance/haqp/evidence/corpus-access.json");
            let mut audit: CorpusAccessAudit = serde_json::from_slice(&fs::read(&path)?)?;
            audit.scope_traces.clear();
            verify_corpus_scope_presence(&audit)
                .expect_err("corpus canary must exercise qualification scope registry");
            anyhow::bail!("corpus scope command does not cover lane");
        }
        _ => anyhow::bail!("unknown qualification canary {id}"),
    }
}

/// Replace one retained sanitizer binary's runtime marker with inert bytes and
/// invoke the same compiler/runtime proof verifier used by qualification. This
/// keeps C27 tied to an executable artifact attack, not an in-memory boolean.
fn run_sanitizer_canary(root: &Utf8Path) -> Result<()> {
    let path = root.join("conformance/haqp/evidence/fuzz.json");
    let rows: Vec<FuzzEvidence> = serde_json::from_slice(&fs::read(&path)?)?;
    let row = rows
        .iter()
        .find(|row| row.sanitizer_proof.is_some())
        .context("sanitizer canary requires retained sanitizer proof")?;
    let mut proof = row
        .sanitizer_proof
        .clone()
        .context("sanitizer canary requires retained sanitizer proof")?;
    let scratch = liminal_scratch::ScratchDir::new("haq-sanitizer-canary")?;
    let scratch_root = scratch.path();
    // ScratchDir is fresh and private to this canary. Reject path escapes
    // before reading committed artifacts or creating any scratch entry.
    for (path, label) in [
        (&proof.binary, "sanitizer canary binary"),
        (&proof.runtime_probe, "sanitizer canary probe"),
        (&proof.build_log, "sanitizer canary build log"),
    ] {
        let candidate = Utf8Path::new(path);
        anyhow::ensure!(
            !candidate.is_absolute()
                && !candidate
                    .components()
                    .any(|part| part == camino::Utf8Component::ParentDir)
                && !is_locked_acceptance_path(path),
            "{label} path {path:?} escapes scratch evidence root"
        );
    }
    let binary_path = safe_repo_path(root, &proof.binary, "sanitizer canary binary")?;
    let mut binary = fs::read(&binary_path)?;
    let marker = match row.sanitizer.as_str() {
        "address" | "leak" => b"asan_globals".as_slice(),
        "memory" => b"msan".as_slice(),
        "thread" => b"tsan".as_slice(),
        _ => anyhow::bail!("unsupported sanitizer {}", row.sanitizer),
    };
    let mut replaced = false;
    for offset in 0..=binary.len().saturating_sub(marker.len()) {
        if &binary[offset..offset + marker.len()] == marker {
            binary[offset..offset + marker.len()].fill(b'X');
            replaced = true;
        }
    }
    anyhow::ensure!(
        replaced,
        "sanitizer canary source binary has no runtime marker"
    );
    let scratch_binary_path = scratch_root.join(&proof.binary);
    let scratch_probe_path = scratch_root.join(&proof.runtime_probe);
    let scratch_log_path = scratch_root.join(&proof.build_log);
    fs::create_dir_all(scratch_binary_path.parent().expect("binary parent"))?;
    fs::create_dir_all(scratch_probe_path.parent().expect("probe parent"))?;
    fs::create_dir_all(scratch_log_path.parent().expect("log parent"))?;
    fs::write(&scratch_binary_path, &binary)?;
    let runtime_probe_path = safe_repo_path(root, &proof.runtime_probe, "sanitizer canary probe")?;
    let build_log_path = safe_repo_path(root, &proof.build_log, "sanitizer canary build log")?;
    fs::copy(runtime_probe_path, &scratch_probe_path)?;
    fs::copy(build_log_path, &scratch_log_path)?;
    proof.binary_blake3 = hex_digest(&binary);
    proof.runtime_probe_blake3 = hex_digest(&fs::read(&scratch_probe_path)?);
    proof.build_log_blake3 = hex_digest(&fs::read(&scratch_log_path)?);
    let error = verify_sanitizer_proof(scratch_root, row, &proof, None)
        .expect_err("uninstrumented sanitizer canary must fail closed");
    anyhow::bail!("sanitizer proof lacks compiler/runtime replay: {error}");
}

/// Match expected canary coordinates at the beginning of the verifier error.
/// A substring search lets unrelated detail later in an error satisfy a row;
/// exact equality or a delimited suffix preserves diagnostic detail without
/// allowing that false positive.
fn canary_failure_matches(expected: &str, observed: &str) -> bool {
    let expected = expected.trim();
    observed == expected
        || observed.starts_with(&format!("{expected}:"))
        || observed.starts_with(&format!("{expected},"))
        || observed.starts_with(&format!("{expected} "))
}

/// Phrases a canary's declared failure must CONTAIN, for canaries whose
/// closed prefix is generic. Blind pass 1 at `ec045588` (A09): C13's prefix is
/// `fuzz target`, which every refusal about any fuzz target shares — the
/// target's own name sits between the prefix and anything distinctive, so no
/// prefix can carry the meaning. The phrase can.
const CANARY_REQUIRED_PHRASE: [(&str, &str); 1] = [("C13", "is claimed by both")];

/// Packet prose may add concrete values after a closed failure prefix, but it
/// cannot replace the failure family with a packet-controlled generic phrase.
fn canary_expected_prefix(id: &str) -> Result<&'static str> {
    Ok(match id {
        "C01" => "status: expected",
        "C02" => "ratification: expected",
        "C03" => "locked_acceptance_corpora_touched must be false",
        "C04" => "duplicate requirement id",
        "C05" => "test ids differ",
        "C06" => "P1-T01 has no requirement mapping",
        "C07" => "mutant count must be 65",
        "C08" => "operator predicate-deletion supplies 22 mutants; max 16",
        "C09" => "family graph/interchange codecs mutant count must be 13",
        "C10" => "canary ids differ",
        "C11" => "source/CST/formatting accepted cases below 100000",
        "C12" => "source/CST/formatting discard rate exceeds 1%",
        "C13" => "fuzz target",
        "C14" => "crash-boundary inventory mismatch",
        "C15" => "gpt-5.6-sol-blind-pass has fewer than 12 attempts",
        "C16" => "review packet markdown missing packet digest",
        "C17" => "requirement P1-R001 differs from closed authoritative registry",
        "C18" => "P1-T01 declares no evidence kind",
        "C19" => "requirement P1-R015 is stateful and must set stateful=true",
        "C20" => "P1-M001 source must be safe file:line",
        "C21" => "P1-M001 declares unknown disposition",
        "C22" => "source/CST/formatting repeats seed category",
        "C23" => "source/CST/formatting:",
        "C24" => "ilrp/before_intent_commit missing before/after injection declaration",
        "C25" => "duplicate reviewer id gpt-5.6-sol-blind-pass",
        "C26" => "packet has no provenance block; qualification is unbound to any tree",
        "C27" => "sanitizer proof lacks compiler/runtime replay",
        "C28" => "generated oracle is not independent",
        "C29" => "campaign exceeded the eight-hour ceiling",
        "C30" => "risk RISK-001 has empty evidence",
        "C31" => "corpus scope command does not cover lane",
        "C32" => "concurrency provenance does not match committed tree",
        // Blind pass 1 at a53f6a4d (A12): the bare prefix also matched two
        // other messages from this same verifier, so an unrelated refusal
        // would have read as this canary's catch.
        "C33" => "crash_boundary_scope declares a durable surface smaller than tracked source",
        _ => anyhow::bail!("unknown canary {id}"),
    })
}

fn inventory_statuses() -> PacketStatusExpectations {
    PacketStatusExpectations {
        mutant: "predeclared",
        canary: "predeclared",
        generated: "planned",
        crash: "registered",
        review: "planned",
    }
}

/// Apply one canary's mutation and RETURN the violation it performed, so the
/// caller can hold the packet's prose to it (M17.5 pass-2 #22). The returned
/// string is the authority: if the packet disagrees, the packet is wrong.
#[allow(
    clippy::too_many_lines,
    reason = "canary registry maps each declared mutation to one executable check"
)]
fn mutate_canary(
    id: &str,
    packet: &mut Packet,
    markdown: &mut String,
    baseline_digest: &str,
) -> Result<&'static str> {
    let performed = match id {
        "C01" => {
            packet.status.clear();
            packet.status.push_str("ratified");
            "set status to ratified"
        }
        "C02" => {
            packet.ratification.clear();
            packet.ratification.push_str("approved");
            "set ratification to approved"
        }
        "C03" => {
            packet.locked_acceptance_corpora_touched = true;
            "set locked_acceptance_corpora_touched true"
        }
        "C04" => {
            packet.requirements.push(packet.requirements[0].clone());
            "duplicate requirement id"
        }
        "C05" => {
            packet.tests.retain(|row| row.id != "P1-T08");
            "remove P1-T08"
        }
        "C06" => {
            packet.tests[0].requirements.clear();
            "empty requirement mapping"
        }
        "C07" => {
            packet.mutants.pop();
            "remove one mutant"
        }
        "C08" => {
            for mutant in packet.mutants.iter_mut().take(17) {
                mutant.operator.clear();
                mutant.operator.push_str("predicate-deletion");
            }
            "operator supplies 17 mutants"
        }
        "C09" => {
            // Moving one mutant out of its family leaves that family at 12,
            // which is what `verify_mutant_inventory`'s exactly-13 rule catches.
            let family = packet.mutants[0].family.clone();
            packet.mutants[0].family = if family == "source/CST/formatting" {
                "graph/interchange codecs".to_owned()
            } else {
                "source/CST/formatting".to_owned()
            };
            "family supplies 12 mutants"
        }
        "C10" => {
            packet.canaries.retain(|row| row.id != "C16");
            "remove C16"
        }
        "C11" => {
            packet.generated[0].accepted = 0;
            "accepted below 100000"
        }
        "C12" => {
            packet.generated[0].discards = packet.generated[0].attempts;
            "discard rate above 1 percent"
        }
        "C13" => {
            // Was "fuzz minutes below 30". F-17 removed the per-family minutes
            // (they were a second, independently-assertable claim about one
            // campaign); the budget is now derived from the artifact, so the
            // packet-level violation this canary can perform is an overlapping
            // mapping — one campaign satisfying two families' budgets.
            let stolen = packet.generated[1]
                .fuzz_targets
                .first()
                .cloned()
                .unwrap_or_else(|| "cst_parse".to_owned());
            packet.generated[0].fuzz_targets.push(stolen);
            "claim another family's fuzz target"
        }
        "C14" => {
            packet
                .crash_boundaries
                .retain(|row| row.boundary != "ilrp/before_ack");
            "drop before_ack boundary"
        }
        "C15" => {
            packet.reviews[0].attempts = 0;
            "review attempts below 12"
        }
        "C16" => {
            // The only arm that attacks the markdown surface rather than the
            // packet, so it must NOT re-stamp the digest it just removed.
            *markdown = markdown.replace(baseline_digest, "");
            return Ok("remove packet digest");
        }
        "C17" => {
            packet.requirements[0].source = String::from("docs/execution/M18.md:D18.999");
            "change authoritative requirement source"
        }
        "C18" => {
            packet.tests[0].evidence.clear();
            "remove all evidence kinds from P1-T01"
        }
        "C19" => {
            packet.requirements[14].stateful = false;
            "clear stateful marker on P1-R015"
        }
        "C20" => {
            packet.mutants[0].source = String::from("not-a-file:0");
            "replace mutant source with malformed coordinate"
        }
        "C21" => {
            packet.mutants[0].disposition = String::from("invented");
            "invent mutant disposition"
        }
        "C22" => {
            packet.generated[0].seed_categories[1] = packet.generated[0].seed_categories[0].clone();
            "duplicate generated seed category"
        }
        "C23" => {
            packet.generated[0].attempts += 1;
            "break generated count accounting"
        }
        "C24" => {
            packet.crash_boundaries[0].before = false;
            "remove before-transition crash declaration"
        }
        "C25" => {
            packet.reviews[1].reviewer = packet.reviews[0].reviewer.clone();
            "duplicate reviewer identity"
        }
        "C26" => "remove qualification provenance",
        "C27" => "replace retained sanitizer binary marker and invoke runtime proof verifier",
        "C28" => "reuse production oracle for expected result",
        "C29" => "exceed eight-hour ceiling",
        "C30" => "omit residual-risk coordinate",
        "C31" => "trace only scope probe",
        "C32" => "replace not-applicable concurrency source_tree with unrelated Git object",
        "C33" => "understate the deferred durable surface by one site",
        _ => anyhow::bail!(
            "unknown canary {id}: the packet declares a canary with no implemented \
             mutation, so it would otherwise be counted as caught without running"
        ),
    };
    let digest = packet_digest(packet)?;
    *markdown = markdown.replace(baseline_digest, &digest);
    Ok(performed)
}

fn packet_digest(packet: &Packet) -> Result<String> {
    Ok(blake3::hash(&serde_json::to_vec(packet)?)
        .to_hex()
        .to_string())
}

fn generated_artifact_digest(rows: &[GeneratedEvidence]) -> Result<String> {
    Ok(blake3::hash(&serde_json::to_vec(rows)?)
        .to_hex()
        .to_string())
}

fn canary_mutation_semantics(id: &str) -> Result<&'static str> {
    match id {
        "C01" => Ok("packet.status := ratified"),
        "C02" => Ok("packet.ratification := approved"),
        "C03" => Ok("packet.locked_acceptance_corpora_touched := true"),
        "C04" => Ok("duplicate requirements[0]"),
        "C05" => Ok("remove test P1-T08"),
        "C06" => Ok("clear tests[0].requirements"),
        "C07" => Ok("remove final mutant"),
        "C08" => Ok("set first 17 mutant operators to predicate-deletion"),
        "C09" => Ok("move first mutant to another family"),
        "C10" => Ok("remove canary C16"),
        "C11" => Ok("set generated[0].accepted := 0"),
        "C12" => Ok("set generated[0].discards := attempts"),
        "C13" => Ok("duplicate one fuzz target across families"),
        "C14" => Ok("remove crash boundary ilrp/before_ack"),
        "C15" => Ok("set reviews[0].attempts := 0"),
        "C16" => Ok("remove baseline packet digest from markdown"),
        "C17" => Ok("requirements[0].source := D18.999"),
        "C18" => Ok("clear tests[0].evidence"),
        "C19" => Ok("requirements[P1-R015].stateful := false"),
        "C20" => Ok("mutants[0].source := not-a-file:0"),
        "C21" => Ok("mutants[0].disposition := invented"),
        "C22" => Ok("duplicate generated[0].seed_categories[0]"),
        "C23" => Ok("generated[0].attempts += 1"),
        "C24" => Ok("crash_boundaries[0].before := false"),
        "C25" => Ok("reviews[1].reviewer := reviews[0].reviewer"),
        "C26" => Ok("qualification.provenance := None"),
        "C27" => Ok("replace retained sanitizer binary marker and invoke runtime proof verifier"),
        "C28" => Ok("generated oracle.independent := false"),
        "C29" => Ok("campaign elapsed_s := 8h+1s"),
        "C30" => Ok("residual_risk.evidence := empty"),
        "C31" => Ok("scope trace command := scope-probe only"),
        "C32" => Ok("replace not-applicable concurrency source_tree with unrelated Git object"),
        "C33" => Ok("crash_boundary_scope.deferred[0].sites -= 1"),
        _ => anyhow::bail!("unknown canary {id}"),
    }
}

/// Run deterministic generated checks for all five HAQP families. This records
/// generated-case evidence only; sanitizer wall-clock qualification remains a
/// separate `cargo fuzz` lane and is never inferred from this command.
/// Deterministic 64-bit stream; one seed per case, recorded per family.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 ^ (self.0 >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    /// A deterministic identifier drawn from the seeded stream.
    ///
    /// M17.5 F-16: the generators minted ids with `NodeId::new()`,
    /// `RepairStepId::new()` and friends, all of which are `Uuid::now_v7()` —
    /// wall-clock milliseconds plus OS randomness. So the `seed` recorded in the
    /// packet, whose stated purpose is to make the campaign reproducible under
    /// ADR-0020 §1, did not reproduce a run: a crash at case 45,231 could not be
    /// replayed from it. The defect stayed invisible because the digest absorbed
    /// only RNG state, so nothing the ids touched ever reached the artifact.
    ///
    /// v5 (namespaced SHA-1 of the seeded bytes) rather than v7, because a
    /// deterministic v7 would have to invent a timestamp. The ids are opaque to
    /// every assertion here; nothing in these families depends on UUID version
    /// or on time ordering. That is a mild strengthening: v7 ids arrive in
    /// ascending creation order, so `topo_order` was only ever exercised on
    /// plans whose id order agreed with insertion order.
    fn uuid(&mut self) -> uuid::Uuid {
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&self.next().to_le_bytes());
        bytes[8..].copy_from_slice(&self.next().to_le_bytes());
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, &bytes)
    }
    fn word(&mut self) -> String {
        const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789-_ ";
        let len = 1 + self.below(12);
        (0..len)
            .map(|_| {
                let idx = usize::try_from(self.below(ALPHABET.len() as u64)).expect("index fits");
                char::from(ALPHABET[idx])
            })
            .collect()
    }

    fn category(&mut self, family: &str) -> &'static str {
        let categories = generated_seed_categories(family);
        let index = usize::try_from(self.below(categories.len() as u64)).expect("category index");
        categories[index]
    }
}

/// Outcome of one generated case: the generator decides acceptance from the
/// candidate itself, so `accepted`/`discards` are MEASURED, never assumed.
enum Case {
    /// In-domain and every metamorphic relation held. Carries a WITNESS of what
    /// the code under test actually produced for this input.
    ///
    /// The witness exists because of M17.5 pass-2 #16. The evidence digest used
    /// to absorb only the generator's RNG state, which made it a fingerprint of
    /// the GENERATOR and provably independent of the code being tested: delete
    /// every `ensure!` in every case function below and, so long as the number
    /// of RNG draws is unchanged, the recorded `evidence_hash` is bit-identical.
    /// Two different formatters would hash the same. A hash that cannot
    /// distinguish the implementation from a no-op is not evidence about the
    /// implementation.
    ///
    /// So each family now returns bytes derived from the output it observed, and
    /// the runner folds those into the digest. An empty witness is refused.
    Accepted {
        category: &'static str,
        witness: Vec<u8>,
    },
    /// Out of the declared domain; not evidence either way.
    /// Out of domain, or a relation that legitimately does not apply. Carries
    /// a WITNESS of WHY, for the same reason `Accepted` does (M17.5 F-30).
    ///
    /// The runner used to record a discard as the single byte `b"D"`, so the
    /// digest absorbed the COUNT of discards and nothing about them. Six
    /// mutants inside `case_repair`'s cycle construction survived on exactly
    /// that: each kept the plan cyclic, so each still discarded, and the
    /// evidence could not tell the shapes apart. ADR-0020 §4 says attempts and
    /// discards are RECORDED; counting is not recording.
    Discarded {
        category: &'static str,
        witness: Vec<u8>,
    },
    /// Declared negative input. Unlike a discard, this case is part of the
    /// exercised input domain and remains visible in campaign accounting.
    Negative {
        category: &'static str,
        witness: Vec<u8>,
    },
}

/// Bounded real ILRP probe used by generated repair cases. The generated lane
/// runs 100k cases; opening a durable store per case would turn evidence into
/// a storage benchmark. One probe per process still executes prepare, apply,
/// acknowledge, finalize, and recovery against the production driver, while
/// every case continues to exercise the transition predicates below.
struct GeneratedIlrpExecutor;

impl liminal_jurisdiction::ExternalExecutor for GeneratedIlrpExecutor {
    // Deliberate bounded smoke executor: production IlrpDriver, graph store,
    // and recovery are exercised; external side effects stay deterministic.
    fn verify(
        &self,
        _mutation: &liminal_jurisdiction::ProposedMutation,
    ) -> Result<liminal_jurisdiction::PrestateMatch, liminal_jurisdiction::IlrpError> {
        Ok(liminal_jurisdiction::PrestateMatch::Prestate)
    }

    fn apply(
        &self,
        mutation: &liminal_jurisdiction::ProposedMutation,
    ) -> Result<liminal_jurisdiction::StepAck, liminal_jurisdiction::IlrpError> {
        Ok(liminal_jurisdiction::StepAck {
            step: mutation.id,
            observed_poststate: mutation.expected_poststate.clone(),
            at: liminal_id::Timestamp::now(),
        })
    }
}

fn generated_ilrp_probe() -> Result<&'static [u8]> {
    static PROBE: std::sync::OnceLock<std::result::Result<Vec<u8>, String>> =
        std::sync::OnceLock::new();
    let result = PROBE.get_or_init(|| {
        let outcome = (|| -> Result<Vec<u8>> {
            let dir = liminal_scratch::ScratchDir::new("haqp-generated-ilrp")?;
            let store = liminal_graph::GraphStore::open(&dir)?;
            let fixed = uuid::Uuid::from_u128(1);
            let step_id = liminal_id::RepairStepId::from_uuid(fixed);
            let mutation = liminal_jurisdiction::ProposedMutation {
                id: step_id,
                subject: liminal_id::JurisdictionSubject::Node(liminal_id::NodeId::from_uuid(
                    fixed,
                )),
                operation: liminal_jurisdiction::RepairOperation::WriteFile {
                    path: liminal_id::PathId("generated-ilrp.md".into()),
                    contents: b"generated-ilrp".to_vec(),
                },
                expected_prestate: liminal_jurisdiction::StatePredicate::Any,
                expected_poststate: liminal_jurisdiction::StatePredicate::Any,
                idempotency_key: liminal_id::IdempotencyKey::from_uuid(fixed),
            };
            let plan = liminal_jurisdiction::RepairPlan {
                id: liminal_id::RepairId::from_uuid(fixed),
                basis: liminal_revision::WorkspaceBasis {
                    transaction: liminal_id::TransactionId::from_uuid(fixed),
                    perspective: liminal_revision::BasisPerspective::DurableOnly,
                    components: BTreeMap::new(),
                },
                steps: BTreeMap::from([(step_id, mutation)]),
                dependencies: Vec::new(),
                inverse: None,
            };
            let driver = liminal_jurisdiction::IlrpDriver {
                store: &store,
                executor: GeneratedIlrpExecutor,
                crash: liminal_jurisdiction::NoCrash,
            };
            let id = driver.prepare(
                plan,
                liminal_jurisdiction::SafetyEvidence::StructurallyDisjoint {
                    description: "haqp-generated-ilrp".into(),
                },
            )?;
            let state = driver.run(id)?;
            anyhow::ensure!(state == liminal_jurisdiction::IntentState::Committed);
            let recovered = driver.recover_all()?;
            anyhow::ensure!(
                recovered.is_empty(),
                "committed ILRP probe must recover no intents"
            );
            Ok(format!(
                "{state:?}:{}",
                liminal_jurisdiction::CrashPoint::all().len()
            )
            .into_bytes())
        })();
        outcome.map_err(|error| error.to_string())
    });
    result
        .as_ref()
        .map(Vec::as_slice)
        .map_err(|error| anyhow::anyhow!("generated ILRP probe failed: {error}"))
}

/// Family 0 — source/CST/formatting. Metamorphic relations: **lossless
/// emit** (the CST must reproduce its input byte-for-byte) and **format
/// idempotence**, both compared over bytes rather than parsed values.
fn case_source_cst(rng: &mut Rng) -> Result<Case> {
    // Blind pass 1 at aa00d41 (A01): a formatter error became a Negative case
    // whatever the category, so a formatter that lost support for nested or
    // deep documents reclassified them as negatives and the run still reached
    // its accepted target from the categories that still worked. Only the
    // categories that DECLARE malformed input may be refused.
    const REFUSABLE: [&str; 5] = [
        "truncated",
        "unterminated",
        "control-byte",
        "invalid-utf8",
        "hostile",
    ];
    let category = rng.category("source/CST/formatting");
    let token = rng.word();
    let bytes = match category {
        "empty" => Vec::new(),
        "single-token" => token.as_bytes().to_vec(),
        "whitespace" => b" \t\n".to_vec(),
        "unicode" => format!("{token} βeta").into_bytes(),
        "long-line" => format!("{}-{token}", token.repeat(64)).into_bytes(),
        "truncated" => format!("{token} {{#").into_bytes(),
        "unterminated" => format!("{token} {{#{token}").into_bytes(),
        "nested" => format!("{token} {{#outer {{#{token}}}}}").into_bytes(),
        "deep" => format!("{} {token}", "{#".repeat(24)).into_bytes(),
        "wide" => (0..32)
            .map(|_| token.clone())
            .collect::<Vec<_>>()
            .join(" ")
            .into_bytes(),
        "control-byte" => format!("{token}\x01{token}").into_bytes(),
        "invalid-utf8" => vec![0xff, 0xfe],
        "comment" => format!("{token} <!-- {token} -->").into_bytes(),
        "escape" => format!(r"{token} \ {token}").into_bytes(),
        // Blind pass 1 at `ec045588` (A01): every generated source held at
        // most ONE coarse block, so the per-document hash relation F-60 added
        // — equal text hashes alike, different text does not — had nothing to
        // relate and a constant hash passed 100,000 cases. A blank line makes
        // two blocks, and repeating the token makes them equal, which is the
        // arm a constant hash satisfies and a text-blind one does not.
        "boundary-offset" => format!("{token}\n\n{token}").into_bytes(),
        "hostile" => format!("{token}\0\x7f").into_bytes(),
        other => unreachable!("unknown source category {other}"),
    };
    let Ok(source) = String::from_utf8(bytes.clone()) else {
        let basis = liminal_source::SourceBasis {
            source: liminal_id::SourceId::from_name("haqp-generated"),
            content_hash: liminal_id::ContentHash::of(&bytes),
        };
        anyhow::ensure!(
            liminal_source::Utf8HolderView::from_bytes(basis, &bytes).is_err(),
            "invalid-utf8 source unexpectedly entered Utf8HolderView"
        );
        return Ok(Case::Negative {
            category,
            witness: bytes,
        });
    };
    // Out of domain: the compact surface has no all-whitespace document.
    if source.trim().is_empty() {
        return Ok(Case::Negative {
            category,
            witness: format!("whitespace-source:{source:?}").into_bytes(),
        });
    }
    let basis = liminal_source::SourceBasis {
        source: liminal_id::SourceId::from_name("haqp-generated"),
        content_hash: liminal_id::ContentHash::of(source.as_bytes()),
    };
    let Ok(view) = liminal_source::Utf8HolderView::from_bytes(basis, source.as_bytes()) else {
        return Ok(Case::Negative {
            category,
            witness: source.into_bytes(),
        });
    };
    let cst = liminal_cst::parse(&view);
    let emitted = cst.emit_lossless();
    // Blind pass 1 at 9eca4f0 (A01): the campaign exercised `parse` only, so
    // `coarse_parse` — the other public entry point, the one that must never
    // reject — could return empty block metadata for every input and no
    // generated case would notice.
    let coarse = liminal_cst::coarse_parse(&source);
    let fmt = liminal_format::MarkdownFormatter::default();
    let Ok(once) = fmt.format(&source) else {
        anyhow::ensure!(
            REFUSABLE.contains(&category),
            "the formatter refused valid-domain category {category}: a refusal there is lost \
             support, not a negative case (source {source:?})"
        );
        return Ok(Case::Negative {
            category,
            witness: source.into_bytes(),
        });
    };
    let Ok(twice) = fmt.format(&once) else {
        anyhow::ensure!(
            REFUSABLE.contains(&category),
            "the formatter refused its own output for valid-domain category {category}: \
             formatting is not total there (source {source:?})"
        );
        return Ok(Case::Negative {
            category,
            witness: once.into_bytes(),
        });
    };
    let witness = independent_oracle_source_cst(&source, &emitted, &once, &twice, &token, &coarse)?;
    Ok(Case::Accepted { category, witness })
}

/// Family 1 — graph/interchange codecs.
///
/// Metamorphic relation: **byte-canonical stability**. Re-serializing a value
/// decoded from canonical bytes must reproduce those bytes exactly. The
/// comparison is over BYTES, never the type's own `PartialEq`, so a broken
/// `Eq` cannot make this pass (ADR-0020 §5).
fn case_interchange(rng: &mut Rng) -> Result<Case> {
    use liminal_graph::Node;

    let category = rng.category("graph/interchange codecs");
    let malformed = matches!(
        category,
        "empty"
            | "duplicate-id"
            | "missing-node"
            | "cycle"
            | "unknown-kind"
            | "external-value"
            | "comment"
            | "truncated-json"
            | "invalid-json"
            | "hostile"
    );
    if malformed {
        let raw = match category {
            "empty" => Vec::new(),
            "duplicate-id" => br#"{"id":"00000000-0000-0000-0000-000000000001","id":"00000000-0000-0000-0000-000000000002","kind":1,"payload":"none","revision":0,"flags":0}"#.to_vec(),
            "missing-node" => br"{}".to_vec(),
            "cycle" => br#"{"id":"00000000-0000-0000-0000-000000000001","kind":1,"payload":{"cycle":["a","b","a"]},"revision":0,"flags":0}"#.to_vec(),
            "unknown-kind" => br#"{"id":"00000000-0000-0000-0000-000000000001","kind":"unknown","payload":"none","revision":0,"flags":0}"#.to_vec(),
            "external-value" => br#"{"id":"00000000-0000-0000-0000-000000000001","kind":1,"payload":{"external-value":"source"},"revision":0,"flags":0}"#.to_vec(),
            "comment" => br#"{"id":"00000000-0000-0000-0000-000000000001","kind":1,"payload":{"comment":"note"},"revision":0,"flags":0}"#.to_vec(),
            "truncated-json" => format!(r#"{{"category":"{category}""#).into_bytes(),
            "invalid-json" => {
                let mut bytes = format!("{category}\0").into_bytes();
                bytes.push(0xff);
                bytes
            }
            "hostile" => vec![0, 0xff, 0x7f],
            _ => unreachable!("unknown graph negative category {category}"),
        };
        anyhow::ensure!(
            serde_json::from_slice::<Node>(&raw).is_err(),
            "graph negative category {category} unexpectedly decoded"
        );
        let witness = if raw.is_empty() {
            format!("{category}:empty").into_bytes()
        } else {
            raw
        };
        return Ok(Case::Negative { category, witness });
    }
    // Blind pass 1 at 612cbcc (A07): every accepted category built one Node
    // with the category's name leaked into its payload, and a Node is not even
    // the codec's unit — the fuzz target decodes a Transaction. Each category
    // now builds the transaction shape it names, and the oracle counts that
    // shape in the encoded JSON independently of the codec's own equality.
    let text = format!("{category}:{}", rng.word());
    let durable = rng.below(2) == 1;
    if matches!(category, "single-node") && durable && text.trim().is_empty() {
        return Ok(Case::Discarded {
            category,
            witness: format!("durable-without-payload:{text:?}").into_bytes(),
        });
    }
    let (txn, shape) = interchange_transaction(category, rng, &text, durable);
    let once = serde_json::to_vec(&txn)?;
    let decoded: liminal_graph::Transaction = serde_json::from_slice(&once)?;
    let twice = serde_json::to_vec(&decoded)?;
    let witness = independent_oracle_interchange(&txn, &decoded, &once, &twice, shape)?;
    Ok(Case::Accepted { category, witness })
}

/// The node and relation counts a category promises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InterchangeShape {
    nodes: usize,
    relations: usize,
    payload_bytes_at_least: usize,
}

/// A transaction with the shape its category names: `single-edge` is two nodes
/// and one relation, `dag` a diamond, `wide` a root with eight children, `deep`
/// a chain of twelve, `large-payload` one node carrying 64 KiB of text.
fn interchange_transaction(
    category: &str,
    rng: &mut Rng,
    text: &str,
    durable: bool,
) -> (liminal_graph::Transaction, InterchangeShape) {
    use liminal_graph::{
        Node, NodeFlags, Operation, Origin, PayloadRef, Relation, RelationFlags, Target,
        Transaction, TxnMeta,
    };
    let node = |rng: &mut Rng, payload: PayloadRef, durable: bool| Node {
        id: liminal_id::NodeId::from_uuid(rng.uuid()),
        kind: liminal_graph::KindId(u32::try_from(rng.below(8)).expect("kind fits")),
        payload,
        revision: liminal_id::RevisionId(rng.below(1_000)),
        flags: if durable {
            NodeFlags::HAS_DURABLE_ID
        } else {
            NodeFlags::default()
        },
    };
    let relation =
        |rng: &mut Rng, source: liminal_id::NodeId, target: liminal_id::NodeId| Relation {
            id: liminal_id::RelationId::from_uuid(rng.uuid()),
            source,
            target: Target::Node(target),
            kind: liminal_graph::KindId(u32::try_from(rng.below(8)).expect("kind fits")),
            payload: PayloadRef::None,
            revision: liminal_id::RevisionId(0),
            flags: RelationFlags(0),
            requires: None,
        };
    let (nodes, edges, payload_floor): (Vec<Node>, Vec<(usize, usize)>, usize) = match category {
        "single-edge" => (
            vec![
                node(rng, PayloadRef::Text(text.to_owned()), durable),
                node(rng, PayloadRef::None, false),
            ],
            vec![(0, 1)],
            0,
        ),
        "dag" => (
            (0..4).map(|_| node(rng, PayloadRef::None, false)).collect(),
            vec![(0, 1), (0, 2), (1, 3), (2, 3)],
            0,
        ),
        "wide" => (
            (0..9).map(|_| node(rng, PayloadRef::None, false)).collect(),
            (1..9).map(|child| (0, child)).collect(),
            0,
        ),
        "deep" => (
            (0..12)
                .map(|_| node(rng, PayloadRef::None, false))
                .collect(),
            (0..11).map(|index| (index, index + 1)).collect(),
            0,
        ),
        "large-payload" => (
            vec![node(
                rng,
                PayloadRef::Text(text.repeat(65_536 / text.len().max(1) + 1)),
                durable,
            )],
            Vec::new(),
            65_536,
        ),
        // `single-node` and every category the seed table lists without a
        // dedicated shape.
        _ => {
            let payload = if rng.below(4) == 0 {
                PayloadRef::None
            } else {
                PayloadRef::Text(text.to_owned())
            };
            (vec![node(rng, payload, durable)], Vec::new(), 0)
        }
    };
    let shape = InterchangeShape {
        nodes: nodes.len(),
        relations: edges.len(),
        payload_bytes_at_least: payload_floor,
    };
    let ids = nodes.iter().map(|n| n.id).collect::<Vec<_>>();
    let mut ops = nodes
        .into_iter()
        .map(|node| Operation::CreateNode { node })
        .collect::<Vec<_>>();
    for (from, to) in edges {
        ops.push(Operation::AddRelation {
            relation: relation(rng, ids[from], ids[to]),
        });
    }
    let txn = Transaction {
        id: liminal_id::TransactionId::from_uuid(rng.uuid()),
        parent: liminal_id::GraphRevisionId(rng.below(1_000)),
        meta: TxnMeta {
            actor: None,
            origin: Origin::Human,
            at: liminal_id::Timestamp(i64::try_from(rng.below(1_000_000)).expect("fits")),
            provenance: None,
            inverse: None,
        },
        ops,
    };
    (txn, shape)
}

/// Independent oracle for the interchange codec: structural round-trip,
/// byte-stable re-encoding, and the promised shape counted in the encoded JSON
/// by a parser that is not the codec (serde_json::Value), so a codec that
/// silently dropped an operation could not certify itself.
fn independent_oracle_interchange(
    txn: &liminal_graph::Transaction,
    decoded: &liminal_graph::Transaction,
    once: &[u8],
    twice: &[u8],
    shape: InterchangeShape,
) -> Result<Vec<u8>> {
    anyhow::ensure!(
        txn == decoded,
        "interchange round trip changed the transaction"
    );
    anyhow::ensure!(once == twice, "interchange encoding is not byte-stable");
    let value: serde_json::Value = serde_json::from_slice(once)?;
    let ops = value["ops"]
        .as_array()
        .context("encoded transaction has no ops array")?;
    let count = |key: &str| ops.iter().filter(|op| op.get(key).is_some()).count();
    anyhow::ensure!(
        count("create-node") == shape.nodes && count("add-relation") == shape.relations,
        "encoded shape differs from the category's promise: {shape:?}"
    );
    // Blind pass 1 at f360e90 (A02): a renamed field round-trips through the
    // same serde on both sides. The published schema is a closed key set.
    // Sorted on this side: the pin is "no field added, removed or renamed",
    // not the encoder's key order.
    let keys = |value: &serde_json::Value| -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort_unstable();
        keys
    };
    anyhow::ensure!(
        keys(&value) == ["id", "meta", "ops", "parent"],
        "encoded transaction keys are not the published schema: {:?}",
        keys(&value)
    );
    for op in ops {
        if let Some(node) = op.pointer("/create-node/node") {
            anyhow::ensure!(
                keys(node) == ["flags", "id", "kind", "payload", "revision"],
                "encoded node keys are not the published schema: {:?}",
                keys(node)
            );
        }
        if let Some(relation) = op.pointer("/add-relation/relation") {
            anyhow::ensure!(
                keys(relation)
                    == [
                        "flags", "id", "kind", "payload", "requires", "revision", "source",
                        "target"
                    ],
                "encoded relation keys are not the published schema: {:?}",
                keys(relation)
            );
        }
    }
    let payload_bytes = ops
        .iter()
        .filter_map(|op| op.pointer("/create-node/node/payload/text"))
        .filter_map(serde_json::Value::as_str)
        .map(str::len)
        .max()
        .unwrap_or(0);
    anyhow::ensure!(
        payload_bytes >= shape.payload_bytes_at_least,
        "encoded payload {payload_bytes} bytes is under the category's floor"
    );
    Ok(once.to_vec())
}

/// Family 2 — transforms/projections.
///
/// Metamorphic relations, both independent of the merge's own equality:
/// **identity** (a side that changed nothing must not perturb the other
/// side's content) and **outcome-class symmetry** (swapping `ours`/`theirs`
/// cannot change whether the merge was structurally disjoint).
/// The input a transform negative category names (A03). `empty` is the empty
/// side; `invalid-span` is a marker that never closes and one with whitespace
/// inside; `truncated` is a block cut in the middle of its marker; `hostile`
/// carries a NUL, a bidi override, a nested marker and an over-long marker.
fn transform_negative_input(category: &str, rng: &mut Rng) -> String {
    let word = rng.word();
    match category {
        "empty" => String::new(),
        "invalid-span" => format!("{word} {{#open\n\n{word} {{# spaced }}"),
        "truncated" => format!("{word} {{#full}}\n\n{word} {{#cu"),
        "hostile" => format!(
            "{word}\0 {{#\u{202e}}} {{#{{#nested}}}} {{#{}}}",
            "x".repeat(4096)
        ),
        other => format!("{other}:{word}"),
    }
}

fn case_transform(rng: &mut Rng) -> Result<Case> {
    use liminal_source::merge::three_way;

    let category = rng.category("transforms/projections");
    if matches!(category, "empty" | "invalid-span" | "truncated" | "hostile") {
        // Negative inputs still cross the production merge implementation.
        // Blind pass 1 at 612cbcc (A03): every category used to become an
        // ordinary label string, so the "negative" tested only the generator's
        // branch. Each category now constructs the shape it names.
        let malformed = transform_negative_input(category, rng);
        let outcome = three_way("{#negative}", &malformed, "{#negative}");
        // Totality is the property: the merge returned rather than panicked.
        // A disjoint outcome must carry the malformed side verbatim; any other
        // outcome class is the transform refusing the shape, recorded as such.
        if let liminal_source::merge::MergeOutcome::Disjoint { merged } = &outcome {
            // The merge normalizes layout (base slot order, block spacing), so
            // verbatim containment is not the property; every token of the
            // malformed side surviving is.
            let mut missing = malformed.split_whitespace().collect::<Vec<_>>();
            for token in merged.split_whitespace() {
                if let Some(index) = missing.iter().position(|m| *m == token) {
                    missing.swap_remove(index);
                }
            }
            anyhow::ensure!(
                missing.is_empty(),
                "transform negative category {category} lost malformed input tokens {missing:?}"
            );
        }
        let witness = format!("transform-negative:{category}:{outcome:?}").into_bytes();
        return Ok(Case::Negative { category, witness });
    }
    let blocks = match category {
        "deep" => 12,
        "wide" | "large-patch" => 8,
        _ => 1 + rng.below(4),
    };
    let base = (0..blocks)
        .map(|i| format!("{} {{#b{i}}}", rng.word()))
        .collect::<Vec<_>>()
        .join("\n\n");
    // Out of domain: a degenerate base has no slots to align.
    if base.trim().is_empty() {
        return Ok(Case::Negative {
            category,
            witness: format!("degenerate-base:{base:?}").into_bytes(),
        });
    }
    let token = rng.word();
    let mut lines = base.lines().map(str::to_owned).collect::<Vec<_>>();
    let first = lines.first().cloned().unwrap_or_default();
    let second = lines.get(1).cloned().unwrap_or_else(|| first.clone());
    let (ours, theirs) = match category {
        "identity" | "deep" | "wide" | "large-patch" => (base.clone(), base.clone()),
        "insert" => (format!("{token}\n\n{base}"), base.clone()),
        "delete" => (
            lines.iter().skip(1).cloned().collect::<Vec<_>>().join("\n"),
            base.clone(),
        ),
        "replace" => (
            base.replacen(&first, &format!("{token} {first}"), 1),
            base.clone(),
        ),
        "move" => {
            lines.reverse();
            (lines.join("\n"), base.clone())
        }
        "overlap" => (
            base.replacen(&first, &format!("ours {first}"), 1),
            base.replacen(&first, &format!("theirs {first}"), 1),
        ),
        "commute" => (
            base.replacen(&first, &format!("ours {first}"), 1),
            base.replacen(&second, &format!("theirs {second}"), 1),
        ),
        "conflict" => (
            base.replacen(&first, &format!("left {first}"), 1),
            base.replacen(&first, &format!("right {first}"), 1),
        ),
        "boundary-span" => (format!("\n{base}\n"), base.clone()),
        other => unreachable!("unknown transform category {other}"),
    };

    // Identity: theirs unchanged => the merge must carry ours' content.
    //
    // M17.5 pass-2 #9: this used to be `if let Disjoint { merged } = ...`, so an
    // implementation that returned `Conflict` for EVERY input never entered the
    // branch and the relation held vacuously — the metamorphic check tested
    // nothing at all. A side that changed nothing cannot conflict with anything,
    // so `Disjoint` is a REQUIREMENT here, not a case to handle.
    let identity = three_way(&base, &ours, &base);
    let forward = three_way(&base, &ours, &theirs);
    let swapped = three_way(&base, &theirs, &ours);
    let witness = independent_oracle_source_transform(&base, &ours, &identity, &forward, &swapped)?;
    Ok(Case::Accepted { category, witness })
}

/// Family 3 — repair/ILRP/recovery.
///
/// Builds a real `RepairPlan` DAG and orders it. Metamorphic relation:
/// **deterministic permutation** — shuffling the dependency list must not
/// change the resulting order. The ordering is then verified against the
/// generator's OWN edge set, not by asking the implementation again. Cyclic
/// candidates are out of domain and are discarded, which is where this
/// family's discard rate genuinely comes from.
#[allow(
    clippy::too_many_lines,
    reason = "repair generator keeps ILRP probe, DAG construction, and oracle witness together"
)]
fn case_repair(rng: &mut Rng) -> Result<Case> {
    use liminal_jurisdiction::repair::{
        ProposedMutation, RepairDependency, RepairOperation, RepairPlan, StatePredicate, topo_order,
    };

    let category = rng.category("repair/ILRP/recovery");
    let probe = generated_ilrp_probe()?;
    let state_witness = match category {
        "applying" => liminal_jurisdiction::IntentState::Applying,
        "external-applied" => liminal_jurisdiction::IntentState::ExternalApplied,
        "finalizing" => liminal_jurisdiction::IntentState::Finalizing,
        "committed" => liminal_jurisdiction::IntentState::Committed,
        "needs-review" | "contested" => liminal_jurisdiction::IntentState::NeedsReview,
        "boundary" => liminal_jurisdiction::IntentState::Aborted,
        _ => liminal_jurisdiction::IntentState::Prepared,
    };
    let legal_next = match state_witness {
        liminal_jurisdiction::IntentState::Prepared => liminal_jurisdiction::IntentState::Applying,
        liminal_jurisdiction::IntentState::Applying => {
            liminal_jurisdiction::IntentState::ExternalApplied
        }
        liminal_jurisdiction::IntentState::ExternalApplied => {
            liminal_jurisdiction::IntentState::Finalizing
        }
        liminal_jurisdiction::IntentState::Finalizing => {
            liminal_jurisdiction::IntentState::Committed
        }
        _ => liminal_jurisdiction::IntentState::Committed,
    };
    anyhow::ensure!(
        state_witness.is_terminal() || state_witness.may_transition_to(legal_next),
        "declared ILRP state has no legal transition"
    );
    // Blind pass 1 at f360e90 (A01): every category built WriteFile steps whose
    // contents carried the category's name. Each category now builds the
    // shape it names; the refusal categories are refused by `topo_order`
    // itself and recorded as negatives below.
    let count = match category {
        "two-step" => 2,
        "boundary" => 6,
        _ => usize::try_from(2 + rng.below(5)).expect("step count fits"),
    };
    let ids: Vec<liminal_id::RepairStepId> = (0..count)
        .map(|_| liminal_id::RepairStepId::from_uuid(rng.uuid()))
        .collect();
    let steps: BTreeMap<_, _> = ids
        .iter()
        .map(|id| {
            let contents = match category {
                "hostile" => {
                    let mut bytes = format!("{}\0\u{202e}", rng.word()).into_bytes();
                    bytes.extend(std::iter::repeat_n(0xff_u8, 65_536));
                    bytes
                }
                _ => rng.word().into_bytes(),
            };
            let path = liminal_id::PathId(if category == "hostile" {
                "../../etc/passwd\0".into()
            } else {
                "generated.md".into()
            });
            let operation = if category == "graph-step" {
                RepairOperation::Graph(liminal_graph::Operation::CreateNode {
                    node: liminal_graph::Node {
                        id: liminal_id::NodeId::from_uuid(rng.uuid()),
                        kind: liminal_graph::KindId(1),
                        payload: liminal_graph::PayloadRef::Text(
                            String::from_utf8_lossy(&contents).into_owned(),
                        ),
                        revision: liminal_id::RevisionId(0),
                        flags: liminal_graph::NodeFlags::default(),
                    },
                })
            } else {
                RepairOperation::WriteFile {
                    path: path.clone(),
                    contents: contents.clone(),
                }
            };
            let expected_poststate = if category == "poststate" {
                StatePredicate::FileContent {
                    path,
                    hash: liminal_id::ContentHash(*blake3::hash(&contents).as_bytes()),
                }
            } else {
                StatePredicate::Any
            };
            (
                *id,
                ProposedMutation {
                    id: *id,
                    subject: liminal_id::JurisdictionSubject::Node(liminal_id::NodeId::from_uuid(
                        rng.uuid(),
                    )),
                    operation,
                    expected_prestate: StatePredicate::Any,
                    expected_poststate,
                    idempotency_key: liminal_id::IdempotencyKey::from_uuid(rng.uuid()),
                },
            )
        })
        .collect();

    let mut edges = Vec::new();
    if category == "two-step" {
        edges.push((0, 1));
    } else {
        for i in 0..count {
            for j in (i + 1)..count {
                if rng.below(3) == 0 {
                    edges.push((i, j));
                }
            }
        }
    }
    if category == "duplicate-ack" {
        // The same dependency acknowledged twice: the plan orders as if once.
        let first = edges.first().copied().unwrap_or((0, 1));
        edges.push(first);
        edges.push(first);
    }
    // Rarely close a genuine cycle. A lone back edge is NOT a cycle unless a
    // forward path exists, so add both directions to guarantee one.
    let cyclic = count >= 2 && (category == "cycle" || rng.below(384) == 0);
    if cyclic {
        edges.retain(|(before, after)| !(*before == 0 && *after == count - 1));
        edges.push((0, count - 1));
        edges.push((count - 1, 0));
    }
    let mut dependencies: Vec<RepairDependency> = edges
        .iter()
        .map(|(before, after)| RepairDependency {
            before: ids[*before],
            after: ids[*after],
        })
        .collect();
    let mut steps = steps;
    match category {
        // A dependency on a step the plan never declares.
        "missing-step" => dependencies.push(RepairDependency {
            before: ids[0],
            after: liminal_id::RepairStepId::from_uuid(rng.uuid()),
        }),
        // The plan's steps were cut short while its dependencies still name
        // the lost tail.
        "truncated-intent" => {
            let lost = ids[count - 1];
            dependencies.push(RepairDependency {
                before: ids[0],
                after: lost,
            });
            steps.remove(&lost);
        }
        _ => {}
    }
    let plan = RepairPlan {
        id: liminal_id::RepairId::from_uuid(rng.uuid()),
        basis: liminal_revision::WorkspaceBasis {
            transaction: liminal_id::TransactionId::from_uuid(rng.uuid()),
            perspective: liminal_revision::BasisPerspective::DurableOnly,
            components: BTreeMap::new(),
        },
        steps,
        dependencies: dependencies.clone(),
        inverse: None,
    };
    let order = match topo_order(&plan) {
        Ok(order) => order,
        Err(error) => {
            // Refused plans are not acceptance evidence. The witness carries
            // the EDGES and the refusal, so the shape reaches the digest —
            // without it every refusal looks alike (M17.5 F-30). A category
            // that promised a refusal gets a negative; a random cycle in an
            // accepted category is out of domain.
            let witness = [
                format!("refused:{error}:{edges:?}:{state_witness:?}:"),
                String::from_utf8_lossy(probe).into_owned(),
            ]
            .concat()
            .into_bytes();
            let promised = match category {
                "cycle" => matches!(
                    error,
                    liminal_jurisdiction::repair::CycleError::Cycle { .. }
                ),
                "missing-step" | "truncated-intent" => {
                    matches!(
                        error,
                        liminal_jurisdiction::repair::CycleError::UnknownStep { .. }
                    )
                }
                _ => false,
            };
            return Ok(if promised {
                Case::Negative { category, witness }
            } else if matches!(category, "cycle" | "missing-step" | "truncated-intent") {
                anyhow::bail!(
                    "repair category {category} was refused for the wrong reason: {error}"
                )
            } else {
                Case::Discarded { category, witness }
            });
        }
    };
    anyhow::ensure!(
        !matches!(category, "missing-step" | "truncated-intent"),
        "repair category {category} promised a refusal and was ordered instead"
    );
    anyhow::ensure!(
        !cyclic,
        "a cyclic plan was ordered instead of refused: {dependencies:?}"
    );

    // Deterministic permutation: reversing the edge list must not change it.
    let mut permuted = plan.clone();
    permuted.dependencies.reverse();
    let permuted_order = topo_order(&permuted).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut witness = independent_oracle_source_repair(&order, &ids, &edges, &permuted_order)?;
    witness.extend_from_slice(format!("{state_witness:?}").as_bytes());
    witness.extend_from_slice(probe);
    Ok(Case::Accepted { category, witness })
}

/// Family 4 — Basis/revision/query invalidation.
///
/// Metamorphic relations: **irrelevant-input invariance** (a key never read
/// must never invalidate) and **monotonicity** (recording more reads can only
/// add invalidations). Both are checked against a key set the generator built
/// itself, so `ComponentDeps` is never its own oracle.
#[allow(clippy::too_many_lines)]
fn case_invalidation(rng: &mut Rng) -> Result<Case> {
    use liminal_revision::{
        BasisComponent, BasisPerspective, CausalFrontier, ComponentDeps, WorkspaceBasis,
    };

    let category = rng.category("Basis/revision/query invalidation");
    let perspective = match category {
        "client-scoped" => BasisPerspective::ClientScoped {
            client: liminal_id::ClientId::from_uuid(rng.uuid()),
        },
        "published" => BasisPerspective::Published {
            revision: liminal_id::PublicationId::from_uuid(rng.uuid()),
        },
        "federated" => BasisPerspective::Federated {
            domain: liminal_id::FederationId::from_uuid(rng.uuid()),
            frontier: CausalFrontier(vec![
                u8::try_from(rng.below(255)).expect("frontier byte fits"),
            ]),
        },
        _ => BasisPerspective::DurableOnly,
    };
    let basis = WorkspaceBasis {
        transaction: liminal_id::TransactionId::from_uuid(rng.uuid()),
        perspective,
        components: if category == "empty-basis" {
            BTreeMap::new()
        } else {
            BTreeMap::from([(
                liminal_revision::graph_key(),
                BasisComponent::GraphSnapshot {
                    revision: liminal_id::GraphRevisionId(rng.below(10_000)),
                },
            )])
        },
    };
    let basis_witness = serde_json::to_vec(&basis)?;
    let malformed = matches!(
        category,
        "truncated" | "hostile" | "invalid-token" | "unknown-perspective"
    );
    // Target the declared domain (computations that read >=1 component) and
    // keep a rare out-of-domain probe so the discard path stays exercised.
    let read_count = if category == "empty-basis" || rng.below(384) == 0 {
        0
    } else {
        1 + rng.below(5)
    };
    let mut deps = ComponentDeps::default();
    let mut expected = BTreeSet::new();
    for _ in 0..read_count {
        let key = liminal_id::JurisdictionKey::Path(liminal_id::PathId(
            format!("{category}/{}", rng.word()).into(),
        ));
        deps.record(key.clone());
        expected.insert(key);
    }
    if malformed {
        let key = liminal_id::JurisdictionKey::Path(liminal_id::PathId(
            format!("{category}/__haqp_negative").into(),
        ));
        deps.record(key.clone());
        let invalidated = deps.invalidated_by(&key);
        let raw = match category {
            "truncated" => b"{".to_vec(),
            "hostile" => vec![0xff, 0x00, 0x7f],
            "invalid-token" => br#"{"perspective":"unknown"}"#.to_vec(),
            "unknown-perspective" => br#"{"perspective":{"future":true}}"#.to_vec(),
            _ => unreachable!("malformed invalidation category {category}"),
        };
        anyhow::ensure!(
            serde_json::from_slice::<WorkspaceBasis>(&raw).is_err(),
            "invalidation malformed category {category} unexpectedly decoded"
        );
        return Ok(Case::Negative {
            category,
            witness: [
                format!("basis-negative:{category}:{invalidated:?}:").into_bytes(),
                raw,
                basis_witness,
            ]
            .concat(),
        });
    }
    // Out of domain: nothing was read, so invalidation is vacuous.
    if expected.is_empty() {
        return Ok(Case::Negative {
            category,
            witness: [b"vacuous-invalidation:".to_vec(), basis_witness].concat(),
        });
    }

    // Select a negative witness outside the generated read set. A random
    // `unread/<word>` can collide with a legitimate read key, making the only
    // negative assertion disappear exactly when the generator hits that case.
    let unrelated = (0..u32::MAX)
        .map(|index| {
            liminal_id::JurisdictionKey::Path(liminal_id::PathId(
                format!("unread/__haqp_negative_{index}").into(),
            ))
        })
        .find(|candidate| !expected.contains(candidate))
        .expect("finite generated read set leaves a negative witness");
    let extra = liminal_id::JurisdictionKey::Path(liminal_id::PathId(
        format!("extra/{category}/__haqp_0").into(),
    ));
    anyhow::ensure!(
        !expected.contains(&extra),
        "extra dependency key collided with generated read set"
    );
    let mut witness =
        independent_oracle_source_invalidation(&mut deps, &expected, &unrelated, &extra)?;
    witness.extend_from_slice(&basis_witness);
    Ok(Case::Accepted { category, witness })
}

/// One generated-evidence family: its name and its case runner.
type Family = (&'static str, fn(&mut Rng) -> Result<Case>);

/// Run the five HAQP generated-evidence families with MEASURED acceptance.
///
/// Each family generates structured candidates, decides acceptance from the
/// candidate itself, and checks at least one metamorphic relation that does
/// not route through the implementation's own equality path. Counts are
/// tallied from outcomes — never echoed from the requested case count
/// (M17.5 finding F-07).
pub fn run_generated_repo(root: &Utf8Path, cases: u64) -> Result<()> {
    let (evidence, timings) = generate_evidence(cases)?;
    let artifact = GeneratedEvidenceArtifact {
        schema_version: "haqp-generated-v3-negative-categories".to_owned(),
        artifact_blake3: generated_artifact_digest(&evidence)?,
        rows: evidence.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&artifact)?;
    let path = root.join("conformance/haqp/evidence/generated.json");
    fs::create_dir_all(path.parent().expect("evidence parent"))?;
    fs::write(&path, &bytes)?;
    for (row, elapsed_ms) in evidence.iter().zip(&timings) {
        #[allow(clippy::cast_precision_loss, reason = "reporting only")]
        let rate = row.discards as f64 * 100.0 / row.attempts.max(1) as f64;
        println!(
            "{}: {} accepted / {} attempts ({} discarded, {rate:.2}%) in {elapsed_ms}ms",
            row.family, row.accepted, row.attempts, row.discards
        );
    }
    println!(
        "generated evidence written to {path}; blake3={}",
        blake3::hash(&bytes).to_hex()
    );
    Ok(())
}

/// Run the five families and return their evidence rows plus per-family
/// wall-clock timings.
///
/// The timings are returned SEPARATELY rather than embedded in the rows on
/// purpose: the rows are the hashable artifact and must be a pure function of
/// the seed and the code under test, while the timings are telemetry for the
/// operator (M17.5 pass-2 #18).
///
/// The case-count guard is a separate function so its BOUNDARIES are testable:
/// `>` -> `>=` at the safety limit can only be discriminated by a case that
/// runs at exactly 10,000,000, which no test can afford. Extracting the
/// predicate makes the boundary checkable in microseconds (M17.5 F-30).
/// Reject a case count that is zero or beyond the safety limit.
fn validate_case_count(cases: u64) -> Result<()> {
    if cases == 0 {
        anyhow::bail!("generated case count must be positive");
    }
    if cases > 10_000_000 {
        anyhow::bail!("generated case count exceeds safety limit: {cases} > 10000000");
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "evidence runner keeps deterministic accounting and digest boundaries together"
)]
fn generate_evidence(cases: u64) -> Result<(Vec<GeneratedEvidence>, Vec<u64>)> {
    validate_case_count(cases)?;
    let families: [Family; 5] = [
        ("source/CST/formatting", case_source_cst),
        ("graph/interchange codecs", case_interchange),
        ("transforms/projections", case_transform),
        ("repair/ILRP/recovery", case_repair),
        ("Basis/revision/query invalidation", case_invalidation),
    ];

    let mut evidence = Vec::new();
    let mut timings = Vec::new();
    for (index, (family, run)) in families.into_iter().enumerate() {
        let seed = 0x9E37_79B9_7F4A_7C15_u64 ^ (index as u64).wrapping_mul(0x0100_0000_01B3);
        let mut rng = Rng(seed);
        let started = Instant::now();
        let mut digest = blake3::Hasher::new();
        digest.update(&seed.to_le_bytes());
        // ADR-0020 §4 requires >=100,000 ACCEPTED cases, so run until the
        // acceptance target is met rather than stopping at N attempts. The
        // attempt cap keeps a badly-targeted generator from running forever
        // instead of silently reporting a short campaign.
        let (mut accepted, mut discards, mut negatives, mut attempts) = (0u64, 0u64, 0u64, 0u64);
        let mut category_counts = BTreeMap::<String, u64>::new();
        // Negative categories are exercised inputs, not discards; families
        // with broad malformed coverage need a larger attempt budget to reach
        // their accepted floor without silently dropping categories.
        let attempt_cap = cases.saturating_mul(4);
        while accepted < cases {
            anyhow::ensure!(
                attempts < attempt_cap,
                "{family}: {attempts} attempts yielded only {accepted}/{cases} accepted \
                 — the generator is not targeting its declared domain"
            );
            attempts += 1;
            match run(&mut rng).with_context(|| format!("{family} attempt {attempts}"))? {
                Case::Accepted { category, witness } => {
                    // A family that witnesses nothing would restore exactly the
                    // #16 defect: a digest independent of the code under test.
                    anyhow::ensure!(
                        !witness.is_empty(),
                        "{family}: accepted attempt {attempts} with an empty witness, so \
                         the evidence hash would not depend on what the code produced"
                    );
                    accepted += 1;
                    *category_counts.entry(category.to_owned()).or_default() += 1;
                    digest.update(b"A");
                    digest.update(category.as_bytes());
                    digest.update(&witness);
                }
                Case::Discarded { category, witness } => {
                    anyhow::ensure!(
                        !witness.is_empty(),
                        "{family}: discarded attempt {attempts} with an empty witness, so \
                         the evidence records that something was discarded and not what"
                    );
                    discards += 1;
                    *category_counts.entry(category.to_owned()).or_default() += 1;
                    digest.update(b"D");
                    digest.update(category.as_bytes());
                    digest.update(&witness);
                }
                Case::Negative { category, witness } => {
                    anyhow::ensure!(
                        !witness.is_empty(),
                        "{family}: negative attempt {attempts} has an empty witness"
                    );
                    negatives += 1;
                    *category_counts.entry(category.to_owned()).or_default() += 1;
                    digest.update(b"N");
                    digest.update(category.as_bytes());
                    digest.update(&witness);
                }
            }
            // The RNG state binds the INPUT; the witness above binds the OUTPUT.
            digest.update(&rng.0.to_le_bytes());
        }
        anyhow::ensure!(
            accepted + discards + negatives == attempts,
            "{family}: accounting lost cases"
        );
        let evidence_hash = digest.finalize().to_hex().to_string();
        let relation_rows = generated_relations(family)
            .iter()
            .map(|relation| GeneratedRelationEvidence {
                relation: (*relation).to_owned(),
                oracle_id: format!("oracle:{}", family.replace(['/', ' ', '-'], "_")),
                result: "pass".to_owned(),
                artifact_blake3: blake3::hash(
                    format!("{family}\0{relation}\0{evidence_hash}").as_bytes(),
                )
                .to_hex()
                .to_string(),
            })
            .collect::<Vec<_>>();
        let relation_matrix_blake3 = blake3::hash(
            serde_json::to_vec(&relation_rows)
                .context("serialize generated relation matrix")?
                .as_slice(),
        )
        .to_hex()
        .to_string();
        evidence.push(GeneratedEvidence {
            family: family.to_owned(),
            accepted,
            attempts,
            discards,
            negatives,
            seed,
            evidence_hash,
            category_counts,
            relations: relation_rows,
            oracle: Some(GeneratedOracleEvidence {
                id: format!("oracle:{}", family.replace(['/', ' ', '-'], "_")),
                source: generated_oracle_source(family).to_owned(),
                independent: true,
                relation_matrix_blake3,
            }),
        });
        timings.push(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
    }
    Ok((evidence, timings))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CanaryEvidence {
    id: String,
    gate: String,
    /// The mutation that was actually performed, checked against the packet's
    /// own prose (M17.5 pass-2 #22).
    violation: String,
    expected_failure: String,
    observed_failure: String,
    caught: bool,
    mutation_semantics: String,
}

/// One family's generated-evidence row.
///
/// Every field here must be a function of the seed and the code under test and
/// of NOTHING ELSE. Wall-clock timing used to live here as `elapsed_ms`, which
/// made the artifact unhashable — two runs of the same seed differed (50→57,
/// 94→110, 12→15 ms), so the artifact could never be compared against a
/// recorded digest (M17.5 pass-2 #18). Timing is operational telemetry, not
/// evidence; it is printed to stdout instead.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedEvidence {
    family: String,
    accepted: u64,
    attempts: u64,
    discards: u64,
    #[serde(default)]
    negatives: u64,
    /// Recorded so the run is reproducible (ADR-0020 §1).
    seed: u64,
    evidence_hash: String,
    /// Measured input-category coverage. Counts are emitted by the same run,
    /// not inferred from packet prose, so declared negative families cannot
    /// hide behind an ASCII-only generator. Default keeps old artifacts
    /// parseable long enough for the v2 schema check to report a clear error.
    #[serde(default)]
    category_counts: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    relations: Vec<GeneratedRelationEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oracle: Option<GeneratedOracleEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedRelationEvidence {
    relation: String,
    oracle_id: String,
    result: String,
    artifact_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedOracleEvidence {
    id: String,
    source: String,
    independent: bool,
    relation_matrix_blake3: String,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedEvidenceArtifact {
    schema_version: String,
    artifact_blake3: String,
    rows: Vec<GeneratedEvidence>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> Utf8PathBuf {
        Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    fn packet_from_repo() -> Packet {
        read_packet(&repo_root()).expect("read HAQP packet")
    }

    #[test]
    fn scope_trace_digest_ignores_scratch_prefix_and_reads_file_syscalls() {
        let first = br#"123 open("/tmp/first/fuzz/corpus/cst_parse", O_RDONLY) = 3
123 readlink("/tmp/first/conformance/haqp/packet.json", 0x0, 0) = 0
"#;
        let second = br#"123 open("/tmp/second/fuzz/corpus/cst_parse", O_RDONLY) = 3
123 readlink("/tmp/second/conformance/haqp/packet.json", 0x0, 0) = 0
"#;
        assert_eq!(
            scope_trace_paths_digest(first),
            scope_trace_paths_digest(second)
        );
        assert!(scope_trace_open_paths(first).contains("/tmp/first/fuzz/corpus/cst_parse"));
        assert!(scope_trace_open_paths(first).contains("/tmp/first/conformance/haqp/packet.json"));
    }

    #[test]
    fn mutation_compilation_failure_is_not_a_semantic_kill() {
        assert!(is_compilation_failure(
            b"error: could not compile `liminal-format` due to 1 previous error",
            b""
        ));
        assert!(is_compilation_failure(
            b"",
            b"error: Could Not Compile `crate`"
        ));
        assert!(!is_compilation_failure(b"test failed", b"assertion failed"));
    }

    fn mark_complete(packet: &mut Packet) {
        packet.qualification_state = "complete".to_owned();
        for mutant in &mut packet.mutants {
            mutant.disposition = "killed".to_owned();
            mutant.patch = Some(MutantPatch {
                file: "Cargo.toml".to_owned(),
                before: "[workspace]".to_owned(),
                after: "[workspace]\n".to_owned(),
            });
        }
        for canary in &mut packet.canaries {
            canary.result = "caught".to_owned();
        }
        for family in &mut packet.generated {
            family.result = "pass".to_owned();
        }
        for row in &mut packet.crash_boundaries {
            row.result = "pass".to_owned();
        }
        for review in &mut packet.reviews {
            review.result = "pass".to_owned();
        }
    }

    #[test]
    fn completed_packet_statuses_can_pass_qualified_layer() {
        let mut packet = packet_from_repo();
        mark_complete(&mut packet);

        verify_packet_shape(&packet).expect("complete packet shape stays valid");
        verify_packet_statuses(
            &packet,
            PacketStatusExpectations {
                mutant: "killed",
                canary: "caught",
                generated: "pass",
                crash: "pass",
                review: "pass",
            },
        )
        .expect("complete statuses accepted by qualified verifier");
    }

    #[test]
    fn mutant_patch_requires_one_relative_source_coordinate() {
        let good = MutantPatch {
            file: "crates/example.rs".to_owned(),
            before: "old".to_owned(),
            after: "new".to_owned(),
        };
        validate_mutant_patch(&good).expect("ordinary relative patch is valid");
        let mut noop = good.clone();
        noop.after = noop.before.clone();
        validate_mutant_patch(&noop).expect_err("a no-op mutant cannot be evaluated");
        for file in ["/tmp/example.rs", "../example.rs"] {
            let bad = MutantPatch {
                file: file.to_owned(),
                ..good.clone()
            };
            validate_mutant_patch(&bad).expect_err("patch must not escape worktree");
        }
    }

    #[test]
    fn threshold_mutation_changes_only_the_declared_integer() {
        let exact = MutantPatch {
            file: "crates/example.rs".to_owned(),
            before: "if count >= 16 {".to_owned(),
            after: "if count >= 17 {".to_owned(),
        };
        verify_mutant_operator_patch("threshold-plus-one", &exact)
            .expect("a single threshold increment is a valid mutation");

        let unrelated = MutantPatch {
            after: "if count >= 17 { panic!(\"unrelated\"); }".to_owned(),
            ..exact
        };
        verify_mutant_operator_patch("threshold-plus-one", &unrelated)
            .expect_err("an integer delta must not excuse unrelated source changes");
    }

    #[test]
    fn missing_enum_dispatch_binds_to_a_match_anchor() {
        let (file, line, anchor) = mutant_source_coordinate("P1-M005").expect("closed entry");
        assert_eq!(file, "crates/liminal-cst/src/parser.rs");
        assert_eq!(line, 64);
        assert_eq!(anchor, "match raw.0 {");
        let patch = MutantPatch {
            file: file.to_owned(),
            before: anchor.to_owned(),
            after: "raw.0 {".to_owned(),
        };
        verify_mutant_operator_patch("missing-enum-dispatch", &patch)
            .expect("the declared operator must be reachable at its source anchor");
    }

    #[test]
    fn specialized_mutant_operators_reject_unrelated_line_edits() {
        let oracle = MutantPatch {
            file: "x".to_owned(),
            before: "if id.is_some() {".to_owned(),
            after: "if true {".to_owned(),
        };
        verify_mutant_operator_patch("oracle-short-circuit", &oracle)
            .expect("closed oracle anchor is valid");
        let mut unrelated = oracle.clone();
        unrelated.after = "if id.is_none() {".to_owned();
        verify_mutant_operator_patch("oracle-short-circuit", &unrelated)
            .expect_err("predicate inversion is not an oracle bypass");

        let ordering = MutantPatch {
            file: "x".to_owned(),
            before: "let lines: Vec<(usize, &str)> = input.lines().enumerate().collect();"
                .to_owned(),
            after: "let lines: HashSet<(usize, &str)> = input.lines().collect();".to_owned(),
        };
        verify_mutant_operator_patch("ordering-nondeterminism", &ordering)
            .expect("Hash replacement is nondeterministic");
        let mut unrelated_ordering = ordering.clone();
        unrelated_ordering.after = unrelated_ordering.before.replace("Vec", "VecDeque");
        verify_mutant_operator_patch("ordering-nondeterminism", &unrelated_ordering)
            .expect_err("type rename is not nondeterminism");

        let holder = MutantPatch {
            file: "x".to_owned(),
            before: "&self.basis".to_owned(),
            after: "&self.text".to_owned(),
        };
        verify_mutant_operator_patch("wrong-holder-selection", &holder)
            .expect("closed holder redirect is valid");
        let mut unrelated_holder = holder.clone();
        unrelated_holder.after = "&self.other".to_owned();
        verify_mutant_operator_patch("wrong-holder-selection", &unrelated_holder)
            .expect_err("generic self-field change is not holder selection");
    }

    #[test]
    fn evaluated_equivalent_mutant_requires_proof_and_two_concurrences() {
        let mut packet = packet_from_repo();
        packet.mutants[0].disposition = "equivalent".to_owned();
        packet.mutants[0].patch = Some(MutantPatch {
            file: "Cargo.toml".to_owned(),
            before: "[workspace]".to_owned(),
            after: "[workspace]\n".to_owned(),
        });
        verify_mutant_inventory(&packet).expect_err("equivalence without proof must fail");
        packet.mutants[0].disposition_proof =
            Some("same observable function over declared domain".to_owned());
        packet.mutants[0].disposition_concurrence = vec![DispositionConcurrence {
            reviewer: "mimo".to_owned(),
            record: "pass-1".to_owned(),
            finding: "F-equivalent".to_owned(),
        }];
        verify_mutant_inventory(&packet).expect_err("equivalence needs both pass concurrences");
        packet.mutants[0]
            .disposition_concurrence
            .push(DispositionConcurrence {
                reviewer: "codex".to_owned(),
                record: "pass-2".to_owned(),
                finding: "F-equivalent".to_owned(),
            });
        verify_mutant_inventory(&packet).expect("fully documented equivalence is admissible");
    }

    #[test]
    fn evaluated_mutant_patch_must_bind_to_declared_source_coordinate() {
        let mut packet = packet_from_repo();
        packet.mutants[0].patch = Some(MutantPatch {
            file: "Cargo.toml".to_owned(),
            before: "[workspace]".to_owned(),
            after: "[workspace]\n".to_owned(),
        });
        let err = verify_mutant_source_coordinates(&repo_root(), &packet)
            .expect_err("patch targeting another file must not satisfy source coordinate");
        assert!(
            err.to_string()
                .contains("does not match mutant source file"),
            "{err}"
        );
    }

    #[test]
    fn mutant_patch_replacement_rejects_zero_or_multiple_matches() {
        let scratch = liminal_scratch::ScratchDir::new("haq-mutant-patch").expect("scratch");
        let fixture_path = scratch.join("target.txt");
        fs::write(&fixture_path, "one old two old").expect("write fixture");
        let replacement = MutantPatch {
            file: "target.txt".to_owned(),
            before: "old".to_owned(),
            after: "new".to_owned(),
        };
        apply_mutant_patch(&scratch, &replacement).expect_err("ambiguous replacement must fail");
        fs::write(&fixture_path, "one old").expect("rewrite fixture");
        apply_mutant_patch(&scratch, &replacement).expect("unique replacement succeeds");
        assert_eq!(
            fs::read_to_string(fixture_path).expect("read fixture"),
            "one new"
        );
    }

    /// M17.5 F-05 canary: dropping one evidence kind must fail the shape
    /// check. Without this, "36 requirements covered" can mean 36 requirements
    /// on one coarse end-to-end test.
    #[test]
    fn shape_check_rejects_a_requirement_missing_an_evidence_kind() {
        let mut packet = packet_from_repo();
        // Strip every `negative` test; the requirements they served lose a kind.
        packet
            .tests
            .retain(|test| test.evidence != vec!["negative".to_owned()]);
        for (index, test) in packet.tests.iter_mut().enumerate() {
            test.id = format!("P1-T{:02}", index + 1);
        }
        for mutant in &mut packet.mutants {
            mutant.killing_tests = vec![packet.tests[0].id.clone()];
        }
        let err = verify_packet_shape(&packet)
            .expect_err("a requirement missing negative evidence must be rejected");
        assert!(
            err.to_string().contains("missing"),
            "unexpected error: {err}"
        );
    }

    /// M17.5 F-05 canary: a 100% kill rate carried by one coarse test is the
    /// clustering ADR-0020 §3 forbids.
    #[test]
    fn shape_check_rejects_mutation_coverage_clustered_on_one_test() {
        let mut packet = packet_from_repo();
        let sole = packet.tests[0].id.clone();
        for mutant in &mut packet.mutants {
            mutant.killing_tests = vec![sole.clone()];
        }
        let err = verify_packet_shape(&packet)
            .expect_err("kill concentration on one test must be rejected");
        assert!(
            err.to_string().contains("clustering"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn kill_concentration_accepts_exactly_twenty_five_percent() {
        let mut packet = packet_from_repo();
        packet.mutants.truncate(4);
        let witness = packet.tests[0].id.clone();
        for mutant in &mut packet.mutants {
            mutant.killing_tests.clear();
        }
        packet.mutants[0].killing_tests = vec![witness];
        verify_kill_concentration(&packet)
            .expect("the 25% ceiling is inclusive; 1 of 4 mutants must be accepted");
    }

    // ── M17.5 pass-2 #11/#12/#13: the fabricated-inventory cluster ──────────
    // An independent reviewer built packets whose rows named nothing real and
    // watched the verifier accept them. One canary per hole.

    /// #12: a requirement with no test behind it inflates the inventory while
    /// proving nothing. The old check exempted non-critical requirements.
    #[test]
    fn shape_check_rejects_a_requirement_mapped_to_no_test() {
        let mut packet = packet_from_repo();
        let orphan = packet.requirements.last().expect("requirements").id.clone();
        for test in &mut packet.tests {
            test.requirements.retain(|id| id != &orphan);
        }
        let err = verify_packet_shape(&packet)
            .expect_err("an unmapped requirement must be rejected even when non-critical");
        assert!(
            err.to_string().contains("not mapped to any test"),
            "unexpected error: {err}"
        );
    }

    /// #13: free-text operators defeat the concentration ceiling, because a
    /// label invented once can never exceed a share of the denominator.
    #[test]
    fn shape_check_rejects_an_invented_mutation_operator() {
        let mut packet = packet_from_repo();
        packet.mutants[0].operator = "artisanal-bespoke-operator".to_owned();
        let err = verify_packet_shape(&packet)
            .expect_err("an operator outside the declared vocabulary must be rejected");
        assert!(
            err.to_string().contains("unknown mutation operator"),
            "unexpected error: {err}"
        );
    }

    /// #13: a mutant must attack something the inventory claims to require.
    #[test]
    fn shape_check_rejects_a_mutant_whose_source_is_not_a_requirement() {
        let mut packet = packet_from_repo();
        packet.mutants[0].source = "P1-R999".to_owned();
        let err = verify_packet_shape(&packet)
            .expect_err("a mutant naming an unknown requirement must be rejected");
        assert!(
            err.to_string().contains("not a declared requirement"),
            "unexpected error: {err}"
        );
    }

    /// #13: an invented family would let a fabricated packet satisfy the
    /// per-family balance with labels that name no evidence family.
    #[test]
    fn shape_check_rejects_an_invented_mutation_family() {
        let mut packet = packet_from_repo();
        packet.mutants[0].family = "vibes/general-goodness".to_owned();
        let err = verify_packet_shape(&packet)
            .expect_err("a family outside ADR-0020 §4's five must be rejected");
        assert!(
            err.to_string().contains("unknown mutation family"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn shape_check_rejects_an_invented_generated_family() {
        let mut packet = packet_from_repo();
        packet.generated[0].family = "invented/family".to_owned();
        let err = verify_packet_shape(&packet)
            .expect_err("generated evidence must name one of the five declared families");
        assert!(
            err.to_string().contains("generated family ids differ"),
            "unexpected error: {err}"
        );
    }

    /// #11: the qualified layer must reject a packet naming tests nobody wrote.
    /// The inventory layer must NOT, because predeclaring M19–M24's tests is
    /// the inventory's purpose — this pins both halves of that asymmetry.
    #[test]
    fn qualified_layer_rejects_test_names_that_do_not_exist() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        packet.tests[0].name = "laws::this_test_was_never_written".to_owned();

        let err = verify_test_names_exist(&root, &packet)
            .expect_err("a qualification naming a nonexistent test must be rejected");
        assert!(
            err.to_string().contains("do not exist in the tree"),
            "unexpected error: {err}"
        );

        // Closed test registry rejects remapping even before qualification.
        let err = verify_packet_shape(&packet)
            .expect_err("a remapped test name must differ from closed registry");
        assert!(err.to_string().contains("test registry digest"), "{err}");
    }

    #[test]
    fn runnable_test_scanner_rejects_comments_helpers_and_ignored_tests() {
        let mut names = BTreeMap::new();
        collect_runnable_test_functions(
            "// fn commented() { }\nfn helper() {}\n#[test]\nfn live() {}\n#[test]\n#[ignore]\nfn dormant() {}\n#[ignore]\n#[test]\nfn reversed() {}",
            &mut names,
        );
        assert_eq!(names, BTreeMap::from([("live".to_owned(), 1)]));
    }

    #[test]
    fn runnable_test_scanner_does_not_let_one_cfg_poison_later_tests() {
        let mut names = BTreeMap::new();
        collect_runnable_test_functions(
            "#[cfg(any())]\nfn compiled_out() {}\n#[test]\nfn live() {}\n#[cfg(unix)]\n#[test]\nfn active_cfg() {}\n#[cfg(test)]\n#[test]\nfn active_test_cfg() {}\n#[cfg(not(test))]\n#[test]\nfn inactive_test_cfg() {}\n#[test]\nfn later_live() {}",
            &mut names,
        );
        assert_eq!(
            names,
            BTreeMap::from([
                ("active_cfg".to_owned(), 1),
                ("active_test_cfg".to_owned(), 1),
                ("later_live".to_owned(), 1),
                ("live".to_owned(), 1),
            ])
        );
    }

    #[test]
    fn inventory_status_layer_still_rejects_completed_rows() {
        let mut packet = packet_from_repo();
        mark_complete(&mut packet);

        let err = verify_packet_statuses(
            &packet,
            PacketStatusExpectations {
                mutant: "predeclared",
                canary: "predeclared",
                generated: "planned",
                crash: "registered",
                review: "planned",
            },
        )
        .expect_err("inventory verifier must reject completed statuses");
        assert!(
            err.to_string().contains("mutant.disposition"),
            "unexpected error: {err}"
        );
    }

    /// #22: a canary row whose prose disagrees with the mutation the runner
    /// actually performs must be rejected. Without this, `gate` and `violation`
    /// are decorative and the canary table describes whatever it likes.
    #[test]
    fn canary_runner_rejects_prose_that_misdescribes_its_own_mutation() {
        let root = repo_root();
        let markdown = fs::read_to_string(root.join("docs/execution/phase1-suite-review.md"))
            .expect("review markdown");
        let baseline = packet_from_repo();

        // The honest packet must pass, or the assertion below proves nothing.
        run_canary_suite(&root, &baseline, &markdown)
            .expect("the committed canary table must agree");

        let mut doctored = baseline.clone();
        doctored.canaries[8].violation = "reticulates splines".to_owned();
        let err = run_canary_suite(&root, &doctored, &markdown)
            .expect_err("prose that misdescribes the executed mutation must fail closed");
        assert!(
            err.to_string().contains("C09.violation"),
            "unexpected error: {err}"
        );
    }

    /// #22, second half: a declared canary with no implemented mutation must
    /// never be recorded as caught.
    ///
    /// This exercises `mutate_canary` directly because the guard is
    /// defence-in-depth: `verify_canary_inventory` pins the id set to exactly
    /// C01–C33, so a C34 row is rejected before `run_canary_suite` ever sees it.
    /// The guard exists for the day that rule is relaxed to admit new canaries —
    /// see the note on `run_canary_suite`.
    #[test]
    fn canary_runner_refuses_a_declared_canary_it_cannot_execute() {
        let mut packet = packet_from_repo();
        let mut markdown = String::new();
        let err = mutate_canary("C34", &mut packet, &mut markdown, "")
            .expect_err("a canary with no arm must not be counted as caught");
        assert!(
            err.to_string().contains("unknown canary C34"),
            "unexpected error: {err}"
        );
    }

    /// The committed fuzz artifact must parse into the struct that checks it.
    /// `deny_unknown_fields` is what makes that meaningful: the artifact already
    /// carried `elapsed_s`, `seed` and `log`, and the struct silently discarded
    /// all three (#17).
    #[test]
    fn committed_fuzz_evidence_parses_with_every_field_checked() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let rows: Vec<FuzzEvidence> =
            serde_json::from_slice(&bytes).expect("every recorded field must be declared");
        assert_eq!(rows.len(), 7, "seven targets");
        assert!(
            rows.iter().all(|row| row.seed != 0),
            "the campaign seed must survive parsing"
        );

        let doctored = String::from_utf8(bytes)
            .expect("utf8")
            .replace("\"seed\":", "\"unchecked_new_field\":");
        serde_json::from_str::<Vec<FuzzEvidence>>(&doctored)
            .expect_err("a field the verifier does not check must not parse silently");
    }

    /// #17: a fuzz row naming a target that does not exist must be rejected. The
    /// old count-only check compared against `packet.generated.len()` — families,
    /// not targets — so an invented target name passed.
    #[test]
    fn fuzz_lane_rejects_a_target_that_does_not_exist() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
        // The committed artifact predates F-17 and carries no log digest. Each
        // of these tests doctors ONE field to exercise ONE check, so the
        // unrelated missing digest is normalised rather than left to fire first.
        for row in &mut rows {
            row.log_blake3 = "ab".repeat(32);
        }
        let present = rows
            .iter()
            .map(|row| row.target.clone())
            .collect::<BTreeSet<_>>();

        // The honest artifact must PASS, or the rejection below proves nothing.
        // This assertion used to expect failure, because the committed evidence
        // still carried F-09's crash; the campaign of 2026-07-30 is clean
        // (seven targets, 217 target-minutes, 0 crashes, 0 artifacts), so the
        // expectation flipped with the evidence.
        verify_fuzz_rows(&rows, &present).expect("the committed fuzz evidence must be clean");

        rows[0].target = "target_nobody_wrote".to_owned();
        let err = verify_fuzz_rows(&rows, &present)
            .expect_err("a target that does not exist must be rejected");
        assert!(
            err.to_string().contains("does not cover the targets"),
            "unexpected error: {err}"
        );
    }

    /// #17: a target claiming a clean full-length campaign that finished far
    /// short of its budget is stating a contradiction.
    #[test]
    fn fuzz_lane_rejects_a_clean_campaign_that_exited_early() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
        // The committed artifact predates F-17 and carries no log digest. Each
        // of these tests doctors ONE field to exercise ONE check, so the
        // unrelated missing digest is normalised rather than left to fire first.
        for row in &mut rows {
            row.log_blake3 = "ab".repeat(32);
        }
        let present = rows
            .iter()
            .map(|row| row.target.clone())
            .collect::<BTreeSet<_>>();
        // Make every row clean so the early-exit check is what fires. Note that
        // `elapsed_s` has to be normalised too: `canonical_round_trip` really did
        // stop early (1675s of an 1800s budget) because of F-09, so simply
        // clearing its exit code leaves behind exactly the contradiction this
        // check exists to catch.
        for row in &mut rows {
            row.exit_code = 0;
            row.artifacts = 0;
            row.elapsed_s = row.seconds + 1;
        }
        verify_fuzz_rows(&rows, &present).expect("an all-clean campaign is otherwise valid");

        rows[0].elapsed_s = 10;
        let err = verify_fuzz_rows(&rows, &present)
            .expect_err("a clean campaign cannot finish in a fraction of its budget");
        assert!(
            err.to_string().contains("not a coherent result"),
            "unexpected error: {err}"
        );
    }

    /// #17: rows carrying different seeds describe different campaigns, none of
    /// which is the one the packet points at. Checking only for a nonzero seed
    /// let the comment claim more than the code enforced.
    #[test]
    fn fuzz_lane_rejects_rows_from_different_campaigns() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
        // The committed artifact predates F-17 and carries no log digest. Each
        // of these tests doctors ONE field to exercise ONE check, so the
        // unrelated missing digest is normalised rather than left to fire first.
        for row in &mut rows {
            row.log_blake3 = "ab".repeat(32);
        }
        let present = rows
            .iter()
            .map(|row| row.target.clone())
            .collect::<BTreeSet<_>>();
        for row in &mut rows {
            row.exit_code = 0;
            row.artifacts = 0;
            row.elapsed_s = row.seconds + 1;
        }
        verify_fuzz_rows(&rows, &present).expect("one campaign, one seed");

        rows[2].seed += 1;
        let err = verify_fuzz_rows(&rows, &present)
            .expect_err("rows from two campaigns must be rejected");
        assert!(
            err.to_string().contains("distinct seeds"),
            "unexpected error: {err}"
        );
    }

    fn crash_evidence_from_repo() -> (CrashEvidence, BTreeSet<String>) {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/crash.json"))
            .expect("committed crash evidence");
        let recorded: CrashEvidence = serde_json::from_slice(&bytes).expect("parse");
        let declared = packet_from_repo()
            .crash_boundaries
            .iter()
            .map(|row| row.boundary.clone())
            .collect::<BTreeSet<_>>();
        (recorded, declared)
    }

    /// #19: the committed artifact and the packet must agree on which
    /// boundaries exist. The lane used to write into gitignored `target/`, so
    /// the artifact could say `pass` while the packet said `registered` and the
    /// inventory exited 0 without ever comparing them.
    #[test]
    fn crash_lane_rejects_evidence_that_does_not_match_the_packet() {
        let (recorded, declared) = crash_evidence_from_repo();
        verify_crash_rows(&recorded, &declared)
            .expect("the committed crash evidence must match the packet");

        let mut doctored = recorded.clone();
        doctored.boundaries.pop();
        let err = verify_crash_rows(&doctored, &declared)
            .expect_err("evidence missing a declared boundary must be rejected");
        assert!(
            err.to_string().contains("different boundaries"),
            "unexpected error: {err}"
        );
    }

    /// #19: a boundary that never fired is indistinguishable from a dead one
    /// (ADR-0020 §5), and a failing recovery must not read as evidence.
    #[test]
    fn crash_lane_rejects_an_unexercised_or_failing_boundary() {
        let (recorded, declared) = crash_evidence_from_repo();

        let mut unexercised = recorded.clone();
        unexercised.boundaries[0].occurrences_exercised = 0;
        let err = verify_crash_rows(&unexercised, &declared)
            .expect_err("an unexercised boundary must be rejected");
        assert!(
            err.to_string().contains("never exercised"),
            "unexpected error: {err}"
        );

        let mut residue = recorded;
        residue.boundaries[0].staged_residue = "one staged file".to_owned();
        let err =
            verify_crash_rows(&residue, &declared).expect_err("staged residue must be rejected");
        assert!(
            err.to_string().contains("staged residue"),
            "unexpected error: {err}"
        );
    }

    /// #20: a review may not count findings as VERIFIED that no second party
    /// reproduced. One model asserting a defect is not evidence of a defect.
    #[test]
    fn review_lane_rejects_verified_findings_nobody_reproduced() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        verify_review_evidence(&root, &packet).expect("the committed review rows are consistent");

        packet.reviews[0].unresolved_verified_findings = 2;
        packet.reviews[0].findings = vec!["P1-F01".to_owned(), "P1-F02".to_owned()];
        // ...but nothing was independently reproduced.
        let err = verify_review_evidence(&root, &packet)
            .expect_err("verified findings with no independent reproduction must be rejected");
        assert!(
            err.to_string().contains("independently reproduced only 0"),
            "unexpected error: {err}"
        );
    }

    /// #20: a review cannot claim to have reproduced a finding it never raised.
    #[test]
    fn review_lane_rejects_reproducing_a_finding_never_raised() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        packet.reviews[1].independently_reproduced = vec!["P1-F99".to_owned()];
        let err = verify_review_evidence(&root, &packet)
            .expect_err("reproducing an unraised finding must be rejected");
        assert!(
            err.to_string().contains("never raised"),
            "unexpected error: {err}"
        );
    }

    /// #20: `pass` requires a committed record. The blind-review lane writes
    /// under `target/`, so without this a passing review is four numbers typed
    /// into the packet.
    #[test]
    fn review_lane_rejects_a_pass_with_no_committed_record() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        packet.reviews[0].result = "pass".to_owned();
        let err = verify_review_evidence(&root, &packet)
            .expect_err("a passing review with no record must be rejected");
        assert!(
            err.to_string().contains("no committed record"),
            "unexpected error: {err}"
        );
    }

    /// #20 hardening: the packet is the artifact this verifier distrusts, so an
    /// evidence path that escapes the repository must be refused rather than
    /// read. `join` discards its base when given an absolute path.
    #[test]
    fn review_lane_refuses_an_evidence_path_that_escapes_the_repo() {
        let root = repo_root();
        for escape in ["/etc/passwd", "../../../etc/passwd"] {
            let mut packet = packet_from_repo();
            packet.reviews[0].result = "pass".to_owned();
            packet.reviews[0].evidence = Some(escape.to_owned());
            let err = verify_review_evidence(&root, &packet)
                .expect_err("an evidence path outside the repo must be refused");
            assert!(
                err.to_string().contains("escapes the repository"),
                "{escape} produced the wrong error: {err}"
            );
        }
    }

    /// #18: the generated artifact must be byte-identical across runs, or it
    /// cannot be hash-compared against a recorded digest. `elapsed_ms` used to
    /// make this impossible.
    ///
    /// This also guards the reproducibility of the generators themselves: any
    /// entropy they draw from outside the seeded `Rng` shows up here as a
    /// differing artifact.
    #[test]
    fn generated_evidence_is_byte_identical_across_runs() {
        let (first, _) = generate_evidence(64).expect("first run");
        let (second, _) = generate_evidence(64).expect("second run");
        assert_eq!(
            serde_json::to_string_pretty(&first).expect("serialize first"),
            serde_json::to_string_pretty(&second).expect("serialize second"),
            "the same seed produced two different artifacts, so the recorded \
             evidence hash certifies nothing"
        );
    }

    // ── M17.5 F-27 / P1-A05, A06, A07 ─────────────────────────────────────
    // The blind lane's own findings. `verify_review_evidence` counted the
    // record's attempts array and read nothing else, so a record of twelve
    // empty objects, a record contradicting its packet row, and two reviews
    // from one model family all passed. Each check gets the degenerate case
    // that must trip it.

    fn attempt(id: &str) -> ReviewRecordAttempt {
        let index = id.trim_start_matches('A').parse::<usize>().unwrap_or(0);
        let target = "crates/liminal-xtask/src/haq.rs:1";
        ReviewRecordAttempt {
            id: id.to_owned(),
            attack_class: REVIEW_ATTACK_CLASSES[index % REVIEW_ATTACK_CLASSES.len()].to_owned(),
            target: target.to_owned(),
            attempt: format!("attempted concrete falsification at {target} for case {id}"),
            observed_result: format!("observed verifier rejection at {target} for case {id}"),
            independently_reproduced: false,
            classification: "caught_violation".to_owned(),
            resolved: false,
            resolution: None,
        }
    }

    /// The retained raw answer a fixture record's digest names (A09): a
    /// scratch directory holding `r.raw.txt`, and the record path beside it.
    /// The transcript a fixture codex receipt attests to.
    /// A rollout opens with its own `session_meta`, and A09 binds the receipt's
    /// id to the `session_id` that record carries.
    const FIXTURE_TRANSCRIPT: &[u8] =
        b"{\"type\":\"session_meta\",\"payload\":{\"session_id\":\"fixture\"}}\n";

    /// A fixture codex receipt, in the canonical JSON the gate re-derives from
    /// the record file: sorted keys, compact separators.
    fn fixture_receipt_json() -> String {
        format!(
            r#"{{"backend":"codex","session_file":"rollout-fixture.jsonl","session_id":"fixture","transcript_bytes":{},"transcript_sha256":"{:x}"}}"#,
            FIXTURE_TRANSCRIPT.len(),
            Sha256::digest(FIXTURE_TRANSCRIPT)
        )
    }

    fn fixture_record_path() -> (liminal_scratch::ScratchDir, Utf8PathBuf) {
        let scratch = liminal_scratch::ScratchDir::new("haq-review-raw").expect("scratch");
        fs::write(scratch.path().join("r.raw.txt"), "fixture raw answer").expect("raw");
        fs::write(scratch.path().join("r.session.jsonl"), FIXTURE_TRANSCRIPT).expect("transcript");
        let path = scratch.path().join("r.json");
        // The claims and receipt the binding hashes are read from this file;
        // the fixture record's binding is computed over the same bytes.
        fs::write(
            &path,
            format!(r#"{{"provider_receipt":{}}}"#, fixture_receipt_json()),
        )
        .expect("record file");
        (scratch, path)
    }

    fn review_record(pass: u8, family: &str, backend: &str) -> ReviewRecord {
        let root = repo_root();
        let commit = git_text(&root, &["rev-parse", "HEAD"]).expect("test HEAD");
        let tree = git_text(&root, &["rev-parse", "HEAD^{tree}"]).expect("test tree");
        let mut record = ReviewRecord {
            schema_version: "haqp-blind-review-v1".to_owned(),
            pass,
            reviewer: ReviewRecordReviewer {
                model_family: family.to_owned(),
                identity_hash: sha256_text(&format!("pass{pass}:{family}:haqp-blind-review-v1")),
                backend: backend.to_owned(),
            },
            attempts: (0..12).map(|i| attempt(&format!("A{i}"))).collect(),
            findings: Vec::new(),
            independently_reproduced: Vec::new(),
            unresolved_verified_findings: 0,
            result: "pass".to_owned(),
            raw_response_sha256: sha256_text("fixture raw answer"),
            schema_retries: 0,
            provider_receipt: serde_json::from_str(&fixture_receipt_json())
                .expect("fixture receipt parses"),
            integrity_binding_sha256: String::new(),
            isolated_session_hash: hex_digest(format!("session:{family}:{pass}").as_bytes()),
            sanitized_prompt_hash: hex_digest(format!("prompt:{family}").as_bytes()),
            prompt_binding_sha256: sha256_text(&format!(
                "haqp-blind-review-v1\0{pass}\0{family}\0{commit}\0{tree}"
            )),
            fixed_base: ReviewFixedBase {
                commit,
                tree,
                clean: true,
            },
            blindness_proof: ReviewRecordBlindness {
                ephemeral_session_requested: backend == "lamu",
                session_state: if backend == "lamu" {
                    "ephemeral-cloud-session".to_owned()
                } else {
                    "fresh-codex-session".to_owned()
                },
                prior_pass_artifact_supplied: false,
                pass_two_original_spec_only: pass == 2,
            },
        };
        record.integrity_binding_sha256 = sha256_text(&format!(
            "haqp-review-integrity-v3\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            record.pass,
            record.reviewer.model_family,
            record.fixed_base.commit,
            record.fixed_base.tree,
            record.prompt_binding_sha256,
            record.raw_response_sha256,
            // fixture_record_path writes only the receipt: claim keys read null.
            sha256_text(r#"{"attempts":null,"findings":null,"independently_reproduced":null}"#),
            sha256_text(&fixture_receipt_json()),
            record.result,
            record.unresolved_verified_findings
        ));
        record
    }

    fn review_row() -> Review {
        Review {
            reviewer: "openai".to_owned(),
            attempts: 12,
            findings: Vec::new(),
            independently_reproduced: Vec::new(),
            unresolved_verified_findings: 0,
            evidence: Some("conformance/haqp/evidence/review.json".to_owned()),
            result: "pass".to_owned(),
        }
    }

    #[test]
    fn a_well_formed_review_record_is_accepted() {
        verify_review_record(
            &repo_root(),
            &review_row(),
            &review_record(1, "openai", "codex"),
            &fixture_record_path().1,
        )
        .expect("a complete record must pass, or every check below is vacuous");
    }

    #[test]
    fn cross_pass_reproduction_requires_identical_falsification_claim() {
        let mut first = review_record(1, "openai", "codex");
        first.attempts[0].attack_class = "vacuity".to_owned();
        first.attempts[0].target = "crates/liminal-xtask/src/haq.rs:1".to_owned();
        first.attempts[0].attempt =
            "first reviewer falsification at crates/liminal-xtask/src/haq.rs:1".to_owned();
        first.attempts[0].observed_result =
            "first reviewer observation at crates/liminal-xtask/src/haq.rs:1".to_owned();
        first.attempts[0].classification = "verified_defect".to_owned();
        first.attempts[0].independently_reproduced = true;
        first.findings = vec![serde_json::json!({"id": "P1-F1", "attempt_id": "A0"})];
        first.independently_reproduced = vec!["P1-F1".to_owned()];

        let mut second = review_record(2, "xiaomi", "lamu");
        second.attempts[0].attack_class = "vacuity".to_owned();
        second.attempts[0].target = "crates/liminal-xtask/src/haq.rs:1".to_owned();
        second.attempts[0].attempt = first.attempts[0].attempt.clone();
        second.attempts[0].observed_result = first.attempts[0].observed_result.clone();
        second.attempts[0].classification = "verified_defect".to_owned();
        second.attempts[0].independently_reproduced = true;
        second.findings = vec![serde_json::json!({"id": "A-F1", "attempt_id": "A0"})];
        second.independently_reproduced = vec!["A-F1".to_owned()];

        verify_cross_pass_reproduction(&[("p1".to_owned(), first), ("p2".to_owned(), second)])
            .expect("identical falsification claims must concur");
    }

    /// P1-A05: array padding is not a set of attempts.
    #[test]
    fn review_record_rejects_empty_attempt_records() {
        let mut record = review_record(1, "openai", "codex");
        record.attempts[3].observed_result = "   ".to_owned();
        let err = verify_review_record(
            &repo_root(),
            &review_row(),
            &record,
            &fixture_record_path().1,
        )
        .expect_err("an attempt with no observed result is not an attempt");
        assert!(err.to_string().contains("empty observed_result"), "{err}");
    }

    #[test]
    fn review_record_rejects_vacuous_attempt_prose() {
        let mut record = review_record(1, "openai", "codex");
        record.attempts[0].attempt = "did a thing".to_owned();
        record.attempts[0].observed_result = "saw a thing".to_owned();
        let err = verify_review_record(
            &repo_root(),
            &review_row(),
            &record,
            &fixture_record_path().1,
        )
        .expect_err("generic prose is not a concrete falsification attempt");
        assert!(err.to_string().contains("substantive"), "{err}");
    }

    #[test]
    fn review_coordinate_binding_rejects_numeric_prefix_collisions() {
        assert!(contains_exact_coordinate(
            "falsified crates/liminal-xtask/src/haq.rs:1 at runtime",
            "crates/liminal-xtask/src/haq.rs:1"
        ));
        assert!(!contains_exact_coordinate(
            "falsified crates/liminal-xtask/src/haq.rs:10 at runtime",
            "crates/liminal-xtask/src/haq.rs:1"
        ));
        assert!(!contains_exact_coordinate(
            "falsified crates/liminal-xtask/src/haq.rs:1a at runtime",
            "crates/liminal-xtask/src/haq.rs:1"
        ));
        assert!(!contains_exact_coordinate(
            "falsified crates/liminal-xtask/src/haq.rs:1:5 at runtime",
            "crates/liminal-xtask/src/haq.rs:1"
        ));
    }

    #[test]
    fn canary_failure_prefixes_are_closed() {
        assert!(canary_failure_matches(
            "status: expected",
            "status: expected value"
        ));
        assert_eq!(
            canary_expected_prefix("C01").expect("known canary"),
            "status: expected"
        );
        assert!(
            canary_expected_prefix("C01").expect("known canary") != "unrelated generic failure"
        );
        canary_expected_prefix("C99").expect_err("unknown canary prefix must fail closed");
    }

    #[test]
    fn review_identity_binding_rejects_arbitrary_hashes() {
        let mut record = review_record(1, "openai", "codex");
        record.reviewer.identity_hash = "a".repeat(64);
        let err = verify_review_record(
            &repo_root(),
            &review_row(),
            &record,
            &fixture_record_path().1,
        )
        .expect_err("review identity must bind to pass and model family");
        assert!(err.to_string().contains("identity_hash"), "{err}");
    }

    #[test]
    fn review_integrity_binding_rejects_arbitrary_digests() {
        let mut record = review_record(1, "openai", "codex");
        record.raw_response_sha256 = "c".repeat(64);
        let err = verify_review_record(
            &repo_root(),
            &review_row(),
            &record,
            &fixture_record_path().1,
        )
        .expect_err("raw-response digest must be bound to the retained answer");
        // A09: the substituted digest is caught against the retained answer
        // before the integrity receipt is even consulted.
        assert!(err.to_string().contains("raw_response_sha256"), "{err}");
    }

    #[test]
    fn review_record_rejects_unknown_classifications_and_duplicate_ids() {
        let mut unknown = review_record(1, "openai", "codex");
        unknown.attempts[0].classification = "inconclusive".to_owned();
        verify_review_record(
            &repo_root(),
            &review_row(),
            &unknown,
            &fixture_record_path().1,
        )
        .expect_err("a classification outside the declared set must be rejected");
        let mut duplicated = review_record(1, "openai", "codex");
        duplicated.attempts[1].id = duplicated.attempts[0].id.clone();
        verify_review_record(
            &repo_root(),
            &review_row(),
            &duplicated,
            &fixture_record_path().1,
        )
        .expect_err("twelve attempts must be twelve DISTINCT attempts");
    }

    #[test]
    fn review_record_requires_reproduction_for_false_positives() {
        let mut record = review_record(1, "openai", "codex");
        record.attempts[0].classification = "false_positive".to_owned();
        record.attempts[0].independently_reproduced = false;
        let err = verify_review_record(
            &repo_root(),
            &review_row(),
            &record,
            &fixture_record_path().1,
        )
        .expect_err("false positives must retain reproduction evidence");
        assert!(err.to_string().contains("false-positive"), "{err}");
        record.attempts[0].independently_reproduced = true;
        verify_review_record(
            &repo_root(),
            &review_row(),
            &record,
            &fixture_record_path().1,
        )
        .expect("a reproduced false positive is admissible");
    }

    #[test]
    fn review_record_rejects_an_unbound_fixed_tree() {
        let mut record = review_record(1, "openai", "codex");
        record.fixed_base.tree = "not-a-digest".to_owned();
        verify_review_record(
            &repo_root(),
            &review_row(),
            &record,
            &fixture_record_path().1,
        )
        .expect_err("a review record without a fixed tree binding is untrusted");
    }

    #[test]
    fn review_attempts_reject_locked_corpus_targets() {
        let root = repo_root();
        let fixed = git_text(&root, &["rev-parse", "HEAD"]).expect("test HEAD");
        let mut locked = attempt("locked");
        locked.target = "conformance/corpora/dev/input:1".to_owned();
        verify_review_attempts(&root, &fixed, "test", &[locked])
            .expect_err("review targets may not name locked corpus paths");
    }

    /// P1-A06: the packet may not summarize the record more kindly than the
    /// record summarizes itself.
    #[test]
    fn review_record_rejects_a_packet_row_that_contradicts_it() {
        let mut failed = review_record(1, "openai", "codex");
        failed.result = "fail".to_owned();
        let err = verify_review_record(
            &repo_root(),
            &review_row(),
            &failed,
            &fixture_record_path().1,
        )
        .expect_err("a pass row over a failing record must be rejected");
        assert!(err.to_string().contains("record says"), "{err}");

        let mut unresolved = review_record(1, "openai", "codex");
        unresolved.unresolved_verified_findings = 9;
        verify_review_record(
            &repo_root(),
            &review_row(),
            &unresolved,
            &fixture_record_path().1,
        )
        .expect_err("a row claiming zero unresolved findings over a record counting nine");
    }

    #[test]
    fn packet_statuses_require_exact_result_tokens() {
        let mut packet = packet_from_repo();
        packet.reviews[0].result = "planned_extra".to_owned();
        verify_packet_statuses(&packet, inventory_statuses())
            .expect_err("status suffixes must not pass exact packet status checks");
    }

    #[test]
    fn review_record_rejects_a_finding_without_independent_reproduction() {
        let mut record = review_record(1, "openai", "codex");
        record.attempts[0].classification = "verified_defect".to_owned();
        record.attempts[0].independently_reproduced = true;
        record.findings = vec![
            serde_json::json!({"id": "F1", "attempt_id": "A0"}),
            serde_json::json!({"id": "F2", "attempt_id": "A0"}),
        ];
        record.independently_reproduced = vec!["F1".to_owned()];
        record.unresolved_verified_findings = 1;

        let mut row = review_row();
        row.findings = vec!["F1".to_owned(), "F2".to_owned()];
        row.independently_reproduced = vec!["F1".to_owned()];
        row.unresolved_verified_findings = 1;
        let err = verify_review_record(&repo_root(), &row, &record, &fixture_record_path().1)
            .expect_err("every emitted finding must be independently reproduced");
        assert!(err.to_string().contains("every finding"), "{err}");
    }

    /// P1-A07: blindness is a property of the run, and it is recorded.
    #[test]
    fn review_record_rejects_a_pass_that_saw_prior_artifacts() {
        let mut leaked = review_record(1, "openai", "codex");
        leaked.blindness_proof.prior_pass_artifact_supplied = true;
        verify_review_record(
            &repo_root(),
            &review_row(),
            &leaked,
            &fixture_record_path().1,
        )
        .expect_err("a reviewer shown the prior pass is not blind");

        let mut informed = review_record(2, "xiaomi", "lamu");
        informed.blindness_proof.pass_two_original_spec_only = false;
        verify_review_record(
            &repo_root(),
            &review_row(),
            &informed,
            &fixture_record_path().1,
        )
        .expect_err("pass 2 must start from the original spec alone");
    }

    #[test]
    fn review_resolution_must_descend_from_fixed_base() {
        let root = repo_root();
        let fixed = git_text(&root, &["rev-parse", "HEAD"]).expect("test HEAD");
        let evidence_path = "conformance/haqp/evidence/fuzz.json";
        let bytes = fs::read(root.join(evidence_path)).expect("fuzz evidence");
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let mut resolved = attempt("resolved");
        resolved.resolved = true;
        resolved.resolution = Some(ReviewResolution {
            commit: fixed.clone(),
            coordinate: "crates/liminal-xtask/src/haq.rs:1".to_owned(),
            evidence_path: evidence_path.to_owned(),
            evidence_sha256: format!("{:x}", hasher.finalize()),
        });
        let base = ReviewFixedBase {
            commit: fixed,
            tree: git_text(&root, &["rev-parse", "HEAD^{tree}"]).expect("test tree"),
            clean: true,
        };
        let err = verify_review_resolution(&root, &base, "test", &resolved)
            .expect_err("a resolution at fixed base is not a descendant");
        assert!(err.to_string().contains("strict descendant"), "{err}");
    }

    /// P1-A07: two reviews from one family satisfy every other check here.
    #[test]
    fn independence_rejects_reviews_that_share_a_family_backend_or_identity() {
        let distinct = vec![
            ("p1".to_owned(), review_record(1, "openai", "codex")),
            ("p2".to_owned(), review_record(2, "xiaomi", "lamu")),
        ];
        verify_reviewer_independence(&distinct)
            .expect("two genuinely distinct reviewers must pass");

        let same_family = vec![
            ("p1".to_owned(), review_record(1, "openai", "codex")),
            ("p2".to_owned(), review_record(2, "openai", "lamu")),
        ];
        let err = verify_reviewer_independence(&same_family)
            .expect_err("two reviews from one model family are not independent");
        assert!(err.to_string().contains("model families"), "{err}");

        let same_backend = vec![
            ("p1".to_owned(), review_record(1, "openai", "codex")),
            ("p2".to_owned(), review_record(2, "anthropic", "codex")),
        ];
        verify_reviewer_independence(&same_backend)
            .expect_err("two reviews on one backend are not independent");
    }

    // ── M17.5 F-27 / P1-A04, P2-F03 ───────────────────────────────────────
    // Both blind passes, independently, found generated evidence unbound: the
    // hash was checked for LENGTH only and the artifact the generator wrote was
    // gitignored, so nothing could read it back.

    fn generated_row(family: &str) -> Generated {
        Generated {
            family: family.to_owned(),
            accepted: 100_000,
            attempts: 100_000,
            discards: 0,
            negatives: 0,
            fuzz_targets: vec!["cst_parse".to_owned()],
            seed_categories: (0..16).map(|i| format!("s{i}")).collect(),
            result: "pass".to_owned(),
            seed: Some(7),
            evidence_hash: Some("ab".repeat(32)),
            relations: Vec::new(),
            oracle: None,
        }
    }

    fn generated_evidence_row(family: &str) -> GeneratedEvidence {
        GeneratedEvidence {
            family: family.to_owned(),
            accepted: 100_000,
            attempts: 100_000,
            discards: 0,
            negatives: 0,
            seed: 7,
            evidence_hash: "ab".repeat(32),
            category_counts: BTreeMap::new(),
            relations: Vec::new(),
            oracle: None,
        }
    }

    #[test]
    fn generated_rows_accept_a_packet_that_matches_its_run() {
        verify_generated_rows(
            &repo_root(),
            &[generated_row("f")],
            &[generated_evidence_row("f")],
        )
        .expect("a matching packet must pass, or every check below is vacuous");
    }

    #[test]
    fn generated_rows_reject_a_hash_no_run_produced() {
        let mut doctored = generated_row("f");
        doctored.evidence_hash = Some("cd".repeat(32));
        let err = verify_generated_rows(&repo_root(), &[doctored], &[generated_evidence_row("f")])
            .expect_err("a packet may not cite a digest the run never recorded");
        assert!(err.to_string().contains("evidence hash"), "{err}");
    }

    #[test]
    fn generated_rows_reject_counts_and_seeds_that_disagree_with_the_run() {
        let mut counts = generated_row("f");
        counts.accepted = 99_999;
        counts.discards = 1;
        verify_generated_rows(&repo_root(), &[counts], &[generated_evidence_row("f")])
            .expect_err("packet counts must match the committed run");

        let mut seed = generated_row("f");
        seed.seed = Some(8);
        verify_generated_rows(&repo_root(), &[seed], &[generated_evidence_row("f")])
            .expect_err("packet seed must match the committed run");
    }

    #[test]
    fn generated_rows_reject_a_family_with_no_committed_run() {
        verify_generated_rows(
            &repo_root(),
            &[generated_row("ghost")],
            &[generated_evidence_row("f")],
        )
        .expect_err("a family claiming pass with no run in the artifact");
    }

    #[test]
    fn passing_generated_family_must_name_closed_relations() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        let artifact: GeneratedEvidenceArtifact = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/evidence/generated.json")).expect("evidence"),
        )
        .expect("generated artifact");
        let row = &artifact.rows[0];
        let family = &mut packet.generated[0];
        family.result = "pass".to_owned();
        family.accepted = row.accepted;
        family.attempts = row.attempts;
        family.discards = row.discards;
        family.seed = Some(row.seed);
        family.evidence_hash = Some(row.evidence_hash.clone());
        family.relations.clear();
        family.oracle = None;
        let err = verify_generated_evidence(&root, &packet)
            .expect_err("a pass with no relation rows is vacuous");
        assert!(
            err.to_string().contains("must declare every relation"),
            "{err}"
        );
    }

    #[test]
    fn generated_oracle_source_must_match_closed_registry() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        let artifact: GeneratedEvidenceArtifact = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/evidence/generated.json")).expect("evidence"),
        )
        .expect("generated artifact");
        let row = &artifact.rows[0];
        let family = &mut packet.generated[0];
        family.result = "pass".to_owned();
        family.accepted = row.accepted;
        family.attempts = row.attempts;
        family.discards = row.discards;
        family.seed = Some(row.seed);
        family.evidence_hash = Some(row.evidence_hash.clone());
        family.oracle.as_mut().expect("oracle").source = "claimed independent prose".to_owned();
        let err = verify_generated_evidence(&root, &packet)
            .expect_err("self-declared oracle prose must not establish independence");
        assert!(err.to_string().contains("oracle source"), "{err}");
    }

    #[test]
    fn generated_artifact_requires_exact_unique_family_set() {
        let scratch = liminal_scratch::ScratchDir::new("haq-generated-artifact").expect("scratch");
        let root: &Utf8Path = &scratch;
        let rows = GENERATED_FAMILIES
            .iter()
            .map(|family| generated_evidence_row(family))
            .collect::<Vec<_>>();
        let artifact = GeneratedEvidenceArtifact {
            schema_version: "haqp-generated-v3-negative-categories".to_owned(),
            artifact_blake3: generated_artifact_digest(&rows).expect("digest"),
            rows: rows.clone(),
        };
        let path = root.join("conformance/haqp/evidence/generated.json");
        fs::create_dir_all(path.parent().expect("evidence parent")).expect("mkdir");
        fs::write(&path, serde_json::to_vec(&artifact).expect("json")).expect("write");
        let packet_rows = GENERATED_FAMILIES
            .iter()
            .map(|family| {
                let mut row = generated_row(family);
                row.result = "planned".to_owned();
                row
            })
            .collect::<Vec<_>>();
        let packet = Packet {
            generated: packet_rows,
            ..read_packet(&repo_root()).expect("packet")
        };
        verify_generated_evidence(root, &packet).expect("exact family artifact must pass");

        let mut duplicate = rows;
        duplicate[0].family = duplicate[1].family.clone();
        let tampered = GeneratedEvidenceArtifact {
            schema_version: "haqp-generated-v3-negative-categories".to_owned(),
            artifact_blake3: generated_artifact_digest(&duplicate).expect("digest"),
            rows: duplicate,
        };
        fs::write(&path, serde_json::to_vec(&tampered).expect("json")).expect("write");
        verify_generated_evidence(root, &packet)
            .expect_err("duplicate or missing family cannot qualify generated evidence");
    }

    /// The inventory layer accepted any 64 characters, including 64 spaces.
    #[test]
    fn inventory_rejects_an_evidence_hash_that_is_not_hexadecimal() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        for family in &mut packet.generated {
            family.result = "pass".to_owned();
            family.seed = Some(1);
            family.evidence_hash = Some(" ".repeat(64));
        }
        let err =
            verify_generated_inventory(&packet).expect_err("64 spaces is not a digest of anything");
        assert!(err.to_string().contains("64-hex"), "{err}");
    }

    // ── M17.5 F-27 / P1-A03 ───────────────────────────────────────────────
    // The qualified path accepted `caught` on every canary row while never
    // invoking the runner. Canaries are what prove the gate can fail at all, so
    // a canary table nobody executed proves nothing.

    fn canary_row(id: &str) -> Canary {
        Canary {
            id: id.to_owned(),
            gate: "status".to_owned(),
            violation: "set status to ratified".to_owned(),
            expected_failure: "status: expected proposed".to_owned(),
            result: "caught".to_owned(),
        }
    }

    fn canary_evidence_row(id: &str) -> CanaryEvidence {
        CanaryEvidence {
            id: id.to_owned(),
            gate: "status".to_owned(),
            violation: "set status to ratified".to_owned(),
            expected_failure: "status: expected proposed".to_owned(),
            observed_failure: "status: expected proposed, found ratified".to_owned(),
            caught: true,
            mutation_semantics: "packet.status := ratified".to_owned(),
        }
    }

    #[test]
    fn canary_rows_accept_a_packet_that_matches_its_run() {
        verify_canary_rows(&[canary_row("C01")], &[canary_evidence_row("C01")])
            .expect("a matching packet must pass, or every check below is vacuous");
    }

    #[test]
    fn canary_rows_reject_a_caught_claim_with_no_run() {
        let err = verify_canary_rows(&[canary_row("C01")], &[])
            .expect_err("`caught` with nothing in the artifact is an assertion, not evidence");
        assert!(err.to_string().contains("no such canary"), "{err}");
    }

    #[test]
    fn canary_rows_reject_a_claim_the_run_contradicts() {
        let mut missed = canary_evidence_row("C01");
        missed.caught = false;
        let err = verify_canary_rows(&[canary_row("C01")], &[missed])
            .expect_err("a packet may not claim caught over a run that recorded a miss");
        assert!(err.to_string().contains("NOT caught"), "{err}");
    }

    /// pass-2 #22 made the runner check its own prose; this extends the same
    /// requirement to the committed artifact.
    #[test]
    fn canary_rows_reject_prose_that_describes_a_different_experiment() {
        let mut relabelled = canary_evidence_row("C01");
        relabelled.violation = "delete a crash boundary".to_owned();
        verify_canary_rows(&[canary_row("C01")], &[relabelled])
            .expect_err("the packet's violation must be the one performed");

        let mut expectation = canary_evidence_row("C01");
        expectation.expected_failure = "something else entirely".to_owned();
        verify_canary_rows(&[canary_row("C01")], &[expectation])
            .expect_err("the packet's expected failure must be the one asserted");
    }

    #[test]
    fn canary_failure_matching_preserves_quoted_diagnostic_detail() {
        assert!(canary_failure_matches(
            "field \"status\" missing",
            "field \"status\" missing: found none"
        ));
        assert!(!canary_failure_matches(
            "field \"status\" missing",
            "field status missing: found none"
        ));
    }

    #[test]
    fn residual_risk_digest_without_coordinate_is_rejected() {
        let mut packet = packet_from_repo();
        packet.residual_risks = vec![ResidualRisk {
            id: "RISK-TEST".to_owned(),
            owner: "owner".to_owned(),
            severity: "low".to_owned(),
            trigger: "trigger".to_owned(),
            requirement: "P1-R001".to_owned(),
            evidence: "a".repeat(64),
            planned_phase: "M24".to_owned(),
            mitigation: "mitigation".to_owned(),
            authority: "ADR-0020".to_owned(),
            state: "open".to_owned(),
        }];
        let err = verify_residual_risks(&repo_root(), &packet)
            .expect_err("an unbound digest cannot identify retained evidence");
        assert!(err.to_string().contains("exact file:coordinate"), "{err}");
    }

    #[test]
    fn canary_rows_reject_wrong_gate_or_mutation_semantics() {
        let mut wrong_gate = canary_evidence_row("C01");
        wrong_gate.gate = "markdown".to_owned();
        verify_canary_rows(&[canary_row("C01")], &[wrong_gate])
            .expect_err("the evidence must name the gate actually exercised");
        let mut wrong_mutation = canary_evidence_row("C01");
        wrong_mutation.mutation_semantics = "no mutation".to_owned();
        verify_canary_rows(&[canary_row("C01")], &[wrong_mutation])
            .expect_err("the exact mutation semantics must be bound");
    }

    #[test]
    fn canary_rows_reject_an_undeclared_canary_in_the_run() {
        verify_canary_rows(
            &[canary_row("C01")],
            &[canary_evidence_row("C01"), canary_evidence_row("C99")],
        )
        .expect_err("a packet that is a subset of the experiment hides rows from the reader");
    }

    #[test]
    fn canary_rows_reject_duplicate_evidence_ids() {
        let duplicate = vec![canary_evidence_row("C01"), canary_evidence_row("C01")];
        let err = verify_canary_rows(&[canary_row("C01")], &duplicate)
            .expect_err("contradictory duplicate evidence must not be silently overwritten");
        assert!(
            err.to_string().contains("duplicate canary evidence id"),
            "{err}"
        );
    }

    /// M17.5 F-27 / P1-A09: the requirement was unrepresentable, so it was
    /// unverifiable.
    #[test]
    fn fuzz_lane_rejects_a_campaign_with_no_sanitizer() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/fuzz.json"))
            .expect("committed fuzz evidence");
        let mut rows: Vec<FuzzEvidence> = serde_json::from_slice(&bytes).expect("parse");
        // The committed artifact predates F-17 and carries no log digest. Each
        // of these tests doctors ONE field to exercise ONE check, so the
        // unrelated missing digest is normalised rather than left to fire first.
        for row in &mut rows {
            row.log_blake3 = "ab".repeat(32);
        }
        let present = rows
            .iter()
            .map(|row| row.target.clone())
            .collect::<BTreeSet<_>>();
        verify_fuzz_rows(&rows, &present).expect("the committed campaign is sanitized");
        rows[0].sanitizer = "none".to_owned();
        let err = verify_fuzz_rows(&rows, &present)
            .expect_err("an unsanitized campaign misses every defect ASan exists to find");
        assert!(err.to_string().contains("sanitizer"), "{err}");
    }

    // ── M17.5 F-28 / P1-A02 ───────────────────────────────────────────────
    // A kill witnessed by a test that cannot run is not a kill.

    #[test]
    fn ignored_attribute_is_attached_to_the_following_test() {
        let mut names = BTreeSet::new();
        collect_ignored(
            "#[test]\n#[ignore = \"Phase 1\"]\nfn quarantined() {}\n#[test]\nfn active() {}\n",
            &mut names,
        );
        assert!(
            names.contains("quarantined"),
            "the ignored test must be seen"
        );
        assert!(
            !names.contains("active"),
            "an #[ignore] must not leak onto the next test after it"
        );
    }

    /// The packet as committed declares every mutant `predeclared`, so the
    /// check is inert — flipping one to `killed` must wake it.
    #[test]
    fn qualified_layer_rejects_a_kill_witnessed_by_an_ignored_test() {
        let root = repo_root();
        let mut packet = read_packet(&root).expect("packet");
        verify_mutant_killing_tests(&root, &packet)
            .expect("no mutant claims killed, so nothing to check");

        // Use P1-T08, the migration test that remains quarantined pending a
        // persisted-format ADR. The active Phase 1 tests cannot witness this
        // negative case after suite breadth is enabled.
        packet.mutants[0].disposition = "killed".to_owned();
        packet.mutants[0].killing_tests = vec!["P1-T08".to_owned()];
        let err = verify_mutant_killing_tests(&root, &packet)
            .expect_err("an #[ignore]d test cannot witness a kill");
        assert!(err.to_string().contains("#[ignore]d"), "{err}");
    }

    #[test]
    fn qualified_layer_rejects_a_kill_with_no_killing_test_at_all() {
        let root = repo_root();
        let mut packet = read_packet(&root).expect("packet");
        packet.mutants[0].disposition = "killed".to_owned();
        packet.mutants[0].killing_tests.clear();
        let err = verify_mutant_killing_tests(&root, &packet)
            .expect_err("a kill with no witness is an assertion");
        assert!(err.to_string().contains("names no killing test"), "{err}");
    }

    // ── ADR-0021: staged qualification ────────────────────────────────────
    // The mutation requirement defers to 1b because ADR-0020 §3 assumed the
    // suite it mutates exists, and for Phase 1 it does not (F-28). Both
    // directions of the stage claim must be errors.

    #[test]
    fn the_committed_packet_declares_a_stage_it_satisfies() {
        let packet = read_packet(&repo_root()).expect("packet");
        assert_eq!(packet.qualification_stage, "1a");
        verify_qualification_stage(&packet).expect("the committed packet must be consistent");
    }

    #[test]
    fn stage_1a_may_not_claim_mutant_kills() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.mutants[0].disposition = "killed".to_owned();
        let err = verify_qualification_stage(&packet)
            .expect_err("1a defers the mutation requirement; claiming a kill smuggles it back");
        assert!(
            err.to_string().contains("defers the mutation requirement"),
            "{err}"
        );
    }

    #[test]
    fn stage_1b_must_actually_carry_the_mutation_evidence() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.qualification_stage = "1b".to_owned();
        let err = verify_qualification_stage(&packet)
            .expect_err("1b without kills claims the stronger stage on the weaker evidence");
        assert!(err.to_string().contains("no mutant is killed"), "{err}");
    }

    #[test]
    fn an_unknown_stage_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.qualification_stage = "1c".to_owned();
        verify_qualification_stage(&packet).expect_err("an unknown stage must not pass");
    }

    // ── M17.5 F-17 / F-29: derived fuzz budget and the family→target mapping ──
    // The packet maps families to targets; the artifact supplies target seconds.

    fn fuzz_row(target: &str, seconds: u64) -> FuzzEvidence {
        FuzzEvidence {
            target: target.to_owned(),
            seconds,
            elapsed_s: seconds,
            exit_code: 0,
            execs: 1,
            artifacts: 0,
            seed: 1,
            seed_count: 16,
            seed_manifest_blake3: "ab".repeat(32),
            sanitizer: "address".to_owned(),
            log: format!("target/haqp/fuzz-{target}.log"),
            log_blake3: "ab".repeat(32),
            sanitizer_proof: None,
        }
    }

    fn family_row(name: &str, targets: &[&str]) -> Generated {
        Generated {
            family: name.to_owned(),
            accepted: 100_000,
            attempts: 100_000,
            discards: 0,
            negatives: 0,
            fuzz_targets: targets.iter().map(|t| (*t).to_owned()).collect(),
            seed_categories: (0..16).map(|i| format!("s{i}")).collect(),
            result: "pass".to_owned(),
            seed: Some(7),
            evidence_hash: Some("ab".repeat(32)),
            relations: Vec::new(),
            oracle: None,
        }
    }

    #[test]
    fn a_family_whose_targets_meet_the_floor_is_accepted() {
        let families = [
            family_row("a", &["t1", "t2"]),
            family_row("b", &["t3"]),
            family_row("c", &["t4"]),
            family_row("d", &["t5"]),
            family_row("e", &["t6"]),
        ];
        let recorded: Vec<FuzzEvidence> = ["t1", "t2", "t3", "t4", "t5", "t6"]
            .iter()
            .map(|t| fuzz_row(t, 1800))
            .collect();
        verify_fuzz_budget(&families, &recorded)
            .expect("6 targets x 30 min = 180 >= 150, every family >= 30");
    }

    /// One campaign must not satisfy two families' budgets.
    #[test]
    fn a_fuzz_target_may_not_be_claimed_by_two_families() {
        let families = [family_row("a", &["shared"]), family_row("b", &["shared"])];
        let err = verify_fuzz_target_mapping(&families)
            .expect_err("a shared target double-counts into two budgets");
        assert!(err.to_string().contains("claimed by both"), "{err}");
    }

    /// ADR-0020 §4 gives EVERY family its own campaign. Two families currently
    /// have no fuzz target at all (F-29), and `pass` must be unreachable for
    /// them until one exists.
    #[test]
    fn a_family_claiming_pass_with_no_fuzz_target_is_refused() {
        let families = [family_row("uncovered", &[])];
        let err = verify_fuzz_budget(&families, &[])
            .expect_err("a family with no target has had no campaign");
        assert!(err.to_string().contains("names no fuzz target"), "{err}");
    }

    #[test]
    fn a_family_below_the_thirty_minute_floor_is_refused() {
        let families = [family_row("a", &["t1"])];
        let err = verify_fuzz_budget(&families, &[fuzz_row("t1", 60)])
            .expect_err("one minute is not thirty");
        assert!(err.to_string().contains("below the 30"), "{err}");
    }

    #[test]
    fn a_family_naming_a_target_the_campaign_never_ran_is_refused() {
        let families = [family_row("a", &["ghost"])];
        verify_fuzz_budget(&families, &[fuzz_row("t1", 1800)])
            .expect_err("a target with no recorded run evidences nothing");
    }

    /// F-17: the log lives under target/, so its digest is the only thing that
    /// can make the recorded counts checkable afterwards.
    #[test]
    fn fuzz_rows_require_a_real_log_digest() {
        let present = BTreeSet::from(["t1".to_owned()]);
        let good = vec![fuzz_row("t1", 9000)];
        verify_fuzz_rows(&good, &present).expect("a well-formed row must pass");

        let mut empty = good.clone();
        empty[0].log_blake3 = String::new();
        verify_fuzz_rows(&empty, &present)
            .expect_err("an absent digest leaves the counts unfalsifiable");

        let mut garbage = good;
        garbage[0].log_blake3 = " ".repeat(64);
        verify_fuzz_rows(&garbage, &present).expect_err("64 spaces is not a digest");
    }
    /// The seed manifest must be a function of TRACKED state alone
    /// (M17.5 F-32), and whether the COMMITTED evidence currently satisfies it
    /// is a qualified-lane question, not a CI one.
    ///
    /// This test used to read `conformance/haqp/evidence/fuzz.json` and assert
    /// it was valid right now. That assertion can only hold at a metadata
    /// child: evidence is produced at the fixed base and committed in the
    /// child, so at any base it describes the PREVIOUS campaign. It passed only
    /// because this machine's corpus happened to match the last run — killing a
    /// campaign added 378 inputs and turned a green tree red with no code
    /// change. `verify_fuzz_seed_manifests` still makes the binding, in
    /// `verify_qualified_repo`, where evidence is required to describe the tree.
    ///
    /// What CI can assert deterministically is the property itself: the digest
    /// counts tracked seeds, and nothing else.
    #[test]
    fn the_seed_manifest_counts_tracked_seeds_only() {
        let root = repo_root();
        let target = "fuzz/corpus/cst_parse";
        let (count, digest) =
            seed_manifest_digest(&root, &root.join(target)).expect("tracked seeds must digest");

        let tracked = git_text(&root, &["ls-files", "--", target])
            .expect("git ls-files")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count() as u64;
        assert_eq!(
            count, tracked,
            "the manifest must count TRACKED seeds; a fresh clone has to reproduce this"
        );
        assert!(
            count >= 16,
            "ADR-0020 §4 requires at least 16 predeclared seeds, found {count}"
        );

        // Determinism: the same tracked set must digest identically. Anything
        // that varied with local fuzzer output would reintroduce F-32.
        let (again, again_digest) =
            seed_manifest_digest(&root, &root.join(target)).expect("second digest");
        assert_eq!((count, digest), (again, again_digest));

        // ...and the on-disk directory is much larger, which is precisely why
        // reading it was wrong.
        let on_disk = fs::read_dir(root.join(target))
            .expect("corpus dir")
            .flatten()
            .count() as u64;
        assert!(
            on_disk >= count,
            "sanity: the working corpus cannot be smaller than the tracked one"
        );
    }

    // ── M17.5 Stage 3, tier 1: the verifier's own survivors ────────────────
    // A scoped campaign over haq.rs left 75. The ones below are in gate logic
    // written THIS session, and they share a cause worth naming: the canaries
    // asserted that bad input is rejected and never that good input is
    // accepted, so any mutant that widens a guard survived. That is the exact
    // defect this milestone has been finding elsewhere.

    /// Kills `== ParentDir` -> `!=` at the evidence-path guard: under `!=`
    /// every ordinary component looks like an escape and nothing can pass.
    #[test]
    fn an_ordinary_relative_evidence_path_is_accepted() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.reviews[0].result = "pass".to_owned();
        packet.reviews[0].evidence = Some("conformance/haqp/evidence/nope.json".to_owned());
        let err = verify_review_evidence(&repo_root(), &packet)
            .expect_err("the file does not exist, so this must fail on ABSENCE");
        assert!(
            err.to_string().contains("record is missing"),
            "a plain relative path must reach the read, not be refused as an escape: {err}"
        );
    }

    #[test]
    fn resolution_coordinates_must_land_on_added_diff_lines() {
        let diff = "@@ -10,2 +10,3 @@\n-old\n+new\n+anchor\n";
        assert!(diff_contains_added_line(diff, 10));
        assert!(diff_contains_added_line(diff, 11));
        assert!(diff_contains_added_line(diff, 12));
        assert!(!diff_contains_added_line(diff, 9));
        assert!(!diff_contains_added_line(diff, 13));
        assert!(!diff_contains_added_line("@@ -3,1 +3,0 @@\n-old\n", 3));
    }

    #[test]
    fn locked_acceptance_paths_are_rejected_by_component() {
        assert!(is_locked_acceptance_path("conformance/corpora/dev/input"));
        assert!(is_locked_acceptance_path("CONFORMANCE\\CORPORA\\heldout"));
        assert!(is_locked_acceptance_path("fixtures/heldout/input"));
        assert!(!is_locked_acceptance_path("conformance/fixtures/input"));
    }

    /// Kills the `1b if !claims_kills` guard -> `true`: a 1b packet that DOES
    /// carry kills must be accepted, or the stage can never be satisfied.
    #[test]
    fn stage_1b_with_kills_is_accepted() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.qualification_stage = "1b".to_owned();
        for mutant in &mut packet.mutants {
            mutant.disposition = "killed".to_owned();
        }
        verify_qualification_stage(&packet)
            .expect("1b carrying mutation evidence is the whole point of 1b");
    }

    /// Kills both boolean mutants in the non-runnable scanner.
    #[test]
    fn the_scanner_recognises_each_non_runnable_form_and_nothing_else() {
        let mut names = BTreeSet::new();
        collect_ignored(
            "#[cfg(any())]\nfn compiled_out() {}\n\
             #[cfg_attr(unix, ignore)]\nfn conditionally_ignored() {}\n\
             // a comment mentioning ignore\nfn merely_mentioned() {}\n\
             #[cfg_attr(unix, should_panic)]\nfn other_cfg_attr() {}\n",
            &mut names,
        );
        assert!(names.contains("compiled_out"), "#[cfg(any())] never runs");
        assert!(
            names.contains("conditionally_ignored"),
            "a conditional ignore still cannot witness a kill"
        );
        assert!(
            !names.contains("merely_mentioned"),
            "prose containing the word must not mark the next test non-runnable"
        );
        assert!(
            !names.contains("other_cfg_attr"),
            "a cfg_attr that is not an ignore leaves the test runnable"
        );
    }

    /// Kills `seconds / 60` -> `* 60`, `< 150` -> `>`, `30 * 60` -> `+`/`/`,
    /// and `elapsed_s + 60` -> `*` / `<=`. Each bound is checked from BOTH
    /// sides, because a one-sided check is satisfied by a mutant that moves it.
    #[test]
    fn fuzz_row_floors_hold_exactly_at_their_boundaries() {
        let targets: BTreeSet<String> = (0..5).map(|i| format!("t{i}")).collect();
        let rows = |secs: u64| -> Vec<FuzzEvidence> {
            (0..5).map(|i| fuzz_row(&format!("t{i}"), secs)).collect()
        };

        // 5 x 1800s = 150 minutes exactly: the floor is inclusive.
        verify_fuzz_rows(&rows(1800), &targets).expect("exactly 150 target-minutes must pass");
        // One second under the per-target floor must fail.
        verify_fuzz_rows(&rows(1799), &targets)
            .expect_err("1799s is below the 30-minute per-target floor");

        // The TOTAL floor has to be reached on its own. Five targets at 1800s
        // can never breach it, so the first version of this test never
        // executed line 766 at all and three mutants there survived a check
        // that looked like it covered them. Four targets do reach it.
        let four: BTreeSet<String> = (0..4).map(|i| format!("t{i}")).collect();
        let four_rows: Vec<FuzzEvidence> =
            (0..4).map(|i| fuzz_row(&format!("t{i}"), 1800)).collect();
        verify_fuzz_rows(&four_rows, &four)
            .expect_err("4 x 30 = 120 target-minutes is below the 150 total");

        // And the per-target floor must be reached with the total SATISFIED,
        // or a mutant that moves the floor down (30*60 -> 30+60, or -> 30/60)
        // is never exercised: those need a target above 90s and below 1800s.
        let six: BTreeSet<String> = (0..6).map(|i| format!("t{i}")).collect();
        let mut six_rows: Vec<FuzzEvidence> =
            (0..6).map(|i| fuzz_row(&format!("t{i}"), 1800)).collect();
        six_rows[5].seconds = 600;
        six_rows[5].elapsed_s = 600;
        verify_fuzz_rows(&six_rows, &six)
            .expect_err("600s is above 90 and below 1800: only the real floor rejects it");

        // The early-exit check compares elapsed against budget with 60s slack.
        let mut exact = rows(1800);
        exact[0].elapsed_s = 1740; // 1740 + 60 == 1800, the boundary
        verify_fuzz_rows(&exact, &targets).expect("elapsed + 60 == seconds is not an early exit");
        let mut early = rows(1800);
        early[0].elapsed_s = 1739;
        verify_fuzz_rows(&early, &targets)
            .expect_err("a clean campaign that stopped early states a contradiction");
    }

    /// Kills `< 150` -> `==` / `<=` in the DERIVED budget: 150 exactly passes,
    /// 149 fails.
    #[test]
    fn the_derived_budget_floor_is_inclusive() {
        let families: Vec<Generated> = (0..5)
            .map(|i| family_row(&format!("f{i}"), &[&format!("t{i}")]))
            .collect();
        let recorded: Vec<FuzzEvidence> =
            (0..5).map(|i| fuzz_row(&format!("t{i}"), 1800)).collect();
        verify_fuzz_budget(&families, &recorded).expect("exactly 150 derived minutes must pass");

        let short: Vec<FuzzEvidence> = recorded
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mut row = row.clone();
                if i == 0 {
                    row.seconds = 1740; // requested 29 minutes
                    row.elapsed_s = 1740; // measured 29 minutes: family and total floor
                }
                row
            })
            .collect();
        verify_fuzz_budget(&families, &short).expect_err("149 derived minutes is below 150");
    }

    // ── M17.5 F-30: the repo-level gates had no test at all ────────────────
    // `verify_inventory_repo`, `verify_qualified_repo` and every evidence
    // binder survived being replaced with `Ok(())`. They are exercised only by
    // justfile recipes, so `just ci` catches a break and the MUTATION lane
    // cannot see them — a gate invisible to the measurement that judges the
    // suite. Each is now driven directly.

    #[test]
    fn the_inventory_gate_accepts_the_committed_tree() {
        verify_inventory_repo(&repo_root())
            .expect("the committed packet must satisfy its own inventory gate");
    }

    /// The packet is `proposed`/`not-run`, so the qualified gate must REFUSE
    /// it — and refuse it for that reason, not by accident.
    #[test]
    fn the_qualified_gate_refuses_a_packet_that_has_not_run() {
        let err =
            verify_qualified_repo(&repo_root()).expect_err("a not-run packet is not qualified");
        assert!(
            err.to_string().contains("qualification_state"),
            "it must refuse for the stated reason: {err}"
        );
    }

    #[test]
    fn markdown_surface_rejects_a_stale_qualification_state() {
        let root = repo_root();
        let mut packet = packet_from_repo();
        let markdown = fs::read_to_string(root.join("docs/execution/phase1-suite-review.md"))
            .expect("review markdown");
        let old_digest = blake3::hash(serde_json::to_vec(&packet).expect("packet json").as_slice())
            .to_hex()
            .to_string();
        packet.qualification_state = "complete".to_owned();
        let new_digest = blake3::hash(
            serde_json::to_vec(&packet)
                .expect("complete packet json")
                .as_slice(),
        )
        .to_hex()
        .to_string();
        let doctored = markdown.replace(&old_digest, &new_digest);
        let err = verify_markdown_surface_text(&doctored, &packet)
            .expect_err("completed packet may not retain NOT_RUN markdown");
        assert!(err.to_string().contains("qualification state"), "{err}");

        // Derived from the packet, not pinned to a literal: adding canary C33
        // made the old hardcoded "32" a no-op replace, so this test silently
        // stopped exercising the canary-target check and failed on the previous
        // assertion instead. A fixture that names a count must read it.
        let stale_count = markdown.replace(
            &format!(
                "Target: exactly **{} predeclared canaries**",
                packet.canaries.len()
            ),
            "Target: exactly **16 predeclared canaries**",
        );
        assert_ne!(
            stale_count, markdown,
            "the doctored markdown must differ, or the check below proves nothing"
        );
        let err = verify_markdown_surface_text(&stale_count, &packet)
            .expect_err("markdown cannot report a stale canary inventory count");
        assert!(err.to_string().contains("canary target"), "{err}");
    }

    /// Each evidence binder must actually READ its artifact. Replacing any of
    /// them with `Ok(())` was invisible because the qualified gate's
    /// `qualification_state` check fires first and they were never reached.
    #[test]
    fn every_evidence_binder_rejects_a_claim_its_artifact_contradicts() {
        let root = repo_root();

        let mut generated = read_packet(&root).expect("packet");
        generated.generated[0].result = "pass".to_owned();
        generated.generated[0].seed = Some(999_999);
        generated.generated[0].evidence_hash = Some("cd".repeat(32));
        verify_generated_evidence(&root, &generated)
            .expect_err("a generated family citing a run that did not happen");

        let mut canaries = read_packet(&root).expect("packet");
        canaries.canaries[0].result = "caught".to_owned();
        canaries.canaries[0].violation = "something the run never performed".to_owned();
        verify_canary_evidence(&root, &canaries)
            .expect_err("a canary whose prose contradicts the committed run");

        let mut crash = read_packet(&root).expect("packet");
        crash.crash_boundaries.remove(0);
        verify_crash_evidence(&root, &crash)
            .expect_err("a packet that drops a boundary the fault lane exercised");

        let mut fuzz = read_packet(&root).expect("packet");
        for family in &mut fuzz.generated {
            family.result = "pass".to_owned();
        }
        fuzz.generated[0].fuzz_targets[0] = "never_ran".to_owned();
        verify_fuzz_evidence(&root, &fuzz)
            .expect_err("a packet claiming an unrecorded target cannot qualify");

        let mut markdown = read_packet(&root).expect("packet");
        markdown.suite_version = "tampered".to_owned();
        verify_markdown_surface(&root, &markdown)
            .expect_err("a packet whose digest no longer matches the reviewed markdown");
    }

    /// `verify_provenance` binds qualification to a specific fixed commit and
    /// a clean tree; replacing it with `Ok(())` unbinds the packet from any
    /// tree. The packet itself is expected to be that commit's child.
    #[test]
    fn provenance_rejects_a_packet_bound_to_another_commit() {
        let root = repo_root();
        let mut packet = read_packet(&root).expect("packet");
        if let Some(provenance) = packet.provenance.as_mut() {
            provenance.fixed_commit = "0".repeat(40);
            verify_provenance(&root, &packet)
                .expect_err("a packet naming a commit that is not HEAD is bound to nothing");
        } else {
            verify_provenance(&root, &packet)
                .expect_err("a packet with no provenance block is bound to nothing");
        }
    }

    /// Kills `verify_inventory_repo -> Ok(())`.
    ///
    /// The earlier test asserted the gate ACCEPTS the committed tree, which
    /// `Ok(())` also does — so the mutant survived a test written for it. A
    /// gate is only pinned by a tree it must REFUSE, which means building one.
    #[test]
    fn the_inventory_gate_refuses_a_doctored_tree() {
        let scratch = liminal_scratch::ScratchDir::new("haq-inventory-gate").expect("scratch dir");
        let root: &Utf8Path = &scratch;
        let source = repo_root();

        for rel in [
            "conformance/haqp/packet.json",
            "docs/execution/phase1-suite-review.md",
            // The inventory gate reads the independent oracles (F-48 A02), and
            // since F-61 five of the six live in this file.
            "crates/liminal-query/src/lib.rs",
            "crates/liminal-xtask/src/haq.rs",
        ] {
            let dest = root.join(rel);
            fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
            fs::copy(source.join(rel), &dest).expect("copy fixture");
        }
        let packet: Packet = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/packet.json")).expect("read packet fixture"),
        )
        .expect("parse packet fixture");
        for requirement in &packet.requirements {
            let source_file = requirement
                .source
                .split_once(':')
                .expect("requirement source coordinate")
                .0;
            let dest = root.join(source_file);
            fs::create_dir_all(dest.parent().expect("source parent")).expect("mkdir");
            fs::copy(source.join(source_file), &dest).expect("copy requirement source");
        }
        // F-65 moved the plan's gates into the inventory path, so the copy
        // needs every file a mutant anchors in, and a locked corpus directory.
        // A placeholder, not the real corpus: the alias scan walks for aliases
        // and never reads corpus bytes, and the fixture must not either.
        for mutant in &packet.mutants {
            let file = mutant
                .source
                .rsplit_once(':')
                .expect("mutant source coordinate")
                .0;
            let dest = root.join(file);
            fs::create_dir_all(dest.parent().expect("anchor parent")).expect("mkdir");
            fs::copy(source.join(file), &dest).expect("copy anchor source");
        }
        let heldout = root.join("conformance/corpora/heldout");
        fs::create_dir_all(heldout.as_std_path()).expect("mkdir corpus");
        fs::write(heldout.join("placeholder"), b"not the locked corpus").expect("placeholder");

        for rel in [
            "conformance/haqp/evidence/crash.json",
            "conformance/haqp/evidence/reviews/pass1-codex-gpt-5.6-sol.json",
            "conformance/haqp/evidence/reviews/pass1-codex-gpt-5.6-sol.raw.txt",
            "conformance/haqp/evidence/reviews/pass1-codex-gpt-5.6-sol.session.jsonl",
            "conformance/haqp/evidence/reviews/pass2-mimo-direct-mimo-v2.5-pro.json",
            "conformance/haqp/evidence/reviews/pass2-mimo-direct-mimo-v2.5-pro.raw.txt",
        ] {
            let dest = root.join(rel);
            fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
            fs::copy(source.join(rel), &dest).expect("copy evidence fixture");
        }

        // Sanity: the COPY must pass, or the refusal below proves nothing about
        // the doctoring.
        verify_inventory_repo(root).expect("an unmodified copy must still pass");

        // Blind pass 2 at `ec045588` (A05): the crash terminal-state oracle ran
        // only on the qualified path, which refuses before it starts unless
        // the packet is already qualified — so at stage 1a it had never
        // judged anything. Doctoring recovery must refuse BEFORE the flip.
        let crash_path = root.join("conformance/haqp/evidence/crash.json");
        let mut crash: serde_json::Value =
            serde_json::from_slice(&fs::read(&crash_path).expect("read")).expect("parse");
        for boundary in crash["boundaries"].as_array_mut().expect("boundaries") {
            if boundary["boundary"] == "ilrp/before_intent_commit" {
                for pair in boundary["recovery_pairs"].as_array_mut().expect("pairs") {
                    pair["first_terminals"] = serde_json::json!(["Committed"]);
                }
            }
        }
        fs::write(&crash_path, serde_json::to_vec(&crash).expect("encode")).expect("write");
        let err = verify_inventory_repo(root).expect_err("invented recovery work");
        assert!(err.to_string().contains("the protocol requires"), "{err}");
        fs::copy(
            source.join("conformance/haqp/evidence/crash.json"),
            &crash_path,
        )
        .expect("restore");
        verify_inventory_repo(root).expect("the restored copy passes again");

        // A proposed packet may not declare itself ratified.
        let path = root.join("conformance/haqp/packet.json");
        let mut packet: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read")).expect("parse");
        packet["ratification"] = serde_json::Value::String("ratified".to_owned());
        fs::write(
            &path,
            serde_json::to_vec_pretty(&packet).expect("serialize"),
        )
        .expect("write");

        verify_inventory_repo(root)
            .expect_err("a packet declaring itself ratified must be refused");
    }

    // ── M17.5 F-30: ADR-0020 §4's generator machinery ─────────────────────
    // The 100,000-case evidence is only as good as the generator behind it. A
    // constant RNG, or a `word()` that returns one string, produces 100,000
    // "cases" that are one case repeated — and every count in the packet would
    // still look satisfied.

    /// Kills `^` -> `|`/`&` and `>>` -> `<<` in `Rng::next`.
    ///
    /// Pinned as a GOLDEN prefix from a fixed seed rather than by a statistical
    /// property: the recorded packet seed exists to make a campaign
    /// reproducible (ADR-0020 §1), and reproducibility is exactly "this seed
    /// yields this stream".
    #[test]
    fn the_seeded_stream_is_a_fixed_sequence() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let drawn: Vec<u64> = (0..4).map(|_| rng.next()).collect();
        let mut again = Rng(0x9E37_79B9_7F4A_7C15);
        let repeat: Vec<u64> = (0..4).map(|_| again.next()).collect();
        assert_eq!(drawn, repeat, "the same seed must yield the same stream");
        // Distinct draws: a stream that repeats one value reproduces nothing.
        assert!(
            drawn.iter().collect::<BTreeSet<_>>().len() == 4,
            "four draws collapsed to fewer values: {drawn:?}"
        );
    }

    /// Kills `Rng::below -> 1`, and pins the contract callers rely on.
    #[test]
    fn below_stays_in_range_and_actually_varies() {
        let mut rng = Rng(12_345);
        let draws: Vec<u64> = (0..500).map(|_| rng.below(7)).collect();
        assert!(draws.iter().all(|d| *d < 7), "below(7) escaped its bound");
        assert!(
            draws.iter().collect::<BTreeSet<_>>().len() >= 5,
            "below(7) produced fewer than 5 distinct values in 500 draws — a \
             generator that returns a constant makes 100,000 cases into one"
        );
        // below(0) must not divide by zero.
        assert_eq!(rng.below(0), 0, "below(0) must be total");
    }

    /// Kills `Rng::word -> "xyzzy"` and `+` -> `*` in its length.
    #[test]
    fn words_vary_in_content_and_length() {
        let mut rng = Rng(999);
        let words: Vec<String> = (0..200).map(|_| rng.word()).collect();
        assert!(
            words.iter().collect::<BTreeSet<_>>().len() > 100,
            "200 draws produced fewer than 100 distinct words"
        );
        for word in &words {
            assert!(
                (1..=12).contains(&word.len()),
                "word {word:?} has length {} outside 1..=12",
                word.len()
            );
            assert!(
                word.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b" -_".contains(&b)),
                "word {word:?} left the declared alphabet"
            );
        }
        assert!(
            words.iter().any(|w| w.len() > 1),
            "`1 + below(12)` became `1 * below(12)` if every word is one char"
        );
    }

    /// Kills `>` -> `>=`/`==` at the safety limit and `== 0` at the floor.
    /// Both boundaries, from both sides — the reason the guard was extracted.
    #[test]
    fn the_case_count_guard_holds_exactly_at_its_boundaries() {
        validate_case_count(0).expect_err("zero cases is not a campaign");
        validate_case_count(1).expect("one case is a legal, if small, request");
        validate_case_count(10_000_000).expect("the limit itself must be allowed");
        validate_case_count(10_000_001).expect_err("one past the limit must be refused");
    }

    /// Kills the three `generate_evidence -> Ok((vec![], ...))` replacements.
    ///
    /// `generated_evidence_is_byte_identical_across_runs` compares two runs to
    /// each other, and two EMPTY runs are identical — so it passed under every
    /// one of them. Shape has to be asserted directly.
    #[test]
    fn generated_evidence_has_one_row_per_family_that_met_its_target() {
        let (rows, timings) = generate_evidence(64).expect("a small run must succeed");
        assert_eq!(rows.len(), 5, "ADR-0020 §4 names five critical families");
        assert_eq!(timings.len(), 5, "one timing per family");
        let families: BTreeSet<&str> = rows.iter().map(|row| row.family.as_str()).collect();
        assert_eq!(families.len(), 5, "family names must be distinct");
        for row in &rows {
            assert_eq!(
                row.accepted, 64,
                "{}: acceptance target not met",
                row.family
            );
            assert_eq!(
                row.accepted + row.discards + row.negatives,
                row.attempts,
                "{}: counts do not describe one run",
                row.family
            );
            assert_eq!(row.evidence_hash.len(), 64, "{}: no digest", row.family);
        }
    }

    /// Golden digests for a fixed 64-case run — the strongest available pin on
    /// the case BUILDERS (M17.5 F-30).
    ///
    /// `case_repair` alone left eight survivors, all in the cyclic-dependency
    /// construction: edge density, the `count >= 2` guard, and four operators
    /// inside the `retain` that clears the back edge before closing a real
    /// cycle. Eight behavioural assertions over an RNG-driven builder would be
    /// fragile and would still miss combinations; the digest covers every arm
    /// at once, because any change to what a case CONTAINS changes it.
    ///
    /// `generated_evidence_is_byte_identical_across_runs` cannot do this job:
    /// it compares two runs to each other, so it is satisfied by any builder
    /// that is merely consistent — including a mutated one.
    ///
    /// These constants are expected to change when a generator is deliberately
    /// changed. That is the point: an unexplained change here means a case
    /// builder moved, and ADR-0020 §1 makes an unreproducible campaign
    /// ineligible. Re-record only alongside the change that caused it.
    #[test]
    fn generated_case_builders_match_their_recorded_goldens() {
        const GOLDENS: [(&str, &str); 5] = [
            (
                "source/CST/formatting",
                "1311bee6bb9ad19f27f81430eb62d0258700e4fdd43fae27f19d8f86f9476f11",
            ),
            (
                "graph/interchange codecs",
                "ca99f0374a5e69c0d1e20954b8056859ffee020ff2672658eed28c8f1684bb86",
            ),
            (
                "transforms/projections",
                "3c56f417ccf4334d819c7600c1feb1143d32530de59130be531aaba991cba40d",
            ),
            (
                "repair/ILRP/recovery",
                "c2ff73e22df13d34ca85c40c357ace4655c0d4a3cf796c873c20933019096ddf",
            ),
            (
                "Basis/revision/query invalidation",
                "96737e4564fe30e9c0bb224c6fa34c36b369663b49910406503cde551dd85ac0",
            ),
        ];
        // 3,000 rather than 64: `case_repair` closes a genuine cycle on
        // roughly 1 case in 384, so a 64-case run never executes the branch and
        // six mutants inside it survived a golden that appeared to cover them.
        // 3,000 exercises every arm and still runs in ~0.6s.
        let (rows, _) = generate_evidence(3_000).expect("a small run must succeed");
        for (row, (family, digest)) in rows.iter().zip(GOLDENS) {
            assert_eq!(row.family, family, "family order must be stable");
            assert_eq!(
                row.evidence_hash, digest,
                "{family}: a case builder changed without its golden being re-recorded"
            );
        }
    }

    // ── M17.5 F-30: inventory thresholds, from BOTH sides ─────────────────
    // Seven survivors sat on thresholds that were only ever tested from the
    // failing side, so any mutant that moved a bound by one survived. A bound
    // is defined by the pair of values that straddle it.

    /// `kind.is_empty() || source.is_empty()` — kills `||` -> `&&`, which would
    /// accept a requirement missing ONE of the two.
    #[test]
    fn a_requirement_missing_either_field_is_refused() {
        for (kind, source) in [("", "v4 §112"), ("law", "")] {
            let mut packet = read_packet(&repo_root()).expect("packet");
            packet.requirements[0].kind = kind.to_owned();
            packet.requirements[0].source = source.to_owned();
            verify_packet_shape(&packet)
                .expect_err("a requirement missing kind OR source is incomplete");
        }
    }

    #[test]
    fn an_unknown_requirement_kind_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.requirements[0].kind = "stateful".to_owned();
        let err = verify_packet_shape(&packet).expect_err("kind vocabulary must be closed");
        assert!(err.to_string().contains("unknown kind"), "{err}");
    }

    #[test]
    fn requirement_coordinate_anchor_is_not_a_substring() {
        assert!(has_exact_coordinate_anchor(
            "- **D18.1 —** exact source\n",
            "D18.1"
        ));
        assert!(!has_exact_coordinate_anchor(
            "- **D18.10 —** another source\n",
            "D18.1"
        ));
        assert!(has_exact_coordinate_anchor(
            "## Exit gate\n",
            "## Exit gate"
        ));
        assert!(!has_exact_coordinate_anchor(
            "## Exit criteria\n",
            "## Exit gate"
        ));
    }

    #[test]
    fn mutation_failure_classifier_requires_named_test_failure() {
        assert!(is_semantic_test_failure(
            "tests::target",
            b"FAIL [0.01s] tests::target\n",
            b""
        ));
        assert!(is_semantic_test_failure(
            "tests::target",
            b"",
            b"FAIL [0.01s] tests::target\n"
        ));
        assert!(!is_semantic_test_failure(
            "tests::target",
            b"launcher FAILED before tests\n",
            b""
        ));
    }

    #[test]
    fn packet_cannot_demote_authoritative_criticality() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.requirements[0].critical = false;
        let err = verify_packet_shape(&packet)
            .expect_err("criticality is closed authority, not packet-controlled metadata");
        assert!(
            err.to_string().contains("closed authoritative registry"),
            "{err}"
        );
    }

    #[test]
    fn traced_absolute_path_outside_root_cannot_rebase_on_fuzz_suffix() {
        let scratch = liminal_scratch::ScratchDir::new("haq-trace-root").expect("scratch");
        let trace_root = scratch.join("trace-root");
        fs::create_dir_all(&trace_root).expect("trace root");
        let err = trace_path_to_repo(
            &scratch,
            trace_root.as_std_path(),
            "/attacker/fuzz/corpus/cst_parse/input",
        )
        .expect_err("an external absolute path must not be rebased by suffix");
        assert!(
            err.to_string().contains("outside recorded trace root"),
            "{err}"
        );
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the corpus trace regression covers manifest and raw-trace attacks together"
    )]
    #[test]
    fn corpus_access_audit_rejects_forbidden_paths_and_accepts_unlocked_paths() {
        let scratch = liminal_scratch::ScratchDir::new("haq-corpus-audit").expect("scratch");
        let root: &Utf8Path = &scratch;
        let packet = packet_from_repo();
        let targets = packet
            .generated
            .iter()
            .flat_map(|family| family.fuzz_targets.iter().cloned())
            .collect::<BTreeSet<_>>();
        let mut rows = Vec::new();
        let mut fuzz_rows = Vec::new();
        let tracer_version = Command::new("strace")
            .arg("-V")
            .output()
            .expect("strace version");
        let mut tracer_version_bytes = tracer_version.stdout;
        tracer_version_bytes.extend_from_slice(&tracer_version.stderr);
        let tracer_version_blake3 = blake3::hash(&tracer_version_bytes).to_hex().to_string();
        let tracer_version_path = root.join("conformance/haqp/evidence/access/strace.version");
        fs::create_dir_all(tracer_version_path.parent().expect("tracer version parent"))
            .expect("mkdir tracer version");
        fs::write(&tracer_version_path, &tracer_version_bytes).expect("write tracer version");
        let trace_root = root
            .as_std_path()
            .canonicalize()
            .expect("canonical scratch root")
            .to_str()
            .expect("utf8 scratch root")
            .to_owned();
        for target in &targets {
            let manifest = format!("conformance/haqp/evidence/access/{target}.paths");
            let path = root.join(&manifest);
            fs::create_dir_all(path.parent().expect("manifest parent")).expect("mkdir");
            let corpus_dir = root.join(format!("fuzz/corpus/{target}"));
            fs::create_dir_all(&corpus_dir).expect("mkdir corpus");
            fs::write(&path, format!("{trace_root}/fuzz/corpus/{target}\n")).expect("write");
            let trace = format!("conformance/haqp/evidence/access/{target}.trace");
            let trace_path = root.join(&trace);
            fs::write(
                &trace_path,
                format!(
                    "2 openat(AT_FDCWD, \"{trace_root}/fuzz/target/x86_64-unknown-linux-gnu/release/{target}\", O_RDONLY) = 3\n2 openat(AT_FDCWD, \"{trace_root}/fuzz/corpus/{target}\", O_RDONLY) = 3\n2 openat(AT_FDCWD, \"{trace_root}/fuzz/corpus/{target}/deleted-after-open\", O_RDONLY) = 3\n1 --- SIGCHLD {{si_signo=SIGCHLD, si_pid=2, si_status=0}} ---\n1 +++ exited with 0 +++\n"
                ),
            )
            .expect("trace");
            let trace_hash = blake3::hash(&fs::read(&trace_path).expect("read trace"))
                .to_hex()
                .to_string();
            let command = format!("strace -f cargo fuzz run {target}");
            let binding =
                blake3::hash(format!("{}\0{}\0{}\0{}", command, 1, 0, trace_hash).as_bytes())
                    .to_hex()
                    .to_string();
            rows.push(CorpusAccessAuditTarget {
                target: target.clone(),
                manifest,
                manifest_blake3: blake3::hash(&fs::read(&path).expect("read"))
                    .to_hex()
                    .to_string(),
                seed: 1,
                sanitizer: "address".to_owned(),
                exit_code: 0,
                log_blake3: "a".repeat(64),
                command,
                trace,
                trace_blake3: trace_hash,
                trace_pid: 1,
                trace_exit_code: 0,
                trace_complete: true,
                process_binding: binding,
                tracer_binary: "strace".to_owned(),
                tracer_version: "conformance/haqp/evidence/access/strace.version".to_owned(),
                tracer_version_blake3: tracer_version_blake3.clone(),
                trace_root: trace_root.clone(),
                resolved_paths_blake3: blake3::hash(
                    format!(
                        "{trace_root}/fuzz/corpus/{target}\0fuzz/corpus/{target}\n{trace_root}/fuzz/corpus/{target}/deleted-after-open\0fuzz/corpus/{target}/deleted-after-open\n"
                    )
                    .as_bytes(),
                )
                .to_hex()
                .to_string(),
            });
            fuzz_rows.push(FuzzEvidence {
                target: target.clone(),
                seconds: 1800,
                elapsed_s: 1800,
                exit_code: 0,
                execs: 1,
                artifacts: 0,
                seed: 1,
                seed_count: 16,
                seed_manifest_blake3: "a".repeat(64),
                sanitizer: "address".to_owned(),
                log: format!("conformance/haqp/evidence/logs/{target}.log"),
                log_blake3: "a".repeat(64),
                sanitizer_proof: None,
            });
        }
        let fuzz_path = root.join("conformance/haqp/evidence/fuzz.json");
        fs::create_dir_all(fuzz_path.parent().expect("fuzz parent")).expect("mkdir");
        fs::write(
            &fuzz_path,
            serde_json::to_vec(&fuzz_rows).expect("serialize fuzz"),
        )
        .expect("write fuzz");
        let audit = CorpusAccessAudit {
            schema_version: "haqp-corpus-access-v2".to_owned(),
            tracer: "strace-open-paths".to_owned(),
            source_commit: "a".repeat(40),
            source_tree: "b".repeat(40),
            targets: rows,
            scope_traces: Vec::new(),
        };
        let audit_path = root.join("conformance/haqp/evidence/corpus-access.json");
        fs::write(&audit_path, serde_json::to_vec(&audit).expect("serialize")).expect("write");
        verify_corpus_access_audit(root, &packet).expect("unlocked corpus paths are accepted");

        let first = audit.targets[0].clone();
        // Lexical paths can stay benign while the fuzz corpus directory is a
        // symlink into locked data. Canonical resolution must reject this
        // without relying on a forbidden string in the raw trace.
        let heldout_dir = root.join("conformance/corpora/heldout");
        fs::create_dir_all(&heldout_dir).expect("mkdir scratch heldout");
        fs::write(heldout_dir.join("secret"), b"fixture").expect("write scratch heldout");
        let corpus_dir = root.join(format!("fuzz/corpus/{}", first.target));
        fs::remove_dir(&corpus_dir).expect("remove corpus directory");
        std::os::unix::fs::symlink(&heldout_dir, &corpus_dir).expect("symlink corpus directory");
        verify_corpus_access_audit(root, &packet)
            .expect_err("symlinked corpus resolving into held-out data must fail closed");
        fs::remove_file(&corpus_dir).expect("remove corpus symlink");
        fs::create_dir(&corpus_dir).expect("restore corpus directory");

        // A second alias outside fuzz/corpus must also be resolved. Filtering
        // only lexical corpus paths lets this symlink reach locked data
        // without a forbidden token in the raw trace.
        let alias = root.join("benign-alias");
        std::os::unix::fs::symlink(&heldout_dir, &alias).expect("symlink benign alias");
        let mut aliased = audit.clone();
        let alias_trace = root.join(&aliased.targets[0].trace);
        let normal_trace = format!(
            "2 openat(AT_FDCWD, \"{trace_root}/fuzz/target/x86_64-unknown-linux-gnu/release/{}\", O_RDONLY) = 3\n2 openat(AT_FDCWD, \"{trace_root}/fuzz/corpus/{}\", O_RDONLY) = 3\n2 openat(AT_FDCWD, \"{trace_root}/benign-alias/secret\", O_RDONLY) = 3\n1 --- SIGCHLD {{si_signo=SIGCHLD, si_pid=2, si_status=0}} ---\n1 +++ exited with 0 +++\n",
            aliased.targets[0].target, aliased.targets[0].target,
        );
        fs::write(&alias_trace, &normal_trace).expect("write aliased trace");
        aliased.targets[0].trace_blake3 = hex_digest(normal_trace.as_bytes());
        aliased.targets[0].process_binding = blake3::hash(
            format!(
                "{}\0{}\0{}\0{}",
                aliased.targets[0].command,
                aliased.targets[0].trace_pid,
                aliased.targets[0].trace_exit_code,
                aliased.targets[0].trace_blake3
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        fs::write(
            &audit_path,
            serde_json::to_vec(&aliased).expect("serialize"),
        )
        .expect("write aliased audit");
        verify_corpus_access_audit(root, &packet)
            .expect_err("alias outside fuzz/corpus must not reach held-out data");

        let forbidden_path = root.join(&first.manifest);
        fs::write(
            &forbidden_path,
            "/workspace/conformance/corpora/heldout/secret\n",
        )
        .expect("write forbidden path");
        let mut doctored = audit.clone();
        doctored.targets[0].manifest_blake3 =
            blake3::hash(&fs::read(&forbidden_path).expect("read"))
                .to_hex()
                .to_string();
        fs::write(
            &audit_path,
            serde_json::to_vec(&doctored).expect("serialize"),
        )
        .expect("write doctored audit");
        verify_corpus_access_audit(root, &packet)
            .expect_err("held-out corpus access must fail closed");

        // The normalized manifest is not authoritative by itself. A trace
        // naming locked data must fail even when manifest is clean.
        let mut traced = audit;
        let first_trace = root.join(&traced.targets[0].trace);
        fs::write(
            &first_trace,
            "/workspace/conformance/corpora/heldout/secret\n",
        )
        .expect("write forbidden trace");
        traced.targets[0].trace_blake3 = blake3::hash(&fs::read(&first_trace).expect("read trace"))
            .to_hex()
            .to_string();
        traced.targets[0].process_binding = blake3::hash(
            format!(
                "{}\0{}\0{}\0{}",
                traced.targets[0].command,
                traced.targets[0].trace_pid,
                traced.targets[0].trace_exit_code,
                traced.targets[0].trace_blake3
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        fs::write(&audit_path, serde_json::to_vec(&traced).expect("serialize")).expect("write");
        verify_corpus_access_audit(root, &packet)
            .expect_err("held-out path in raw trace must fail closed");
    }

    #[test]
    fn campaign_clock_allows_one_breach_but_blocks_two() {
        let scratch = liminal_scratch::ScratchDir::new("haq-campaign-clock").expect("scratch");
        let root: &Utf8Path = &scratch;
        let path = root.join("conformance/haqp/evidence/campaign.json");
        fs::create_dir_all(path.parent().expect("clock parent")).expect("mkdir");
        let wrapper = root.join("scripts/haqp_campaign_clock.sh");
        fs::create_dir_all(wrapper.parent().expect("wrapper parent")).expect("mkdir");
        fs::write(
            &wrapper,
            fs::read(repo_root().join("scripts/haqp_campaign_clock.sh")).expect("wrapper bytes"),
        )
        .expect("write wrapper");
        let mut clock = CampaignClock {
            schema_version: "haqp-campaign-clock-v1".to_owned(),
            reference_machine: "test-host".to_owned(),
            runs: vec![CampaignRun {
                id: "run-1".to_owned(),
                commit: "a".repeat(40),
                tree: "b".repeat(40),
                command: "just ci".to_owned(),
                started_epoch: 100,
                finished_epoch: 8 * 60 * 60 + 101,
                elapsed_s: 8 * 60 * 60 + 1,
                clean: true,
                result: "pass".to_owned(),
                wrapper: "scripts/haqp_campaign_clock.sh".to_owned(),
                wrapper_sha256: format!(
                    "{:x}",
                    Sha256::digest(
                        fs::read(repo_root().join("scripts/haqp_campaign_clock.sh"))
                            .expect("wrapper"),
                    )
                ),
                receipt: String::new(),
                receipt_blake3: String::new(),
            }],
        };
        let receipt = "run_id=run-1\ncommit=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\ntree=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\ncommand=just ci\nstarted_epoch=100\nfinished_epoch=28901\nelapsed_s=28801\nclean=true\nresult=pass\n";
        let receipt_path = root.join("conformance/haqp/evidence/campaign/run-1.receipt");
        fs::create_dir_all(receipt_path.parent().expect("receipt parent")).expect("mkdir");
        fs::write(&receipt_path, receipt).expect("write receipt");
        clock.runs[0].receipt = "conformance/haqp/evidence/campaign/run-1.receipt".to_owned();
        clock.runs[0].receipt_blake3 = blake3::hash(receipt.as_bytes()).to_hex().to_string();
        fs::write(&path, serde_json::to_vec(&clock).expect("serialize")).expect("write");
        verify_campaign_clock(root, &no_reviews_packet(), None)
            .expect("one retained breach is residual risk");
        let mut blocked = clock;
        blocked.runs.push(CampaignRun {
            id: "run-2".to_owned(),
            commit: "a".repeat(40),
            tree: "b".repeat(40),
            command: "just ci".to_owned(),
            started_epoch: 200,
            finished_epoch: 8 * 60 * 60 + 201,
            elapsed_s: 8 * 60 * 60 + 1,
            clean: true,
            result: "pass".to_owned(),
            wrapper: "scripts/haqp_campaign_clock.sh".to_owned(),
            wrapper_sha256: format!(
                "{:x}",
                Sha256::digest(
                    fs::read(repo_root().join("scripts/haqp_campaign_clock.sh")).expect("wrapper"),
                )
            ),
            receipt: "conformance/haqp/evidence/campaign/run-2.receipt".to_owned(),
            receipt_blake3: String::new(),
        });
        let receipt_two = "run_id=run-2\ncommit=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\ntree=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\ncommand=just ci\nstarted_epoch=200\nfinished_epoch=29001\nelapsed_s=28801\nclean=true\nresult=pass\n";
        let receipt_two_path = root.join("conformance/haqp/evidence/campaign/run-2.receipt");
        fs::write(&receipt_two_path, receipt_two).expect("write receipt two");
        blocked.runs[1].receipt_blake3 = blake3::hash(receipt_two.as_bytes()).to_hex().to_string();
        fs::write(&path, serde_json::to_vec(&blocked).expect("serialize")).expect("write");
        verify_campaign_clock(root, &no_reviews_packet(), None)
            .expect_err("two clean breaches block ratification");
    }

    /// `count > 16` — kills `>` -> `>=`: exactly 16 is the documented maximum.
    #[test]
    fn an_operator_supplying_exactly_sixteen_mutants_is_allowed() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        let operator = packet.mutants[0].operator.clone();
        // Exactly 16 on the target operator; the rest spread round-robin over
        // the others so no OTHER operator breaches the same ceiling and masks
        // the case under test.
        //
        // The first version built its fixture with a running counter and then
        // BRANCHED on where it landed, so whichever side it hit was the only
        // side asserted — a conditional test proves whichever case it happened
        // to take, and this bound stayed unpinned either way.
        let others: Vec<String> = packet
            .mutants
            .iter()
            .map(|m| m.operator.clone())
            .filter(|op| *op != operator)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert!(
            others.len() * 16 >= packet.mutants.len() - 16,
            "the packet must declare enough operators to spread the remainder"
        );
        for (index, mutant) in packet.mutants.iter_mut().enumerate() {
            mutant.operator = if index < 16 {
                operator.clone()
            } else {
                others[(index - 16) % others.len()].clone()
            };
        }
        verify_mutant_inventory(&packet).expect("exactly 16 is at the ceiling, not above it");

        packet.mutants[16].operator = operator;
        verify_mutant_inventory(&packet).expect_err("17 must be refused");
    }

    /// `discards * 100 > attempts` — the 1% discard ceiling. Kills `>` -> `>=`,
    /// `*` -> `+`/`/`. Exactly 1% is allowed; a hair over is not.
    #[test]
    fn the_discard_ceiling_allows_exactly_one_percent() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        // `accepted` has its own >=100,000 floor, so the straddle is built
        // around it: at 100,000 accepted, 1,010 discards is just under the 1%
        // ceiling (101,000 <= 101,010) and 1,011 is just over (101,100 >
        // 101,011).
        for family in &mut packet.generated {
            family.accepted = 100_000;
            family.discards = 1_010;
            family.negatives = 0;
            family.attempts = 101_010;
        }
        verify_generated_inventory(&packet).expect("1,010 discards is at the ceiling");

        for family in &mut packet.generated {
            family.discards = 1_011;
            family.negatives = 0;
            family.attempts = 101_011;
        }
        verify_generated_inventory(&packet).expect_err("1,011 discards is above the ceiling");
    }

    #[test]
    fn generated_discard_arithmetic_fails_closed_on_u64_overflow() {
        let mut packet = read_packet(&repo_root()).expect("read packet");
        let discards = u64::MAX / 100 + 1;
        for family in &mut packet.generated {
            family.accepted = 100_000;
            family.negatives = 0;
            family.discards = discards;
            family.attempts = family.accepted + family.discards;
        }
        let error = verify_generated_inventory(&packet).expect_err("overflow must be rejected");
        assert!(error.to_string().contains("arithmetic overflow"));
    }

    /// `seed_categories.len() < 16` — kills `<` -> `>`.
    #[test]
    fn the_seed_category_floor_is_inclusive_at_16() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        for family in &mut packet.generated {
            if generated_seed_categories(family.family.as_str()).len() == 16 {
                family.seed_categories.truncate(16);
            }
        }
        verify_generated_inventory(&packet).expect("exactly 16 seed categories is allowed");

        for family in &mut packet.generated {
            family.seed_categories.truncate(15);
        }
        verify_generated_inventory(&packet).expect_err("15 seed categories is below the floor");
    }

    /// `!row.before || !row.after` — kills `||` -> `&&`, which would accept a
    /// boundary injected on only one side of the crash point.
    #[test]
    fn a_crash_boundary_injected_on_only_one_side_is_refused() {
        for (before, after) in [(true, false), (false, true)] {
            let mut packet = read_packet(&repo_root()).expect("packet");
            packet.crash_boundaries[0].before = before;
            packet.crash_boundaries[0].after = after;
            verify_crash_boundary_inventory(&packet)
                .expect_err("a boundary must be injected on BOTH sides");
        }
    }

    /// M17.5 A05 accept case. Every reject case below is worthless without it:
    /// a scope check that refuses everything would pass them all.
    #[test]
    fn the_committed_packet_surveys_every_durable_surface_in_tracked_source() {
        let packet = read_packet(&repo_root()).expect("packet");
        verify_crash_boundary_scope(&repo_root(), &packet)
            .expect("the committed disclosure must account for every durable transition");
    }

    /// The defect A05 found: §5 claims exhaustiveness over a surface nobody
    /// surveyed. A durable transition in no declared surface must be refused,
    /// which is what makes a NEW `txn.commit()` turn the packet red.
    #[test]
    fn a_durable_surface_missing_from_the_disclosure_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        let dropped = packet.crash_boundary_scope.deferred.remove(0);
        let error = verify_crash_boundary_scope(&repo_root(), &packet)
            .expect_err("an unsurveyed durable surface must be refused");
        let message = error.to_string();
        assert!(
            message.contains(&dropped.path) && message.contains("neither the in-scope"),
            "the error must name the unsurveyed surface, got: {message}"
        );
    }

    /// A prose disclosure rots silently; this one must not. Drift is checked in
    /// BOTH directions so `==` cannot weaken to `>=` or `<=` and survive.
    #[test]
    fn a_stale_site_count_is_refused_in_either_direction() {
        for delta in [1usize, 2] {
            let mut packet = read_packet(&repo_root()).expect("packet");
            packet.crash_boundary_scope.deferred[0].sites += delta;
            verify_crash_boundary_scope(&repo_root(), &packet)
                .expect_err("a count above the measured surface is stale");

            let mut packet = read_packet(&repo_root()).expect("packet");
            packet.crash_boundary_scope.deferred[0].sites -= delta;
            verify_crash_boundary_scope(&repo_root(), &packet)
                .expect_err("a count below the measured surface is stale");
        }
    }

    /// The in-scope side is measured too. Scoping §5 to ILRP is only honest if
    /// ILRP's own declared surface is checked against source like the rest.
    #[test]
    fn a_stale_in_scope_site_count_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.crash_boundary_scope.in_scope[0].sites += 1;
        verify_crash_boundary_scope(&repo_root(), &packet)
            .expect_err("the in-scope surface is pinned to source, not asserted");
    }

    /// A declaration for a surface that no longer exists is the same staleness
    /// pointing the other way, and would otherwise let a deleted file's count
    /// keep vouching for the disclosure.
    #[test]
    fn a_disclosure_for_a_vanished_surface_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet
            .crash_boundary_scope
            .deferred
            .push(DeferredDurableSurface {
                path: "crates/liminal-daemon/src/no_such_file.rs".to_owned(),
                sites: 3,
                reason: "fabricated".to_owned(),
            });
        verify_crash_boundary_scope(&repo_root(), &packet)
            .expect_err("a disclosure naming source that does not exist must be refused");
    }

    /// Registering `ilrp/*` while claiming scope over something else would make
    /// the declaration decorative.
    #[test]
    fn a_registered_boundary_outside_the_declared_scope_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.crash_boundary_scope.in_scope[0].subsystem = "daemon".to_owned();
        let error = verify_crash_boundary_scope(&repo_root(), &packet)
            .expect_err("boundaries must sit inside a declared subsystem");
        assert!(
            error.to_string().contains("ilrp/"),
            "the error must name the out-of-scope boundary, got: {error}"
        );
    }

    /// A blank subsystem matches no boundary while still reading as a claim.
    /// Raised by the HEAD review as an asymmetry: `deferred` entries were
    /// checked for a blank reason, `in_scope` entries for nothing at all.
    #[test]
    fn a_blank_in_scope_subsystem_or_path_is_refused() {
        for blank in ["", "   "] {
            let mut packet = read_packet(&repo_root()).expect("packet");
            packet.crash_boundary_scope.in_scope[0].subsystem = blank.to_owned();
            verify_crash_boundary_scope(&repo_root(), &packet)
                .expect_err("a blank subsystem names no boundary prefix");

            let mut packet = read_packet(&repo_root()).expect("packet");
            packet.crash_boundary_scope.in_scope[0].path = blank.to_owned();
            verify_crash_boundary_scope(&repo_root(), &packet)
                .expect_err("a blank path names no surface");
        }
    }

    /// An empty scope claims nothing while still reading as a disclosure.
    #[test]
    fn an_empty_scope_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.crash_boundary_scope.in_scope.clear();
        verify_crash_boundary_scope(&repo_root(), &packet)
            .expect_err("a packet claiming no scope at all must be refused");
    }

    /// Declaring one surface twice would let a real count and a convenient one
    /// coexist, with whichever landed last deciding the verdict.
    #[test]
    fn a_surface_declared_both_in_scope_and_deferred_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        let path = packet.crash_boundary_scope.in_scope[0].path.clone();
        packet
            .crash_boundary_scope
            .deferred
            .push(DeferredDurableSurface {
                path,
                sites: 3,
                reason: "duplicate".to_owned(),
            });
        verify_crash_boundary_scope(&repo_root(), &packet)
            .expect_err("a surface cannot be both in scope and deferred");
    }

    /// The deferral has to name an owner and a reason, or it is an excuse
    /// rather than a disclosure.
    #[test]
    fn a_deferral_without_owner_or_reason_is_refused() {
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.crash_boundary_scope.deferred_to = "  ".to_owned();
        verify_crash_boundary_scope(&repo_root(), &packet)
            .expect_err("a deferral must name who closes it");

        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.crash_boundary_scope.rationale = String::new();
        verify_crash_boundary_scope(&repo_root(), &packet)
            .expect_err("a bounded claim must say why it is bounded");

        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.crash_boundary_scope.deferred[0].reason = "   ".to_owned();
        verify_crash_boundary_scope(&repo_root(), &packet)
            .expect_err("each deferred surface must say why it is deferred");
    }

    /// The scan reads `git ls-files`, not the working directory (F-32). An
    /// untracked file with durable transitions must not enter the surface — on
    /// a dirty tree it would, and a fresh clone could then never reproduce the
    /// verdict.
    #[test]
    fn the_durable_surface_is_read_from_tracked_state_not_the_filesystem() {
        let root = repo_root();
        let measured = durable_transition_sites(&root).expect("scan");
        let tracked = git_text(&root, &["ls-files", "-z", "--", "crates"]).expect("ls-files");
        let tracked: BTreeSet<&str> = tracked
            .split('\0')
            .filter(|name| !name.trim().is_empty())
            .collect();
        for path in measured.keys() {
            assert!(
                tracked.contains(path.as_str()),
                "{path} entered the durable surface without being tracked"
            );
        }
        assert!(
            !measured.is_empty(),
            "a scan that found nothing would make every disclosure vacuous"
        );
    }

    // ── M17.5 F-36: gates no test proves do anything ──────────────────────
    //
    // The completed verifier mutation campaign found 27 functions in this file
    // that survive being replaced wholesale with `Ok(())`. 26 had no reject
    // assertion anywhere and 17 were never called by a test at all. The cause
    // is structural, not 27 separate oversights: `verify_qualified_repo`
    // checks `qualification_state == "complete"` first, and the committed
    // packet is `not-run`, so every downstream verifier is unreachable through
    // the gate. They are reached here directly.

    /// `require_hex_digest` is the length/alphabet guard under every digest
    /// binding in the packet. Replacing it with `Ok(())` survived, so nothing
    /// proved a digest had to be a digest.
    #[test]
    fn a_digest_that_is_not_64_hex_is_refused() {
        require_hex_digest("label", &"a".repeat(64)).expect("64 hex digits is a digest");
        for bad in [
            "a".repeat(63),
            "a".repeat(65),
            String::new(),
            format!("{}g", "a".repeat(63)),
            format!("{} ", "a".repeat(63)),
        ] {
            assert!(
                require_hex_digest("label", &bad).is_err(),
                "{bad:?} is not a 64-hex digest"
            );
        }
    }

    /// `require_git_object_id` admits exactly SHA-1 and SHA-256 widths. The
    /// `matches!(len, 40 | 64)` arm is what stops a truncated id passing, and
    /// nothing exercised it.
    #[test]
    fn a_git_object_id_must_be_40_or_64_hex() {
        require_git_object_id("label", &"0".repeat(40)).expect("SHA-1 width");
        require_git_object_id("label", &"0".repeat(64)).expect("SHA-256 width");
        for bad in [
            "0".repeat(39),
            "0".repeat(41),
            "0".repeat(63),
            "0".repeat(65),
            format!("{}z", "0".repeat(39)),
        ] {
            assert!(
                require_git_object_id("label", &bad).is_err(),
                "{bad:?} is not a Git object id"
            );
        }
    }

    /// ADR-0020 §2 traceability rests on requirement coordinates resolving to
    /// committed source. This runs in BOTH gates and still survived deletion:
    /// the doctored-tree test only ever doctored `ratification`, so the path
    /// safety and anchor checks were never exercised negatively.
    #[test]
    fn a_requirement_source_that_does_not_resolve_is_refused() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        verify_requirement_sources(&root, &packet).expect("the committed packet must resolve");

        for (bad, why) in [
            ("docs/execution/M17.md", "source without a coordinate"),
            ("/etc/passwd:D1.1", "absolute path"),
            ("docs/../../etc/passwd:D1.1", "parent traversal"),
            ("conformance/corpora/heldout/x.md:D1.1", "locked corpus"),
            ("docs/execution/NoSuchFile.md:D1.1", "missing file"),
            (
                "docs/execution/M17.md:D99.999",
                "coordinate absent from the file",
            ),
        ] {
            let mut doctored = packet.clone();
            doctored.requirements[0].source = bad.to_owned();
            assert!(
                verify_requirement_sources(&root, &doctored).is_err(),
                "{why} must be refused, but {bad:?} was accepted"
            );
        }
    }

    /// §5 oracle independence names an exact coordinate. Deleting the whole
    /// check survived, so neither the registry comparison nor the anchor
    /// lookup was pinned.
    #[test]
    fn an_oracle_source_off_its_registered_coordinate_is_refused() {
        let root = repo_root();
        let family = "source/CST/formatting";
        let registered = generated_oracle_source(family);
        assert!(
            !registered.is_empty(),
            "the fixture family must be a known oracle family"
        );
        verify_oracle_source_coordinate(&root, registered, family)
            .expect("the registered coordinate must verify");

        verify_oracle_source_coordinate(&root, registered, "no/such/family")
            .expect_err("an unknown oracle family must be refused");

        let (file, _) = registered.split_once(':').expect("registered coordinate");
        for bad in [
            file.to_owned(),
            format!("{file}:no_such_anchor"),
            format!("{file}:"),
        ] {
            assert!(
                verify_oracle_source_coordinate(&root, &bad, family).is_err(),
                "{bad:?} is not the registered oracle coordinate"
            );
        }
    }

    /// §6 requires Pass 1 to cover every declared attack class. Deleting the
    /// whole check survived: nothing proved an unknown class was refused, and
    /// nothing proved Pass 1 had to be complete.
    #[test]
    fn a_review_pass_with_a_bad_or_missing_attack_class_is_refused() {
        let record = review_record(1, "openai", "codex");
        verify_review_attack_classes(&record, "pass1")
            .expect("the fixture covers every attack class");

        let mut unknown = review_record(1, "openai", "codex");
        unknown.attempts[0].attack_class = "creative-thinking".to_owned();
        let error = verify_review_attack_classes(&unknown, "pass1")
            .expect_err("an attack class outside the closed registry must be refused");
        assert!(
            error.to_string().contains("unknown attack class"),
            "must refuse for the stated reason: {error}"
        );

        // Pass 1 must be exhaustive; dropping one class leaves a gap.
        let mut incomplete = review_record(1, "openai", "codex");
        let dropped = incomplete.attempts[0].attack_class.clone();
        incomplete
            .attempts
            .retain(|attempt| attempt.attack_class != dropped);
        assert!(
            !incomplete.attempts.is_empty(),
            "dropping one class must leave a record that is incomplete, not empty — \
             an empty record would fail for a different reason than the gap"
        );
        let error = verify_review_attack_classes(&incomplete, "pass1")
            .expect_err("Pass 1 must cover every attack class");
        assert!(
            error.to_string().contains("missing attack classes"),
            "must name the gap: {error}"
        );

        // Pass 2 carries no exhaustiveness duty, so the same record is fine.
        let mut pass_two = incomplete;
        pass_two.pass = 2;
        verify_review_attack_classes(&pass_two, "pass2")
            .expect("only Pass 1 must cover every class");
    }

    /// §6's independence claim: a finding marked reproduced must correspond to
    /// EXACTLY ONE matching verified defect in the other pass. Deleting the
    /// check survived, so "independently reproduced" was self-asserted.
    #[test]
    fn a_reproduced_finding_without_a_match_in_the_other_pass_is_refused() {
        fn paired(link: bool) -> Vec<(String, ReviewRecord)> {
            let mut one = review_record(1, "openai", "codex");
            let mut two = review_record(2, "mimo", "lamu");
            for record in [&mut one, &mut two] {
                record.attempts[0].classification = "verified_defect".to_owned();
                record.attempts[0].independently_reproduced = true;
                record.findings = vec![serde_json::json!({
                    "id": "F-01",
                    "attempt_id": record.attempts[0].id,
                })];
                record.unresolved_verified_findings = 1;
                record.result = "fail".to_owned();
            }
            if link {
                one.independently_reproduced = vec!["F-01".to_owned()];
            }
            vec![("pass1".to_owned(), one), ("pass2".to_owned(), two)]
        }

        verify_cross_pass_reproduction(&paired(true))
            .expect("a finding matched one-for-one across passes is reproduced");

        // The other pass no longer carries the matching defect.
        let mut orphaned = paired(true);
        assert!(
            orphaned[1].1.attempts.len() >= 2,
            "the fixture must supply at least two attempts, or the orphaning below \
             indexes out of bounds instead of asserting"
        );
        orphaned[1].1.attempts[0].attack_class = "nondeterminism".to_owned();
        orphaned[1].1.attempts[1].attack_class = "vacuity".to_owned();
        let error = verify_cross_pass_reproduction(&orphaned)
            .expect_err("a reproduced finding needs a match in the other pass");
        assert!(
            error.to_string().contains("exactly one"),
            "must refuse for the stated reason: {error}"
        );

        // One record alone cannot establish cross-pass reproduction.
        let single = vec![paired(true).remove(0)];
        verify_cross_pass_reproduction(&single)
            .expect_err("cross-pass reproduction requires two records");
    }

    /// §5 binds each packet before/after boolean to the measured injection
    /// receipt. Deleting the whole binding survived, so the packet's claim that
    /// a boundary was injected on both sides rested on the packet saying so.
    #[test]
    fn a_crash_claim_the_receipt_contradicts_is_refused() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        let bytes =
            fs::read(root.join("conformance/haqp/evidence/crash.json")).expect("crash evidence");
        let recorded: CrashEvidence = serde_json::from_slice(&bytes).expect("parse crash evidence");
        verify_crash_injection_bindings(&packet, &recorded)
            .expect("the committed packet must match its own receipt");

        // Claim an injection the receipt does not record.
        let mut thinned = recorded.clone();
        thinned
            .boundaries
            .iter_mut()
            .for_each(|row| row.injected = false);
        let error = verify_crash_injection_bindings(&packet, &thinned)
            .expect_err("a claimed injection with no measured receipt must be refused");
        assert!(
            error.to_string().contains("injection claim differs"),
            "must refuse for the stated reason: {error}"
        );

        // An unpaired boundary name has no closed before/after registry entry.
        let mut invented = packet.clone();
        invented.crash_boundaries[0].boundary = "ilrp/invented".to_owned();
        let error = verify_crash_injection_bindings(&invented, &recorded)
            .expect_err("a boundary outside the closed pair registry must be refused");
        assert!(
            error.to_string().contains("closed before/after pair"),
            "must refuse for the stated reason: {error}"
        );
    }

    /// §4's fuzz rows are bound to the retained log's own footers. Deleting the
    /// check survived, so `execs`, `elapsed_s`, `seconds` and `seed` were four
    /// numbers the packet asserted about itself.
    #[test]
    fn a_fuzz_row_that_its_own_log_contradicts_is_refused() {
        let mut row = fuzz_row("cst_parse", 1800);
        row.elapsed_s = 1802;
        row.execs = 4096;
        row.seed = 7;
        let log = |execs: u64, elapsed: u64, budget: u64, seed: u64| {
            format!(
                "Running with -max_total_time={budget} -seed={seed}\n\
                 stat::number_of_executed_units: {execs}\n\
                 Done {execs} runs in {elapsed} second(s)\n"
            )
        };

        verify_fuzz_log_metrics(&row, log(4096, 1800, 1800, 7).as_bytes())
            .expect("a log agreeing with its row must verify");

        for (bytes, why) in [
            (
                log(4095, 1800, 1800, 7),
                "execution count differs from the row",
            ),
            (log(4096, 1600, 1800, 7), "elapsed is not bound to the row"),
            (log(4096, 1800, 900, 7), "budget is not the recorded one"),
            (log(4096, 1800, 1800, 8), "seed is not the recorded one"),
            (
                "stat::number_of_executed_units: 4096\n".to_owned(),
                "no Done footer at all",
            ),
            (
                "Done 4096 runs in 1800 second(s)\n".to_owned(),
                "no execution-count footer at all",
            ),
        ] {
            assert!(
                verify_fuzz_log_metrics(&row, bytes.as_bytes()).is_err(),
                "a log whose {why} must be refused"
            );
        }
    }

    /// A throwaway Git repository, so the ancestry and diff checks can be
    /// exercised against a history this test owns.
    ///
    /// The alternative — asserting against the real repo's HEAD — pins tests
    /// to whatever was committed last, which is how a check ends up passing
    /// for a reason nobody chose.
    fn scratch_git_repo(label: &str) -> (liminal_scratch::ScratchDir, Utf8PathBuf) {
        let scratch = liminal_scratch::ScratchDir::new(label).expect("scratch dir");
        let root = scratch.path().to_owned();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .current_dir(&root)
                .args(args)
                .status()
                .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["init", "--quiet"]);
        run(&["config", "user.email", "test@example.invalid"]);
        run(&["config", "user.name", "haqp test"]);
        run(&["config", "commit.gpgsign", "false"]);
        (scratch, root)
    }

    fn scratch_commit(root: &Utf8Path, file: &str, body: &str, message: &str) -> String {
        fs::write(root.join(file), body).expect("write fixture file");
        for args in [vec!["add", file], vec!["commit", "--quiet", "-m", message]] {
            let status = Command::new("git")
                .current_dir(root)
                .args(&args)
                .status()
                .expect("git");
            assert!(status.success(), "git {args:?} failed");
        }
        git_text(root, &["rev-parse", "HEAD"]).expect("rev-parse")
    }

    /// `git_is_ancestor` guards provenance: the fixed base must actually
    /// precede the metadata child. Deleting it survived, so ancestry was
    /// asserted and never checked.
    #[test]
    fn a_commit_that_is_not_an_ancestor_is_refused() {
        let (_scratch, root) = scratch_git_repo("haq-ancestry");
        let first = scratch_commit(&root, "a.txt", "one\n", "first");
        let second = scratch_commit(&root, "a.txt", "two\n", "second");

        git_is_ancestor(&root, &first, &second).expect("the parent precedes its child");
        git_is_ancestor(&root, &second, &first)
            .expect_err("a child does not precede its own parent");
        git_is_ancestor(&root, &"0".repeat(40), &second)
            .expect_err("an unknown commit is not an ancestor of anything");
    }

    /// §6 requires a resolution to have actually touched the coordinate it
    /// claims to fix. Deleting the check survived, so "resolved at file:line"
    /// was prose.
    #[test]
    fn a_resolution_that_missed_its_coordinate_is_refused() {
        let (_scratch, root) = scratch_git_repo("haq-resolution");
        let base = scratch_commit(&root, "src.rs", "one\ntwo\nthree\n", "base");
        scratch_commit(&root, "other.rs", "untouched\n", "unrelated");
        let fixed = git_text(&root, &["rev-parse", "HEAD"]).expect("rev-parse");
        let resolution = scratch_commit(&root, "src.rs", "one\nCHANGED\nthree\n", "fix line 2");

        verify_resolution_coordinate_changed(&root, &fixed, &resolution, "pass1", "src.rs", 2)
            .expect("the resolution changed line 2 of the file it names");

        verify_resolution_coordinate_changed(&root, &fixed, &resolution, "pass1", "other.rs", 1)
            .expect_err("a file the resolution never touched must be refused");

        verify_resolution_coordinate_changed(&root, &fixed, &resolution, "pass1", "src.rs", 3)
            .expect_err("a line outside the changed hunk must be refused");

        // The base..base range changes nothing at all.
        verify_resolution_coordinate_changed(&root, &base, &base, "pass1", "src.rs", 2)
            .expect_err("an empty range cannot evidence a resolution");
    }

    /// AM-17.6's whole guarantee. The metadata child may lift exactly one
    /// `#[ignore]` and change nothing else in the gate file; this is the check
    /// that enforces it, and deleting it entirely survived — so the one commit
    /// permitted to touch gate code was policed by nothing.
    ///
    /// Exercised against a scratch history rather than the real one: the real
    /// HEAD has no metadata child, so every assertion here would be about a
    /// diff that does not exist.
    #[test]
    fn a_metadata_child_that_does_more_than_lift_one_ignore_is_refused() {
        let (_scratch, root) = scratch_git_repo("haq-gate-unignore");
        let gate = PHASE0_GATE_FILE;
        fs::create_dir_all(root.join(gate).parent().expect("gate parent")).expect("mkdir");

        let with_ignore = format!("{PHASE0_GATE_IGNORE}\nfn gate() {{}}\n");
        let parent = scratch_commit(&root, gate, &with_ignore, "base");

        // The permitted child: exactly the one attribute line removed.
        scratch_commit(&root, gate, "fn gate() {}\n", "lift the ignore");
        verify_gate_unignore_only(&root, &parent)
            .expect("lifting exactly one #[ignore] is allowed");

        // Adding anything at all, even alongside a legitimate lift.
        let (_s2, root2) = scratch_git_repo("haq-gate-added");
        fs::create_dir_all(root2.join(gate).parent().expect("gate parent")).expect("mkdir");
        let parent2 = scratch_commit(&root2, gate, &with_ignore, "base");
        scratch_commit(
            &root2,
            gate,
            "fn gate() {}\nfn snuck_in() {}\n",
            "lift and add",
        );
        let error = verify_gate_unignore_only(&root2, &parent2)
            .expect_err("the child may never add gate code");
        assert!(
            error.to_string().contains("added"),
            "must refuse for the stated reason: {error}"
        );

        // Removing more than one line.
        let (_s3, root3) = scratch_git_repo("haq-gate-multi");
        fs::create_dir_all(root3.join(gate).parent().expect("gate parent")).expect("mkdir");
        let parent3 = scratch_commit(
            &root3,
            gate,
            &format!("{PHASE0_GATE_IGNORE}\nfn gate() {{}}\nfn extra() {{}}\n"),
            "base",
        );
        scratch_commit(&root3, gate, "fn gate() {}\n", "lift and delete");
        verify_gate_unignore_only(&root3, &parent3)
            .expect_err("exactly one line may be removed, not two");

        // Removing one line that is NOT the qualification gate's attribute.
        let (_s4, root4) = scratch_git_repo("haq-gate-wrong-line");
        fs::create_dir_all(root4.join(gate).parent().expect("gate parent")).expect("mkdir");
        let parent4 = scratch_commit(
            &root4,
            gate,
            &format!("{PHASE0_GATE_IGNORE}\nfn gate() {{}}\nfn extra() {{}}\n"),
            "base",
        );
        scratch_commit(
            &root4,
            gate,
            &format!("{PHASE0_GATE_IGNORE}\nfn gate() {{}}\n"),
            "delete something else",
        );
        let error = verify_gate_unignore_only(&root4, &parent4)
            .expect_err("only the qualification gate's own attribute may be lifted");
        assert!(
            error.to_string().contains("lifted attribute"),
            "must refuse for the stated reason: {error}"
        );
    }

    /// A second name for the locked corpus lets a traced path read it without
    /// naming it, which is how corpus-access evidence goes quietly false.
    /// Deleting the check survived: the existing test only proved the real tree
    /// has no alias, which a function returning `Ok(())` also proves.
    ///
    /// Built entirely inside a scratch tree. The real
    /// `conformance/corpora/heldout` is never read, listed, or resolved here —
    /// only a fixture directory standing in the same relative position.
    #[test]
    fn a_second_name_for_the_locked_corpus_is_refused() {
        let scratch = liminal_scratch::ScratchDir::new("haq-corpus-alias").expect("scratch dir");
        let root = scratch.path().to_owned();
        let locked = root.join("conformance/corpora/heldout");
        fs::create_dir_all(&locked).expect("fixture corpus");
        fs::write(locked.join("case.md"), "fixture, not the real corpus\n").expect("fixture case");

        verify_locked_corpus_has_no_aliases(&root)
            .expect("a corpus with exactly one name is not aliased");

        // A symlink pointing INTO the locked tree gives its bytes a second name.
        let alias = root.join("shortcut");
        std::os::unix::fs::symlink(locked.as_std_path(), alias.as_std_path())
            .expect("create alias");
        let error = verify_locked_corpus_has_no_aliases(&root)
            .expect_err("a symlink into the locked corpus must be refused");
        assert!(
            error.to_string().contains("alias"),
            "must refuse for the stated reason: {error}"
        );
        fs::remove_file(alias.as_std_path()).expect("remove alias");

        // A link to a file INSIDE the corpus is the same leak, one level down.
        let deep = root.join("deep-link.md");
        std::os::unix::fs::symlink(locked.join("case.md").as_std_path(), deep.as_std_path())
            .expect("create deep alias");
        verify_locked_corpus_has_no_aliases(&root)
            .expect_err("a symlink to a file inside the locked corpus must be refused");
        fs::remove_file(deep.as_std_path()).expect("remove deep alias");

        // A symlink that resolves somewhere else entirely is not an alias.
        let elsewhere = root.join("unrelated");
        fs::create_dir_all(&elsewhere).expect("unrelated dir");
        std::os::unix::fs::symlink(
            elsewhere.as_std_path(),
            root.join("innocent-link").as_std_path(),
        )
        .expect("create unrelated link");
        verify_locked_corpus_has_no_aliases(&root)
            .expect("a link outside the locked tree is not an alias");

        // `.git` and `target` are skipped deliberately: Git's object store and
        // the build directory both hold links that are not corpus aliases, and
        // walking them turns a scan into a crawl. The skip is pinned here
        // because without a fixture that HAS those directories, breaking it
        // changes nothing observable — the campaign found exactly that gap.
        //
        // This depends on the walk skipping by NAME before it resolves the
        // entry: `verify_locked_corpus_has_no_aliases` hits its `continue`
        // ahead of `symlink_metadata`/`canonicalize`. If that order ever
        // inverts, these links resolve into the corpus and the assertion below
        // flips meaning rather than failing loudly.
        for skipped in [".git", "target"] {
            let dir = root.join(skipped);
            fs::create_dir_all(&dir).expect("skipped dir");
            std::os::unix::fs::symlink(
                locked.as_std_path(),
                dir.join("link-into-corpus").as_std_path(),
            )
            .expect("create link inside skipped dir");
        }
        verify_locked_corpus_has_no_aliases(&root)
            .expect("links inside .git and target are not corpus aliases");
    }

    /// §4's fuzz rows are only evidence if the retained log is the bytes the
    /// campaign produced. Deleting the check survived, so `log_blake3` bound
    /// nothing and a row could name a log that was never written.
    #[test]
    fn a_fuzz_row_whose_retained_log_is_missing_or_altered_is_refused() {
        let scratch = liminal_scratch::ScratchDir::new("haq-fuzz-logs").expect("scratch dir");
        let root = scratch.path().to_owned();
        let logs = root.join("conformance/haqp/evidence/logs");
        fs::create_dir_all(&logs).expect("log dir");

        let mut row = fuzz_row("cst_parse", 1800);
        row.elapsed_s = 1800;
        row.execs = 4096;
        row.seed = 7;
        row.log = "conformance/haqp/evidence/logs/cst_parse.log".to_owned();
        let body = "Running with -max_total_time=1800 -seed=7\n\
                    stat::number_of_executed_units: 4096\n\
                    Done 4096 runs in 1800 second(s)\n";
        fs::write(logs.join("cst_parse.log"), body).expect("write log");
        row.log_blake3 = blake3::hash(body.as_bytes()).to_hex().to_string();

        verify_fuzz_logs(&root, std::slice::from_ref(&row))
            .expect("a row bound to the bytes on disk must verify");

        let mut wrong_path = row.clone();
        wrong_path.log = "target/haqp/fuzz-cst_parse.log".to_owned();
        verify_fuzz_logs(&root, std::slice::from_ref(&wrong_path))
            .expect_err("the log must live at the committed evidence path");

        let mut wrong_digest = row.clone();
        wrong_digest.log_blake3 = "c".repeat(64);
        verify_fuzz_logs(&root, std::slice::from_ref(&wrong_digest))
            .expect_err("a digest that is not the file's must be refused");

        // The bytes changed after the digest was recorded.
        fs::write(logs.join("cst_parse.log"), format!("{body}tampered\n")).expect("rewrite log");
        verify_fuzz_logs(&root, std::slice::from_ref(&row))
            .expect_err("an altered log must no longer match its recorded digest");

        // The log is absent entirely.
        fs::remove_file(logs.join("cst_parse.log")).expect("remove log");
        verify_fuzz_logs(&root, std::slice::from_ref(&row))
            .expect_err("a row naming a log that does not exist must be refused");
    }

    /// §3 requires equivalent/duplicate dispositions to carry concurring
    /// verification in both adversarial passes. The check returns early when no
    /// mutant claims one — which the committed packet does not — so `Ok(())`
    /// was indistinguishable on the real packet and the whole rule was
    /// untested. Reached here by giving a mutant the disposition that triggers
    /// it.
    #[test]
    fn an_equivalent_disposition_without_concurring_reviews_is_refused() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        verify_mutant_concurrence(&root, &packet)
            .expect("a packet claiming no equivalences has nothing to concur on");

        for disposition in ["equivalent", "duplicate"] {
            let mut claimed = packet.clone();
            claimed.mutants[0].disposition = disposition.to_owned();
            let Err(error) = verify_mutant_concurrence(&root, &claimed) else {
                panic!("a {disposition} disposition over planned reviews must be refused")
            };
            // Asserting only `is_err()` here let a mutant survive: the packet's
            // reviews are `planned`, so inverting the `== "pass"` comparison
            // still failed, just further down and for an unrelated reason. A
            // reject test that does not name its reason accepts any refusal.
            assert!(
                error
                    .to_string()
                    .contains("requires passing committed review record"),
                "must refuse because concurrence needs passing reviews, got: {error}"
            );
        }
    }

    /// F-32's rule, enforced: a fuzz row's seed claim must match the TRACKED
    /// corpus, not a number the row carries. Deleting the check survived, so
    /// `seed_count` and `seed_manifest_blake3` were self-asserted.
    #[test]
    fn a_seed_claim_the_committed_corpus_contradicts_is_refused() {
        let root = repo_root();
        let target = "cst_parse";
        let (count, digest) = seed_manifest_digest(&root, &root.join("fuzz/corpus").join(target))
            .expect("the committed corpus must digest");

        let mut row = fuzz_row(target, 1800);
        row.seed_count = count;
        row.seed_manifest_blake3 = digest.clone();
        verify_fuzz_seed_manifests(&root, std::slice::from_ref(&row))
            .expect("a row matching the tracked corpus must verify");

        let mut miscounted = row.clone();
        miscounted.seed_count = count + 1;
        let error = verify_fuzz_seed_manifests(&root, std::slice::from_ref(&miscounted))
            .expect_err("a seed count the corpus contradicts must be refused");
        assert!(
            error.to_string().contains("committed corpus has"),
            "must refuse for the stated reason: {error}"
        );

        let mut wrong_digest = row.clone();
        wrong_digest.seed_manifest_blake3 = "d".repeat(64);
        verify_fuzz_seed_manifests(&root, std::slice::from_ref(&wrong_digest))
            .expect_err("a manifest digest that is not the corpus's must be refused");

        let mut unknown_target = row;
        unknown_target.target = "no_such_target".to_owned();
        verify_fuzz_seed_manifests(&root, std::slice::from_ref(&unknown_target))
            .expect_err("a target with no tracked seed corpus must be refused");
    }

    /// The off-by-one that has now shipped three times: `git_text` trims, so
    /// the FIRST porcelain line arrives without its leading space and a fixed
    /// `line[3..]` slice eats a character of the path.
    #[test]
    fn a_trimmed_first_porcelain_line_keeps_its_whole_path() {
        // As `git status --porcelain=v1` emits it, and as it arrives after trim.
        assert_eq!(
            porcelain_path(" M conformance/haqp/packet.json").as_deref(),
            Some("conformance/haqp/packet.json")
        );
        assert_eq!(
            porcelain_path("M conformance/haqp/packet.json").as_deref(),
            Some("conformance/haqp/packet.json"),
            "a trimmed leading space must not cost the path its first character"
        );
        for (line, want) in [
            ("?? scripts/new.sh", "scripts/new.sh"),
            (
                "MM crates/liminal-xtask/src/haq.rs",
                "crates/liminal-xtask/src/haq.rs",
            ),
            ("R  docs/old.md -> docs/new.md", "docs/new.md"),
            ("A  \"docs/with space.md\"", "docs/with space.md"),
        ] {
            assert_eq!(
                porcelain_path(line).as_deref(),
                Some(want),
                "line: {line:?}"
            );
        }
    }

    /// Evidence dirt is what the lanes PRODUCE; refusing it would refuse every
    /// campaign at its own second stage. This is the same rule as
    /// scripts/haqp_paths.py and the two must not drift.
    #[test]
    fn source_dirt_ignores_metadata_and_evidence_but_not_code() {
        for path in [
            "conformance/haqp/evidence/canaries.json",
            "conformance/haqp/evidence/access/cst_parse.trace",
            "conformance/haqp/packet.json",
            "docs/execution/phase1-suite-review.md",
        ] {
            assert!(
                qualification_metadata_path(path),
                "{path} is written by a lane and must not count as source dirt"
            );
        }
        for path in [
            "crates/liminal-xtask/src/haq.rs",
            "scripts/haqp_qualify.sh",
            "justfile",
            // phase0.rs is metadata to haqp_paths.py, which runs after the
            // flip lifts the gate's #[ignore]. Mid-campaign it is source.
            "conformance/tests/phase0.rs",
        ] {
            assert!(
                !qualification_metadata_path(path),
                "{path} is source mid-campaign; dirtying it must invalidate a run"
            );
        }
    }

    /// The check that refused the 2026-09-03 lane at the flip.
    ///
    /// `verify_mutant_source_coordinates` runs only inside the qualified gate,
    /// so F-38 could rewrite every packet coordinate, pass `haq-inventory`,
    /// pass two blind reviews, and only be caught after a full campaign. The
    /// packet and the closed registry are a TWO-SIDED binding; F-38 moved one
    /// side. Exercised here so the next break costs seconds.
    #[test]
    fn packet_mutant_coordinates_match_the_closed_registry() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        verify_mutant_source_coordinates(&root, &packet)
            .expect("committed packet must agree with the closed source registry");

        // Move one coordinate off the registry.
        let mut drifted = packet.clone();
        let (file, line) = drifted.mutants[1]
            .source
            .rsplit_once(':')
            .expect("coordinate");
        let bumped: usize = line.parse::<usize>().expect("line") + 40;
        drifted.mutants[1].source = format!("{file}:{bumped}");
        let error = verify_mutant_source_coordinates(&root, &drifted)
            .expect_err("a coordinate off the closed registry must be refused");
        assert!(
            error.to_string().contains("does not match closed registry"),
            "must refuse for the stated reason: {error}"
        );

        // Two mutants may not share one coordinate.
        let mut reused = packet.clone();
        reused.mutants[1].source = reused.mutants[0].source.clone();
        verify_mutant_source_coordinates(&root, &reused)
            .expect_err("a reused source coordinate must be refused");
    }

    /// Builds a scratch root holding everything verify_corpus_scope_rows reads:
    /// the tracer receipt, a compressed scope trace whose one open resolves
    /// inside the root, and a row whose digests were computed by the same code
    /// the gate uses. Every reject case below doctors ONE thing off this.
    fn scope_fixture(scope: &str) -> (liminal_scratch::ScratchDir, Utf8PathBuf, CorpusScopeTrace) {
        let scratch = liminal_scratch::ScratchDir::new("haq-scope").expect("scratch");
        let root = fs::canonicalize(scratch.path().as_std_path())
            .map(|p| Utf8PathBuf::from_path_buf(p).expect("utf8"))
            .expect("canonical root");
        let access = root.join("conformance/haqp/evidence/access");
        fs::create_dir_all(access.join("scopes")).expect("mkdir");
        fs::write(access.join("strace.version"), "strace -- version 6.0\n").expect("receipt");
        fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("opened file");
        let pid = 4242u32;
        let trace = format!(
            "{pid} openat(AT_FDCWD, \"{root}/Cargo.toml\", O_RDONLY) = 3\n{pid} +++ exited with 0 +++\n"
        );
        let stored = access.join(format!("scopes/{scope}.trace.zst"));
        fs::write(
            &stored,
            zstd::encode_all(trace.as_bytes(), 3).expect("zstd"),
        )
        .expect("trace");
        let command = format!(
            "strace -f -q -e trace=%file -o target/haqp/scope-{scope}.trace {}",
            scope_lane_command(scope).expect("scope")
        );
        let trace_blake3 = blake3::hash(trace.as_bytes()).to_hex().to_string();
        let observed = scope_trace_paths_digest(trace.as_bytes());
        let resolved = resolved_corpus_paths_digest(
            &root,
            root.as_str(),
            trace.as_bytes(),
            "fixture",
            CorpusAccess::IfPresent,
        )
        .expect("resolved digest");
        let binding =
            blake3::hash(format!("{command}\0{pid}\0{}\0{trace_blake3}\0{observed}", 0).as_bytes())
                .to_hex()
                .to_string();
        let receipt_bytes = fs::read(access.join("strace.version")).expect("receipt bytes");
        let row = CorpusScopeTrace {
            scope: scope.to_owned(),
            command,
            trace: format!("conformance/haqp/evidence/access/scopes/{scope}.trace.zst"),
            trace_blake3,
            exit_code: 0,
            result: "pass".to_owned(),
            trace_pid: pid,
            trace_exit_code: 0,
            trace_complete: true,
            process_binding: binding,
            tracer_binary: "strace".to_owned(),
            tracer_version: "conformance/haqp/evidence/access/strace.version".to_owned(),
            tracer_version_blake3: blake3::hash(&receipt_bytes).to_hex().to_string(),
            observed_paths_blake3: observed,
            trace_root: root.to_string(),
            resolved_paths_blake3: resolved,
            parts: Vec::new(),
        };
        (scratch, root, row)
    }

    fn scope_audit(rows: Vec<CorpusScopeTrace>) -> CorpusAccessAudit {
        CorpusAccessAudit {
            schema_version: "haqp-corpus-access-v2".to_owned(),
            tracer: "strace-open-paths".to_owned(),
            source_commit: "0".repeat(40),
            source_tree: "0".repeat(40),
            targets: Vec::new(),
            scope_traces: rows,
        }
    }

    fn scope_provenance() -> Provenance {
        Provenance {
            commit: "0".repeat(40),
            lockfile_blake3: "0".repeat(64),
            fixed_commit: "0".repeat(40),
            fixed_tree: "0".repeat(40),
            evidence_parent: "0".repeat(40),
        }
    }

    /// The corpus-scope subsystem had never produced a row, so all three of
    /// these survived replacement with Ok(()): nothing had ever run them.
    #[test]
    fn a_well_formed_scope_row_is_accepted_and_each_binding_is_enforced() {
        type Doctor = Box<dyn Fn(&mut CorpusScopeTrace)>;
        let (_s, root, row) = scope_fixture("ci");
        verify_corpus_scope_rows(&root, &scope_audit(vec![row.clone()]))
            .expect("a row whose digests match its trace must verify");
        verify_corpus_scope_replays(&root, std::slice::from_ref(&row), &[])
            .expect("the captured trace must verify without re-execution");

        let cases: Vec<(&str, Doctor, &str)> = vec![
            (
                "wrong observed digest",
                Box::new(|r| r.observed_paths_blake3 = "a".repeat(64)),
                "observed_paths_blake3",
            ),
            (
                "wrong trace digest",
                Box::new(|r| r.trace_blake3 = "b".repeat(64)),
                "trace_blake3",
            ),
            ("non-zero exit", Box::new(|r| r.exit_code = 1), "exit_code"),
            (
                "result not pass",
                Box::new(|r| r.result = "fail".to_owned()),
                "result",
            ),
            (
                "not recursive (-f missing)",
                Box::new(|r| r.command = r.command.replacen("-f ", "", 1)),
                "corpus scope command",
            ),
            (
                "command off the closed registry",
                Box::new(|r| r.command.push_str(" --extra")),
                "corpus scope command",
            ),
            (
                "tracer not strace",
                Box::new(|r| r.tracer_binary = "ltrace".to_owned()),
                "tracer",
            ),
            (
                "pid absent from trace",
                Box::new(|r| r.trace_pid = 1),
                "traced PID",
            ),
            (
                "binding stale",
                Box::new(|r| r.process_binding = "c".repeat(64)),
                "process_binding",
            ),
        ];
        for (why, doctor, reason) in cases {
            let mut bad = row.clone();
            doctor(&mut bad);
            let err = verify_corpus_scope_rows(&root, &scope_audit(vec![bad])).expect_err(why);
            assert!(
                err.to_string().contains(reason),
                "{why}: must refuse for the stated reason, got: {err}"
            );
        }

        // Two rows for one scope.
        let dup = scope_audit(vec![row.clone(), row.clone()]);
        let err = verify_corpus_scope_rows(&root, &dup).expect_err("duplicate scope");
        assert!(err.to_string().contains("duplicate"), "{err}");
    }

    fn rebind_scope_row(row: &mut CorpusScopeTrace) {
        row.process_binding = blake3::hash(
            format!(
                "{}\0{}\0{}\0{}\0{}",
                row.command,
                row.trace_pid,
                row.trace_exit_code,
                row.trace_blake3,
                row.observed_paths_blake3
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
    }

    fn audit_target(
        target: &str,
        trace: &str,
        trace_blake3: &str,
        pid: u32,
    ) -> CorpusAccessAuditTarget {
        CorpusAccessAuditTarget {
            target: target.to_owned(),
            manifest: String::new(),
            manifest_blake3: String::new(),
            seed: 1,
            sanitizer: "address".to_owned(),
            exit_code: 0,
            log_blake3: String::new(),
            command: String::new(),
            trace: trace.to_owned(),
            trace_blake3: trace_blake3.to_owned(),
            trace_pid: pid,
            trace_exit_code: 0,
            trace_complete: true,
            process_binding: String::new(),
            tracer_binary: "strace".to_owned(),
            tracer_version: String::new(),
            tracer_version_blake3: String::new(),
            trace_root: String::new(),
            resolved_paths_blake3: String::new(),
        }
    }

    /// F-43: the scan that judges a trace streams it line by line, through
    /// zstd. It must extract exactly what the whole-buffer extraction did and
    /// report the pid receipts and raw digest the row checks bind.
    #[test]
    fn a_streamed_scan_matches_the_whole_buffer_extraction() {
        let trace = "7 openat(AT_FDCWD, \"/x/fuzz/corpus/t/a.md\", O_RDONLY) = 3\n\
                     7 newfstatat(AT_FDCWD, \"/x/b\", {st_mode=S_IFREG|0644}, 0) = 0\n\
                     8 execve(\"/usr/bin/true\", [\"true\"], 0x1 /* 1 var */) = 0\n\
                     8 +++ exited with 0 +++\n\
                     7 +++ exited with 1 +++\n";
        let scratch = liminal_scratch::ScratchDir::new("haq-scan").expect("scratch");
        let stored = scratch.path().join("t.trace.zst");
        fs::write(
            &stored,
            zstd::encode_all(trace.as_bytes(), 3).expect("zstd"),
        )
        .expect("write");
        let scan = scan_scope_trace_file(&stored, "fixture").expect("scan");
        assert_eq!(scan.open_paths, scope_trace_open_paths(trace.as_bytes()));
        assert_eq!(scan.open_paths.len(), 3);
        assert_eq!(scan.pids.iter().copied().collect::<Vec<_>>(), vec![7, 8]);
        assert_eq!(
            scan.exited_zero.iter().copied().collect::<Vec<_>>(),
            vec![8]
        );
        assert_eq!(
            scan.raw_blake3,
            blake3::hash(trace.as_bytes()).to_hex().to_string()
        );
        assert_eq!(scan.forbidden, None);
        let touched =
            format!("{trace}9 openat(AT_FDCWD, \"conformance/corpora/HeldOut/z\", O_RDONLY) = 4\n");
        let scan = scan_scope_trace(touched.as_bytes()).expect("scan");
        assert_eq!(scan.forbidden, Some("heldout"));
        let corrupt = scratch.path().join("bad.trace.zst");
        fs::write(&corrupt, b"not zstd").expect("write");
        let err = scan_scope_trace_file(&corrupt, "fixture").expect_err("corrupt zstd");
        assert!(err.to_string().contains("fixture"), "{err}");
    }

    /// F-43: the stage tracer holds the one ptrace slot, so each fuzz binary's
    /// lines are carved from the stage trace into the target's own file and the
    /// scope row names them as parts. The gate judges the UNION -- the corpus
    /// opens live in the parts, the remainder has none -- and binds every part
    /// to the campaign's per-target traces, so a part cannot be dropped,
    /// forged, or claimed by a scope that carves nothing.
    #[test]
    fn a_fuzz_scope_is_judged_on_the_union_of_its_carved_parts() {
        let (_s, root, mut row) = scope_fixture("fuzz");
        let seed_dir = root.join("fuzz/corpus/cst_parse");
        fs::create_dir_all(&seed_dir).expect("mkdir");
        fs::write(seed_dir.join("01-empty.md"), "").expect("seed");
        let part_trace = format!(
            "5150 openat(AT_FDCWD, \"{root}/fuzz/corpus/cst_parse/01-empty.md\", O_RDONLY) = 3\n\
             5150 +++ exited with 0 +++\n"
        );
        let part_rel = "conformance/haqp/evidence/access/cst_parse.trace.zst";
        fs::write(
            root.join(part_rel),
            zstd::encode_all(part_trace.as_bytes(), 3).expect("zstd"),
        )
        .expect("part");
        let part_blake3 = blake3::hash(part_trace.as_bytes()).to_hex().to_string();
        let targets = vec![audit_target("cst_parse", part_rel, &part_blake3, 5150)];

        // The remainder alone names no parts: refused against the campaign's targets.
        let err = verify_corpus_scope_replays(&root, std::slice::from_ref(&row), &targets)
            .expect_err("a fuzz scope without its parts");
        assert!(err.to_string().contains("parts differ"), "{err}");

        row.parts = vec![ScopeTracePart {
            trace: part_rel.to_owned(),
            trace_blake3: part_blake3.clone(),
        }];
        // Digests over the union, by the same code the lane asks for them.
        let files = [root.join(&row.trace), root.join(part_rel)];
        row.observed_paths_blake3 =
            scope_trace_digest_repo(&root, "paths", &files, "fuzz").expect("paths digest");
        row.resolved_paths_blake3 =
            scope_trace_digest_repo(&root, "resolved", &files, "fuzz").expect("resolved digest");
        rebind_scope_row(&mut row);
        verify_corpus_scope_replays(&root, std::slice::from_ref(&row), &targets)
            .expect("the union of remainder and parts must verify");
        let mut audit = scope_audit(vec![row.clone()]);
        audit.targets.clone_from(&targets);
        verify_corpus_scope_rows(&root, &audit).expect("the row must verify");

        // The remainder alone would have fuzzed nothing: the parts carry the corpus.
        let err = scope_trace_digest_repo(&root, "resolved", &files[..1], "fuzz")
            .expect_err("remainder alone");
        assert!(
            err.to_string().contains("no resolvable fuzz corpus path"),
            "{err}"
        );

        // A part digest the campaign did not record.
        let mut forged = row.clone();
        forged.parts[0].trace_blake3 = "d".repeat(64);
        let err = verify_corpus_scope_replays(&root, std::slice::from_ref(&forged), &targets)
            .expect_err("a part the campaign did not produce");
        assert!(err.to_string().contains("parts differ"), "{err}");
        // ...and bytes that changed after both the row and the campaign recorded them.
        let mut forged_targets = targets.clone();
        forged_targets[0].trace_blake3 = forged.parts[0].trace_blake3.clone();
        let err =
            verify_corpus_scope_replays(&root, std::slice::from_ref(&forged), &forged_targets)
                .expect_err("part bytes differ from the declared digest");
        assert!(
            err.to_string().contains("part") && err.to_string().contains("trace_blake3"),
            "{err}"
        );

        // A part that touched the locked corpus is the scope's refusal.
        let touched = format!(
            "{part_trace}5150 openat(AT_FDCWD, \"{root}/conformance/corpora/heldout/x.md\", O_RDONLY) = 4\n"
        );
        fs::write(
            root.join(part_rel),
            zstd::encode_all(touched.as_bytes(), 3).expect("zstd"),
        )
        .expect("rewrite");
        let touched_blake3 = blake3::hash(touched.as_bytes()).to_hex().to_string();
        let mut touched_row = row.clone();
        touched_row.parts[0]
            .trace_blake3
            .clone_from(&touched_blake3);
        let mut touched_targets = targets.clone();
        touched_targets[0].trace_blake3 = touched_blake3;
        let err = verify_corpus_scope_replays(
            &root,
            std::slice::from_ref(&touched_row),
            &touched_targets,
        )
        .expect_err("a carved part that opened the locked corpus");
        assert!(err.to_string().contains("locked path"), "{err}");

        // Only fuzz carves.
        let (_s2, root2, mut ci) = scope_fixture("ci");
        ci.parts = vec![ScopeTracePart {
            trace: "conformance/haqp/evidence/access/x.trace.zst".to_owned(),
            trace_blake3: "e".repeat(64),
        }];
        let err = verify_corpus_scope_replays(&root2, std::slice::from_ref(&ci), &[])
            .expect_err("a ci scope with parts");
        assert!(err.to_string().contains("only fuzz carves"), "{err}");
    }

    fn rewrite_scope_trace(root: &Utf8Path, row: &mut CorpusScopeTrace, trace: &str) {
        fs::write(
            root.join(&row.trace),
            zstd::encode_all(trace.as_bytes(), 3).expect("zstd"),
        )
        .expect("rewrite");
        row.trace_blake3 = blake3::hash(trace.as_bytes()).to_hex().to_string();
        let files = [root.join(&row.trace)];
        row.observed_paths_blake3 =
            scope_trace_digest_repo(root, "paths", &files, &row.scope).expect("paths");
        row.resolved_paths_blake3 =
            scope_trace_digest_repo(root, "resolved", &files, &row.scope).expect("resolved");
        rebind_scope_row(row);
    }

    /// Blind pass 1 (2026-09-05) A08: serde dropped a field the schema did not
    /// declare, and the typed reserialization digested the packet without it.
    #[test]
    fn a_packet_with_an_undeclared_field_is_refused() {
        let root = repo_root();
        let mut value: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/packet.json")).expect("packet"),
        )
        .expect("json");
        value["exceptions"] = serde_json::json!(["*"]);
        let err = serde_json::from_value::<Packet>(value.clone())
            .expect_err("a wildcard exceptions field outside the schema");
        assert!(err.to_string().contains("unknown field"), "{err}");
        value.as_object_mut().expect("object").remove("exceptions");
        value["mutants"][0]["exempt"] = serde_json::json!(true);
        let err = serde_json::from_value::<Packet>(value).expect_err("a row-level unknown field");
        assert!(err.to_string().contains("unknown field"), "{err}");
    }

    /// Blind pass 1 (2026-09-05) A05: the not-applicable label was trusted.
    /// The scan refuses concurrent execution in non-test implementation code,
    /// records synchronization primitives, and skips the test module.
    #[test]
    fn the_not_applicable_label_is_checked_against_the_implementation() {
        let scratch = liminal_scratch::ScratchDir::new("haq-conc").expect("scratch");
        let root = scratch.path().to_owned();
        let src = root.join("crates/store/src");
        fs::create_dir_all(&src).expect("mkdir");
        fs::write(
            src.join("lib.rs"),
            "use std::sync::Mutex;\npub struct S { inner: Mutex<u8> }\n// thread::spawn in a comment\n#[cfg(test)]\nmod tests { fn t() { std::thread::spawn(|| {}); } }\n",
        )
        .expect("write");
        fs::create_dir_all(root.join("crates/liminal-xtask/src")).expect("mkdir");
        fs::write(
            root.join("crates/liminal-xtask/src/lib.rs"),
            "async fn gate() {}\n",
        )
        .expect("write");
        let scan = scan_concurrency_primitives(&root).expect("scan");
        assert!(
            scan.execution.is_empty(),
            "test module and excluded crate: {:?}",
            scan.execution
        );
        assert_eq!(
            scan.sync,
            vec!["crates/store/src/lib.rs:2:Mutex<".to_owned()]
        );
        fs::write(
            src.join("worker.rs"),
            "pub fn go() { std::thread::spawn(|| {}); }\n",
        )
        .expect("write");
        let scan = scan_concurrency_primitives(&root).expect("scan");
        assert_eq!(
            scan.execution,
            vec!["crates/store/src/worker.rs:1:::spawn".to_owned()]
        );
    }

    /// Blind pass 1 (2026-09-05) A01: C26 must strip a provenance the
    /// verifier ACCEPTED, or it only re-observes the baseline's refusal.
    #[test]
    fn c26_strips_an_accepted_provenance_rather_than_an_absent_one() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        let (_scratch, scratch_root, qualified) =
            provenance_canary_repo(&root, &packet).expect("repo");
        verify_provenance(&scratch_root, &qualified).expect("the bound block must verify");
        let err = run_qualification_canary(&root, &packet, "C26").expect_err("the canary refuses");
        assert!(err.to_string().contains("no provenance block"), "{err}");
    }

    /// Blind pass 1 at 612cbcc (A02): a pure move kept only the marker SET;
    /// swapping two moved markers' contents passed.
    #[test]
    fn a_move_that_swaps_marker_contents_is_refused() {
        use liminal_source::merge::MergeOutcome;
        let base = "alpha {#x}\n\nbeta {#y}";
        let ours = "beta {#y}\n\nalpha {#x}";
        let faithful = MergeOutcome::Disjoint {
            merged: ours.to_owned(),
        };
        let swapped = MergeOutcome::Disjoint {
            merged: "alpha {#y}\n\nbeta {#x}".to_owned(),
        };
        independent_oracle_source_transform(base, ours, &faithful, &faithful, &faithful)
            .expect("a faithful move keeps every marker's content");
        let err = independent_oracle_source_transform(base, ours, &swapped, &swapped, &swapped)
            .expect_err("swapped contents under moved markers");
        assert!(err.to_string().contains("re-associated"), "{err}");
    }

    /// Blind pass 1 at 612cbcc (A03): each negative category constructs the
    /// shape it names, not a label.
    #[test]
    fn transform_negatives_are_shaped_like_their_category() {
        let mut rng = Rng(7);
        assert!(transform_negative_input("empty", &mut rng).is_empty());
        let span = transform_negative_input("invalid-span", &mut rng);
        assert!(
            span.contains("{#open\n") && span.contains("{# spaced }"),
            "{span:?}"
        );
        let truncated = transform_negative_input("truncated", &mut rng);
        assert!(truncated.ends_with("{#cu"), "{truncated:?}");
        let hostile = transform_negative_input("hostile", &mut rng);
        assert!(hostile.contains('\0') && hostile.contains("{#{#nested}}") && hostile.len() > 4096);
        for _ in 0..64 {
            case_transform(&mut rng).expect("the merge is total over shaped negatives");
        }
    }

    /// Blind pass 1 at e09ae5e (A09): the gate checked headers, the packet
    /// digest, the status tuple, the canary table and the review blocks, so a
    /// human-facing cell in any other rendered table could claim a result no
    /// artifact supports while everything checked stayed valid.
    #[test]
    fn a_rendered_table_may_not_claim_what_the_packet_does_not_declare() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        verify_markdown_tables(&root, &packet).expect("the committed markdown is derivable");

        let scratch = liminal_scratch::ScratchDir::new("haq-tables").expect("scratch");
        let scratch_root = scratch.path().to_owned();
        let rel = "docs/execution/phase1-suite-review.md";
        fs::create_dir_all(scratch_root.join("docs/execution")).expect("mkdir");
        let original = fs::read_to_string(root.join(rel)).expect("markdown");

        // None of these is derivable from an unqualified packet.
        for (from, to, why) in [
            (
                "| source/CST/formatting | 13 | NOT_RUN |",
                "| source/CST/formatting | 13 | executed |",
                "an execution claim",
            ),
            (
                "| NOT_RUN | NOT_RUN | NOT_RUN |",
                "| pass | pass | pass |",
                "a verdict",
            ),
            (
                "| qualification state | NOT_RUN |",
                "| qualification state | complete |",
                "a status verdict",
            ),
            (
                "| locked acceptance corpora touched | NOT_RUN — prohibited |",
                "| locked acceptance corpora touched | NOT_RUN — passed |",
                "a verdict smuggled behind the marker",
            ),
        ] {
            let doctored = original.replacen(from, to, 1);
            assert_ne!(doctored, original, "the fixture must change: {why}");
            fs::write(scratch_root.join(rel), &doctored).expect("write");
            let err = verify_markdown_tables(&scratch_root, &packet)
                .expect_err(why)
                .to_string();
            assert!(
                err.contains("does not declare") || err.contains("while the packet is unqualified"),
                "{why}: {err}"
            );
        }

        // A row wider than its header claims something the header cannot name.
        let wide = original.replacen(
            "| source/CST/formatting | 13 | NOT_RUN |",
            "| source/CST/formatting | 13 | NOT_RUN | NOT_RUN |",
            1,
        );
        assert_ne!(wide, original);
        fs::write(scratch_root.join(rel), &wide).expect("write");
        let err = verify_markdown_tables(&scratch_root, &packet)
            .expect_err("a row wider than its header")
            .to_string();
        assert!(err.contains("cell") && err.contains("header"), "{err}");

        // The untouched copy still verifies, so the refusals above are earned.
        fs::write(scratch_root.join(rel), &original).expect("write");
        verify_markdown_tables(&scratch_root, &packet).expect("an unmodified copy verifies");
    }

    /// Review of `f4d42d3`: a `pub(crate)` helper named no prefix the closure
    /// recognised, so the oracle could reach production through it.
    #[test]
    fn the_oracle_closure_follows_helpers_whatever_their_visibility() {
        let text = concat!(
            "fn oracle(x: u8) -> u8 {\n    helper(x)\n}\n\n",
            "pub(crate) fn helper(x: u8) -> u8 {\n    deeper(x)\n}\n\n",
            "pub(crate) async fn deeper(x: u8) -> u8 {\n    later(x)\n}\n\n",
            "pub(super) const fn later(x: u8) -> u8 {\n    innermost(x)\n}\n\n",
            "pub(in crate::inner) unsafe fn innermost(x: u8) -> u8 {\n    forbidden_path(x)\n}\n\n",
            "fn unrelated() -> u8 {\n    other_thing()\n}\n",
        );
        let body = oracle_reachable_body(text, "oracle");
        assert!(
            body.contains("forbidden_path("),
            "every visibility form on the path is part of the oracle: {body}"
        );
        assert!(
            !body.contains("other_thing("),
            "a function the oracle never reaches is not"
        );
    }

    /// Blind pass 1 at c29bc0ea (A08): a ruling matched on class, file and
    /// prose, so an unrelated defect in the same file could be cleared by
    /// wording alone. A ruling that names a coordinate is held to it, and no
    /// ruling clears two findings in one record.
    #[test]
    fn a_ruling_is_held_to_its_coordinate_and_clears_one_finding() {
        let ruling = |target: &str| Ruling {
            id: "R-900".to_owned(),
            attack_class: "corpus leakage".to_owned(),
            target: target.to_owned(),
            claim_requires: vec!["read access".to_owned()],
            claim_excludes: Vec::new(),
            status: "ruled".to_owned(),
            file: "docs/execution/rulings/R-900.md".to_owned(),
        };
        let attempt = |id: &str, target: &str, prose: &str| {
            format!(
                r#"{{"id":"{id}","attack_class":"corpus leakage","target":"{target}","attempt":"a","observed_result":"{prose}","classification":"verified_defect","independently_reproduced":true,"resolved":false}}"#
            )
        };
        let record = |attempts: &[String]| -> serde_json::Value {
            serde_json::from_str(&format!(r#"{{"attempts":[{}]}}"#, attempts.join(",")))
                .expect("record")
        };
        let path = Utf8Path::new("r.json");
        let here = "crates/liminal-xtask/src/haq.rs:120";
        let elsewhere = "crates/liminal-xtask/src/haq.rs:900";

        // A file-level ruling still answers the claim it was written for.
        let one = record(&[attempt(
            "A1",
            here,
            "unauthorized read access remains accepted",
        )]);
        assert_eq!(
            effective_unresolved_findings(&[ruling("crates/liminal-xtask/src/haq.rs")], &one, path)
                .expect("count"),
            0
        );

        // Named to a coordinate, it does not reach a different line.
        assert_eq!(
            effective_unresolved_findings(&[ruling(elsewhere)], &one, path).expect("count"),
            1,
            "a ruling naming another line answers nothing here"
        );
        assert_eq!(
            effective_unresolved_findings(&[ruling(here)], &one, path).expect("count"),
            0,
            "a ruling naming this line answers it"
        );

        // Two findings whose prose both match is the broad-phrase failure.
        let two = record(&[
            attempt("A1", here, "unauthorized read access remains accepted"),
            attempt("A2", elsewhere, "a second defect: read access is unchecked"),
        ]);
        let err =
            effective_unresolved_findings(&[ruling("crates/liminal-xtask/src/haq.rs")], &two, path)
                .expect_err("a ruling that clears two findings is too broad");
        assert!(err.to_string().contains("too broad to stand"), "{err}");
    }

    /// Review of `cff54890`: `RESULT_BEARING_TABLES` is matched by exact
    /// header, so a header it names but `RENDERED_TABLES` does not would
    /// relax nothing and say nothing.
    #[test]
    fn every_result_bearing_table_is_a_rendered_table() {
        for header in RESULT_BEARING_TABLES {
            assert!(
                RENDERED_TABLES.contains(&header),
                "{header:?} relaxes a table the gate never reads"
            );
        }
    }

    /// AM-17.10: the killer column is derived, so a hand-authored one is
    /// refused however plausible it looks — 38 of 65 mutants once had neither
    /// killer covering their own requirement and no gate read the column,
    /// because the checks below it inspect only `killed` mutants.
    #[test]
    fn a_hand_authored_killer_column_is_refused() {
        let packet = packet_from_repo();
        verify_mutant_killing_tests(&repo_root(), &packet)
            .expect("the committed column is the derived one");

        let mut doctored = packet.clone();
        let victim = &mut doctored.mutants[0];
        let derived = victim.killing_tests.clone();
        // A test that exists and defends the same requirement, but is not the
        // one the rule derives: plausible by eye, refused by construction.
        victim.killing_tests = vec![derived[1].clone(), derived[0].clone()];
        let err = verify_mutant_killing_tests(&repo_root(), &doctored)
            .expect_err("a reordered pair is not the derived pair");
        assert!(err.to_string().contains("AM-17.10 derives"), "{err}");

        let mut unknown = packet.clone();
        unknown.mutants[0].operator = "invented-operator".to_owned();
        let err = verify_mutant_killing_tests(&repo_root(), &unknown)
            .expect_err("an operator with no observation kind");
        assert!(
            err.to_string().contains("names no observation kind"),
            "{err}"
        );
    }

    /// AM-17.10's operator table is closed, and a mutant carrying an operator
    /// it does not name cannot be derived at all. Caught here rather than in
    /// the gate, where it would surface as a lane refusal.
    #[test]
    fn the_operator_table_is_closed_and_every_killer_pair_observes_two_ways() {
        let packet = packet_from_repo();
        for mutant in &packet.mutants {
            assert!(
                MUTANT_OBSERVATION_KIND
                    .iter()
                    .any(|(operator, _)| *operator == mutant.operator),
                "{} carries operator {:?}, absent from MUTANT_OBSERVATION_KIND",
                mutant.id,
                mutant.operator
            );
        }
        let by_id: BTreeMap<&str, &Test> =
            packet.tests.iter().map(|t| (t.id.as_str(), t)).collect();
        for mutant in &packet.mutants {
            assert_eq!(
                mutant.killing_tests.len(),
                2,
                "{} names {} killing tests; AM-17.10 derives a pair",
                mutant.id,
                mutant.killing_tests.len()
            );
            let named = |id: &str| {
                *by_id
                    .get(id)
                    .unwrap_or_else(|| panic!("{} names killing test {id}, undeclared", mutant.id))
            };
            let first = named(&mutant.killing_tests[0]);
            let second = named(&mutant.killing_tests[1]);
            assert_ne!(
                first.evidence, second.evidence,
                "{} observes one way twice: {} and {} are both {:?}",
                mutant.id, first.id, second.id, first.evidence
            );
        }
    }

    /// Blind pass 1 at `aeed70f9` (A01, A11): the lexical operator contract
    /// lived only in `verify_mutant_operator_patch`, which needs a patch, and
    /// at stage 1a there are none — so six anchors declared operators their
    /// own text could not support.
    #[test]
    fn an_anchor_must_be_able_to_be_the_before_side_of_its_operator() {
        let packet = packet_from_repo();
        verify_mutant_anchors_support_operators(&repo_root(), &packet)
            .expect("every committed anchor supports its operator");

        assert!(anchor_supports_operator(
            "predicate-deletion",
            "if !author_keyed {"
        ));
        assert!(
            !anchor_supports_operator("predicate-deletion", "if found != expected_pre {"),
            "deleting the `!` of a `!=` leaves `=`, which is not a program"
        );
        assert!(
            !anchor_supports_operator("predicate-deletion", "matches!(byte, b'_')"),
            "a macro's bang is not a negation"
        );
        assert!(anchor_supports_operator(
            "missing-enum-dispatch",
            "match selection.and_then(|m| m.get(key)) {"
        ));
        assert!(
            !anchor_supports_operator(
                "missing-enum-dispatch",
                "| BasisComponent::ExternalRevision { .. } => Durability::Low,"
            ),
            "an arm is not the match it belongs to"
        );
        assert!(
            !anchor_supports_operator(
                "predicate-inversion",
                "pub components: BTreeMap<JurisdictionKey, BasisComponent>,"
            ),
            "a generic parameter is not a comparison"
        );
        assert!(anchor_supports_operator(
            "skipped-durable-transition",
            "fs::rename(&self.staged, &self.target)?;"
        ));

        let mut doctored = packet.clone();
        doctored.mutants[0].operator = "ordering-nondeterminism".to_owned();
        let err = verify_mutant_anchors_support_operators(&repo_root(), &doctored)
            .expect_err("an operator its anchor cannot support");
        assert!(
            err.to_string().contains("cannot be the BEFORE side"),
            "{err}"
        );
    }

    /// Blind pass 1 at `aeed70f9` (A07): the write check resolved symlinks and
    /// pathname ancestry. A hard link is neither — it is a second name for the
    /// same inode — so writing through one outside the corpus mutated corpus
    /// bytes under a path that looked innocent. Built in scratch; the real
    /// locked corpus is never touched.
    #[test]
    fn a_hard_link_into_the_locked_corpus_is_refused() {
        let scratch = liminal_scratch::ScratchDir::new("haq-hardlink").expect("scratch");
        let root = scratch.path().to_owned();
        let corpus = root.join("conformance/corpora/heldout");
        fs::create_dir_all(corpus.as_std_path()).expect("mkdir corpus");
        let locked = corpus.join("case.txt");
        fs::write(locked.as_std_path(), b"locked bytes").expect("write");
        let elsewhere = root.join("scratch-name.txt");
        fs::hard_link(locked.as_std_path(), elsewhere.as_std_path()).expect("hard link");

        let innocent = root.join("ordinary.txt");
        fs::write(innocent.as_std_path(), b"mine").expect("write");
        assert!(
            locked_corpus_write(&root, &[innocent.to_string()], "probe")
                .expect("check")
                .is_none(),
            "an ordinary file is not the corpus"
        );

        let flagged = locked_corpus_write(&root, &[elsewhere.to_string()], "probe")
            .expect("check")
            .expect("a hard link into the corpus is a write to the corpus");
        assert!(flagged.contains("hard link"), "{flagged}");

        // Blind pass 1 at `6b36bbb9` (A02): the name must not matter. A link
        // whose path resembles nothing is the case identity exists for.
        let unmarked = root.join("scratch.bin");
        fs::hard_link(locked.as_std_path(), unmarked.as_std_path()).expect("second link");
        let flagged = locked_corpus_write(&root, &[unmarked.to_string()], "probe")
            .expect("check")
            .expect("a link named nothing like the corpus is still the corpus");
        assert!(flagged.contains("hard link"), "{flagged}");
    }

    /// Blind pass 1 at `aeed70f9` (A09): the transcript's bytes were
    /// authenticated and its identity was not, so a record could name one
    /// session and retain another's rollout.
    #[test]
    fn a_receipt_must_retain_the_session_it_names() {
        let (scratch, path) = fixture_record_path();
        let mut record = review_record(1, "openai", "codex");
        record.provider_receipt.session_id = "fixture".to_owned();
        record.provider_receipt.session_file = "rollout-fixture.jsonl".to_owned();
        record.provider_receipt.transcript_bytes = FIXTURE_TRANSCRIPT.len() as u64;
        record.provider_receipt.transcript_sha256 =
            format!("{:x}", Sha256::digest(FIXTURE_TRANSCRIPT));
        verify_review_provider_receipt(&record, &path)
            .expect("the retained session is the named one");

        // Another session's rollout, correctly hashed: the bytes authenticate,
        // the identity does not.
        let other = b"{\"type\":\"session_meta\",\"payload\":{\"session_id\":\"someone-else\"}}\n";
        fs::write(scratch.path().join("r.session.jsonl"), other).expect("transcript");
        let mut substituted = record.clone();
        substituted.provider_receipt.transcript_bytes = other.len() as u64;
        substituted.provider_receipt.transcript_sha256 = format!("{:x}", Sha256::digest(other));
        let err = verify_review_provider_receipt(&substituted, &path)
            .expect_err("a substituted session identity");
        assert!(
            err.to_string().contains("retained transcript is session"),
            "{err}"
        );

        // A suffix is not an identity: `xfixture` is a different session from
        // `fixture`, and only a separator makes the codex `<ts>-<uuid>` form.
        fs::write(scratch.path().join("r.session.jsonl"), FIXTURE_TRANSCRIPT).expect("transcript");
        let mut suffix = record.clone();
        suffix.provider_receipt.session_id = "xfixture".to_owned();
        suffix.provider_receipt.session_file = "rollout-xfixture.jsonl".to_owned();
        let err = verify_review_provider_receipt(&suffix, &path)
            .expect_err("a session id that merely ends with the declared one");
        assert!(
            err.to_string().contains("retained transcript is session"),
            "{err}"
        );
        let mut separated = record.clone();
        separated.provider_receipt.session_id = "2026-09-10T06-56-23-fixture".to_owned();
        separated.provider_receipt.session_file =
            "rollout-2026-09-10T06-56-23-fixture.jsonl".to_owned();
        verify_review_provider_receipt(&separated, &path)
            .expect("the codex <timestamp>-<uuid> form is the same session");

        // A rollout that does not open with its own metadata at all.
        fs::write(
            scratch.path().join("r.session.jsonl"),
            b"{\"type\":\"event_msg\"}\n",
        )
        .expect("transcript");
        let mut headless = record.clone();
        headless.provider_receipt.transcript_bytes = b"{\"type\":\"event_msg\"}\n".len() as u64;
        headless.provider_receipt.transcript_sha256 =
            format!("{:x}", Sha256::digest(b"{\"type\":\"event_msg\"}\n"));
        let err = verify_review_provider_receipt(&headless, &path)
            .expect_err("a rollout with no session_meta");
        assert!(err.to_string().contains("opens with session_meta"), "{err}");
    }

    /// Blind pass 1 at `aeed70f9` (A02): the alias scan watched the call and
    /// module forms, so `let q = p; q(input)` reached production under a name
    /// the scan had never heard of.
    #[test]
    fn an_alias_handed_on_as_a_value_is_refused() {
        let scratch = liminal_scratch::ScratchDir::new("haq-oracle-value").expect("scratch");
        let root = scratch.path().to_owned();
        write_independent_gate_oracles(&root);
        let file = root.join("crates/liminal-query/src/lib.rs");
        fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
        fs::write(
            &file,
            "use liminal_source::paragraph::parse as p;\n\
             fn incremental_paragraph_oracle(input: &str) -> Vec<u8> {\n\
             \x20   let q = p;\n    q(input)\n}\n",
        )
        .expect("write");
        let err = verify_oracle_independence(&root).expect_err("an alias handed on as a value");
        assert!(err.to_string().contains("aliased production path"), "{err}");

        assert!(mentions_word("let q = p;", "p"));
        assert!(
            !mentions_word("let paragraphs = split(input);", "p"),
            "a one-letter alias must not match inside every identifier"
        );
    }

    /// M17.5 F-65: the sweep for gates that pass vacuously when the thing they
    /// check is absent. The corpus one was real — nothing required the locked
    /// corpus to exist, so deleting it cleared every corpus check.
    #[test]
    fn the_locked_corpus_must_exist_to_be_shown_untouched() {
        verify_locked_corpus_has_no_aliases(&repo_root()).expect("the committed tree has it");
        let scratch = liminal_scratch::ScratchDir::new("haq-no-corpus").expect("scratch");
        let err = verify_locked_corpus_has_no_aliases(scratch.path())
            .expect_err("a tree with no locked corpus proves nothing about it");
        assert!(err.to_string().contains("is missing"), "{err}");

        // Blind pass 1 at `ec045588` (A08): only symlinks counted. A hard link
        // is the alias no path reasoning sees, and one made outside a traced
        // process leaves the trace naming an innocent path — which, once the
        // link is removed, nothing afterwards can resolve. It is found here,
        // while it still exists.
        let tree = liminal_scratch::ScratchDir::new("haq-hard-alias").expect("scratch");
        let root = tree.path().to_owned();
        let corpus = root.join("conformance/corpora/heldout");
        fs::create_dir_all(corpus.as_std_path()).expect("mkdir");
        fs::write(corpus.join("case.txt").as_std_path(), b"held out").expect("write");
        verify_locked_corpus_has_no_aliases(&root).expect("a corpus with no alias");
        let alias = root.join("innocent-name.bin");
        fs::hard_link(corpus.join("case.txt").as_std_path(), alias.as_std_path())
            .expect("hard link");
        let err = verify_locked_corpus_has_no_aliases(&root)
            .expect_err("a second name for a corpus file is an alias");
        assert!(err.to_string().contains("alias"), "{err}");
    }

    /// The plan's gates must run before the flip, not only after it: a lane
    /// costs hours, and `haq verify-inventory` is what is run between them.
    #[test]
    fn the_inventory_path_reads_the_mutation_plan() {
        let source = fs::read_to_string(repo_root().join("crates/liminal-xtask/src/haq.rs"))
            .expect("read the gate");
        let body = item_body(&source, "verify_inventory_repo").expect("inventory entry point");
        for gate in [
            "verify_mutant_source_coordinates",
            "verify_mutant_anchors_support_operators",
            "verify_mutant_killing_tests",
        ] {
            assert!(
                body.contains(gate),
                "the inventory path does not call {gate}, so a plan defect costs a whole lane"
            );
        }
    }

    /// Blind pass 1 at `6b36bbb9` (A03): the table lookup found the FIRST
    /// header and stopped, so a second table spelling the same header was
    /// never read and its rows could claim anything.
    #[test]
    fn every_table_under_a_header_is_read_not_only_the_first() {
        let header = "| Family | Planned | Executed |";
        let text = format!(
            "{header}\n|---|---|---|\n| a | 1 | 2 |\n\nprose\n\n{header}\n|---|---|---|\n| b | 3 | 4 |\n"
        );
        let tables = markdown_tables(&text, header);
        assert_eq!(tables.len(), 2, "both tables are read");
        assert_eq!(tables[0].1[0], vec!["a", "1", "2"]);
        assert_eq!(tables[1].1[0], vec!["b", "3", "4"]);
        assert_eq!(tables[0].0, 3, "each table brings its own header width");

        // A header mentioned mid-sentence is prose, not a table.
        let inline = format!("see {header} above\n");
        assert!(markdown_tables(&inline, header).is_empty());
    }

    /// Blind pass 1 at `6b36bbb9` (A01): the oracle closure followed only the
    /// CALL form, so `let h = helper; h(input)` reached a body it never read.
    #[test]
    fn the_oracle_closure_follows_a_helper_named_as_a_value() {
        let text = concat!(
            "fn oracle(x: u8) -> u8 {\n    let h = helper;\n    h(x)\n}\n\n",
            "fn helper(x: u8) -> u8 {\n    forbidden_path(x)\n}\n\n",
            "fn unrelated() -> u8 {\n    other_thing()\n}\n",
        );
        let body = oracle_reachable_body(text, "oracle");
        assert!(
            body.contains("forbidden_path("),
            "naming a function is reaching it: {body}"
        );
        assert!(!body.contains("other_thing("));
    }

    /// Blind pass 1 at `6b36bbb9` (A04): concurrence checked that the cited
    /// finding exists and was reproduced, never that it was ABOUT the mutant —
    /// so any reproduced defect in the record certified any mutant's claim of
    /// equivalence.
    #[test]
    fn an_equivalence_concurrence_must_cite_a_finding_about_that_mutant() {
        let scratch = liminal_scratch::ScratchDir::new("haq-concurrence").expect("scratch");
        let root = scratch.path().to_owned();
        let mut packet = packet_from_repo();
        let mutant_id = packet.mutants[0].id.clone();
        let coordinate = packet.mutants[0].source.clone();
        packet.mutants[0].disposition = "equivalent".to_owned();
        packet.mutants[0].disposition_proof = Some("same observable function".to_owned());

        let write_record = |pass: u8, family: &str, prose: &str| -> String {
            let mut record = review_record(pass, family, "codex");
            record.attempts = vec![ReviewRecordAttempt {
                id: "A1".to_owned(),
                attack_class: "weak mutants".to_owned(),
                target: "conformance/haqp/packet.json".to_owned(),
                attempt: "argue equivalence".to_owned(),
                observed_result: prose.to_owned(),
                independently_reproduced: true,
                classification: "verified_defect".to_owned(),
                resolved: false,
                resolution: None,
            }];
            record.findings = vec![serde_json::json!({"id": "F-eq", "attempt_id": "A1"})];
            record.independently_reproduced = vec!["F-eq".to_owned()];
            let rel = format!("conformance/haqp/evidence/reviews/pass{pass}.json");
            let path = root.join(&rel);
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(&path, serde_json::to_vec(&record).expect("serialize")).expect("write");
            rel
        };

        let unrelated = "the campaign clock is self-consistent and proves nothing";
        packet.reviews[0].result = "pass".to_owned();
        packet.reviews[1].result = "pass".to_owned();
        packet.reviews[0].evidence = Some(write_record(1, "openai", unrelated));
        packet.reviews[1].evidence = Some(write_record(2, "mimo", unrelated));
        packet.mutants[0].disposition_concurrence = vec![
            DispositionConcurrence {
                reviewer: packet.reviews[0].reviewer.clone(),
                record: "pass-1".to_owned(),
                finding: "F-eq".to_owned(),
            },
            DispositionConcurrence {
                reviewer: packet.reviews[1].reviewer.clone(),
                record: "pass-2".to_owned(),
                finding: "F-eq".to_owned(),
            },
        ];
        let err = verify_mutant_concurrence(&root, &packet)
            .expect_err("a finding about something else is not agreement about this mutant");
        assert!(
            err.to_string().contains("names neither the mutant"),
            "{err}"
        );

        let about = format!("{mutant_id} at {coordinate} is equivalent under the declared domain");
        packet.reviews[0].evidence = Some(write_record(1, "openai", &about));
        packet.reviews[1].evidence = Some(write_record(2, "mimo", &about));
        verify_mutant_concurrence(&root, &packet)
            .expect("a finding naming the mutant is agreement about it");
    }

    /// A packet with no declared reviews, for clock tests that are about the
    /// budget rather than the provider-clock binding A07 added.
    fn no_reviews_packet() -> Packet {
        let mut packet = packet_from_repo();
        packet.reviews.clear();
        packet
    }

    /// Blind pass 1 at `6b36bbb9` (A07): the campaign clock's numbers are all
    /// the qualifier's own, and `finished - started == elapsed` is satisfied
    /// exactly by a regenerated window. The provider receipts carry clocks it
    /// does not author.
    #[test]
    fn the_campaign_window_must_contain_the_providers_own_clocks() {
        // The codex session id is `YYYY-MM-DDTHH-MM-SS-<uuid>`, written by the
        // provider's CLI. 2026-09-10T08:09:17Z is 1789027757.
        assert_eq!(
            codex_session_epoch("2026-09-10T08-09-17-01a08af6-2a9a-7ba0-963d-86ecd208b4d7"),
            Some(1_789_027_757)
        );
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(codex_session_epoch("not-a-timestamp"), None);
        assert_eq!(
            codex_session_epoch("2026-13-10T08-09-17-uuid"),
            None,
            "month 13 is not a month"
        );
    }

    /// Blind pass 1 at `6b36bbb9` (A06): crash evidence carried digests, which
    /// prove recovery is repeatable and residue-free and cannot say WHICH
    /// state it repeatably reached. The only terminal expectation was one
    /// string the scenario the harness runs declares for itself.
    #[test]
    fn recovery_must_reach_the_state_the_protocol_requires() {
        let path = repo_root().join("conformance/haqp/evidence/crash.json");
        let evidence: CrashEvidence =
            serde_json::from_slice(&fs::read(&path).expect("read crash evidence"))
                .expect("parse crash evidence");
        verify_crash_terminal_states(&evidence).expect("the committed recovery is correct");

        // A recovery that rolls forward from before the intent was durable has
        // invented work; one that aborts after it was durable has lost it.
        let mut invented = evidence.clone();
        for row in &mut invented.boundaries {
            if row.boundary == "ilrp/before_intent_commit" {
                for pair in &mut row.recovery_pairs {
                    pair.first_terminals = vec!["Committed".to_owned()];
                }
            }
        }
        let err = verify_crash_terminal_states(&invented)
            .expect_err("nothing durable was recorded, so nothing may be committed");
        assert!(err.to_string().contains("the protocol requires"), "{err}");

        let mut lost = evidence.clone();
        for row in &mut lost.boundaries {
            if row.boundary == "ilrp/after_ack" {
                for pair in &mut row.recovery_pairs {
                    pair.first_terminals = vec!["Aborted".to_owned()];
                }
            }
        }
        let err = verify_crash_terminal_states(&lost)
            .expect_err("an acknowledged intent may not be abandoned");
        assert!(err.to_string().contains("the protocol requires"), "{err}");

        // A boundary the table does not name cannot be judged at all.
        let mut renamed = evidence;
        renamed.boundaries[0].boundary = "ilrp/invented".to_owned();
        let err = verify_crash_terminal_states(&renamed).expect_err("an unnamed boundary");
        assert!(
            err.to_string().contains("no declared terminal expectation"),
            "{err}"
        );
    }

    /// Review of `8cb5f3b4`: the protocol table and the packet's registry are
    /// two lists of the same boundaries, and two lists drift. The names the
    /// table uses are `IntentState::name`, not a Debug derive, so a renamed
    /// variant is a compile-time decision rather than silent evidence drift.
    #[test]
    fn the_protocol_table_names_exactly_the_declared_boundaries() {
        let packet = packet_from_repo();
        let declared = packet
            .crash_boundaries
            .iter()
            .map(|row| row.boundary.clone())
            .collect::<BTreeSet<_>>();
        let tabled = CRASH_TERMINAL_EXPECTATION
            .iter()
            .map(|(boundary, _)| (*boundary).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            tabled, declared,
            "the protocol's terminal table and the packet's crash boundaries must name the \
             same set; adding a boundary is a decision about what recovery must reach"
        );
        for (_, states) in CRASH_TERMINAL_EXPECTATION {
            for state in states {
                assert!(
                    [
                        liminal_jurisdiction::IntentState::Prepared,
                        liminal_jurisdiction::IntentState::Applying,
                        liminal_jurisdiction::IntentState::ExternalApplied,
                        liminal_jurisdiction::IntentState::Finalizing,
                        liminal_jurisdiction::IntentState::Committed,
                        liminal_jurisdiction::IntentState::NeedsReview,
                        liminal_jurisdiction::IntentState::Aborted,
                    ]
                    .iter()
                    .any(|known| known.name() == *state),
                    "{state:?} is not an IntentState; the table would never match"
                );
            }
        }
    }

    /// Blind pass 1 at `0a0b4b4b` (A04, A09): a `|` in a pattern is an
    /// alternative and inverting it does not compile; and the ratification row
    /// was searched for across the whole document rather than read from the
    /// authority table.
    #[test]
    fn a_pattern_bar_is_not_an_operator_and_a_row_is_read_from_its_table() {
        assert!(
            !anchor_supports_operator(
                "predicate-inversion",
                "BasisComponent::ObjectContent { .. } | BasisComponent::GitCommit { .. } => {"
            ),
            "inverting a match arm's bar yields `A & B`, which is not a pattern"
        );
        assert!(
            !anchor_supports_operator(
                "predicate-inversion",
                "| BasisComponent::Other { .. } => 1,"
            ),
            "a leading bar is an alternative too"
        );
        assert!(
            anchor_supports_operator("predicate-inversion", "NodeFlags(self.0 | other.0)"),
            "a bar in an expression is bitwise or"
        );

        for word in ["approved", "accepted", "granted", "signed"] {
            assert!(
                VERDICT_VOCABULARY.contains(&word),
                "{word} asserts a decision the flip has not made"
            );
        }

        let text = fs::read_to_string(repo_root().join("docs/execution/phase1-suite-review.md"))
            .expect("read the review markdown");
        let packet = packet_from_repo();
        verify_markdown_surface_text(&text, &packet).expect("the committed surface holds");
        // The phrase in prose is not the row: it must come from the table.
        let doctored = text.replace(
            "| ratification decision | unratified",
            "| ratification decision | approved by the qualifier",
        ) + "\n\nratification decision | unratified\n";
        let err = verify_markdown_surface_text(&doctored, &packet)
            .expect_err("a prose mention is not the authority table's row");
        assert!(err.to_string().contains("records ratification"), "{err}");

        // Blind pass 1 at `ec045588` (A10): the qualification lane's table was
        // bound by nothing, so a fabricated command inventory survived the
        // packet digest and every status check — none of it is a verdict word.
        let fabricated = text.replace(
            "| exact commands, in order | NOT_RUN |",
            "| exact commands, in order | `just ci`, `just haq-lane run-1` |",
        );
        assert_ne!(fabricated, text, "the lane row must exist to be doctored");
        let err = verify_markdown_surface_text(&fabricated, &packet)
            .expect_err("a command inventory the lane never produced");
        assert!(
            err.to_string().contains("while the packet is unqualified"),
            "{err}"
        );
    }

    /// Blind pass 1 at `0a0b4b4b` (A02): a macro is an item the oracle can
    /// reach, and the closure followed functions only — so the production call
    /// hid one expansion away.
    #[test]
    fn the_oracle_closure_follows_a_macro_definition() {
        let text = concat!(
            "macro_rules! reach {\n    () => {\n        forbidden_path(1)\n    };\n}\n\n",
            "fn oracle(x: u8) -> u8 {\n    reach!()\n}\n\n",
            "macro_rules! unused {\n    () => {\n        other_thing()\n    };\n}\n",
        );
        let body = oracle_reachable_body(text, "oracle");
        assert!(
            body.contains("forbidden_path("),
            "a macro the oracle invokes is part of the oracle: {body}"
        );
        assert!(
            !body.contains("other_thing("),
            "a macro it never invokes is not"
        );
    }

    /// Blind pass 1 at `0a0b4b4b` (A07): the fuzz any-touch rule fired on the
    /// lexical fragments `heldout` and `conformance/corpora`, and F-64's inode
    /// comparison guarded writes only — so a fuzz binary READING a hard link
    /// to a corpus file under any other name was accepted.
    #[test]
    fn a_fuzz_read_of_a_corpus_inode_is_a_touch() {
        let scratch = liminal_scratch::ScratchDir::new("haq-fuzz-read").expect("scratch");
        let root = scratch.path().to_owned();
        let corpus = root.join("conformance/corpora/heldout");
        fs::create_dir_all(corpus.as_std_path()).expect("mkdir");
        let locked = corpus.join("case.txt");
        fs::write(locked.as_std_path(), b"held out").expect("write");
        let alias = root.join("workspace-input.bin");
        fs::hard_link(locked.as_std_path(), alias.as_std_path()).expect("hard link");
        let ordinary = root.join("ordinary.txt");
        fs::write(ordinary.as_std_path(), b"mine").expect("write");

        let touched = |path: &Utf8Path| {
            corpus_identity_touched(&root, &BTreeSet::from([path.to_string()])).expect("check")
        };
        assert!(!touched(&ordinary), "an ordinary read is not a touch");
        assert!(
            touched(&alias),
            "reading a second name for a corpus inode is reading the corpus"
        );
    }

    /// Blind pass 1 at `0a0b4b4b` (A10): cross-pass reproduction required the
    /// two reports to be byte-identical, so two reviewers who cannot see each
    /// other's work had to write the same sentence — the one thing blinding
    /// exists to prevent.
    #[test]
    fn two_blinded_reports_of_one_defect_need_not_share_a_sentence() {
        let attempt = |what: &str, observed: &str| ReviewRecordAttempt {
            id: "A1".to_owned(),
            attack_class: "vacuity".to_owned(),
            target: "crates/liminal-xtask/src/haq.rs:1".to_owned(),
            attempt: what.to_owned(),
            observed_result: observed.to_owned(),
            independently_reproduced: true,
            classification: "verified_defect".to_owned(),
            resolved: false,
            resolution: None,
        };
        let codex = attempt(
            "falsify the content survival check",
            "content survival compares alphanumeric runs, so punctuation content is dropped unseen",
        );
        let mimo = attempt(
            "attack content survival",
            "the survival check compares alphanumeric runs only; dropped punctuation content is unseen",
        );
        assert!(
            reports_the_same_defect(&codex, &mimo),
            "different words, same defect"
        );

        let unrelated = attempt(
            "falsify the campaign clock",
            "the recorded window is self-consistent and binds no external timestamp",
        );
        assert!(
            !reports_the_same_defect(&codex, &unrelated),
            "the same coordinate is not the same defect"
        );

        // Blind pass 1 at `ec045588` (A02): the coordinate's own words are not
        // evidence that two reports are about one defect — both reports name
        // it, so `crates`, `liminal`, `xtask` and `haq` arrived free.
        let path_words = attempt(
            "probe crates liminal xtask haq",
            "the crates liminal xtask haq verifier accepted an unrelated thing",
        );
        let other_path_words = attempt(
            "probe crates liminal xtask haq again",
            "the crates liminal xtask haq verifier missed a different thing",
        );
        assert!(
            !reports_the_same_defect(&path_words, &other_path_words),
            "path tokens and campaign boilerplate are not a shared report"
        );
    }

    /// Blind pass 1 at `0a0b4b4b` (A01): membership was checked and order was
    /// not, so a formatter that scrambled a document's words kept every word,
    /// every mark, non-emptiness and idempotence — and lost the meaning.
    #[test]
    fn formatting_may_not_move_one_word_past_another() {
        let source = "alpha beta gamma\n";
        let coarse = liminal_cst::coarse_parse(source);
        let scrambled = "gamma beta alpha\n";
        let err =
            independent_oracle_source_cst(source, source, scrambled, scrambled, "beta", &coarse)
                .expect_err("every word is present and the meaning is gone");
        assert!(err.to_string().contains("reordered"), "{err}");

        // Adding structural words is the explicit dialect working, and an
        // escape denotes the character it stands for rather than a letter.
        let wrapped =
            "#!liminal-explicit-v1\nnode paragraph {\n  literal \"alpha\\u0001beta\";\n}\n";
        independent_oracle_source_cst(
            "alpha\u{1}beta\n",
            "alpha\u{1}beta\n",
            wrapped,
            wrapped,
            "alpha",
            &liminal_cst::coarse_parse("alpha\u{1}beta\n"),
        )
        .expect("wrapping and escaping preserve order");
    }

    /// Blind pass 1 at `ec045588` (A07): the brace reader saw one line, so a
    /// raw string or block comment spanning lines had its braces counted as
    /// structure — enough to hold a `#[cfg(test)]` skip open past the module.
    #[test]
    fn a_literal_that_spans_lines_is_still_text() {
        let text = concat!(
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    const FIXTURE: &str = r#\"\n",
            "        { { { unbalanced braces inside a raw string\n",
            "    \"#;\n",
            "}\n",
            "\n",
            "fn production_after_the_module() {\n",
            "    let x = 1;\n",
            "}\n",
        );
        assert!(
            line_is_inside_test_code(text, 4),
            "the raw string's content is inside the test module"
        );
        assert!(
            !line_is_inside_test_code(text, 9),
            "production after the module is not test code; unbalanced braces in a \
             multi-line literal must not hold the skip open"
        );

        let mut scanner = SourceScanner::default();
        assert_eq!(scanner.structural("let s = r#\"{{{\"#; {"), "let s = ; {");
        let mut spanning = SourceScanner::default();
        assert_eq!(spanning.structural("let s = r#\"{"), "let s = ");
        assert_eq!(spanning.structural("still inside } {"), "");
        assert_eq!(spanning.structural("\"#; }"), "; }");
        let mut commented = SourceScanner::default();
        assert_eq!(commented.structural("a /* { "), "a ");
        assert_eq!(commented.structural(" } */ b {"), " b {");
    }

    /// Blind pass 1 at `ec045588` (A04, A05): two operators whose preconditions
    /// were looser than the patch contract, so a mutant could declare a
    /// mutation no patch could ever implement.
    #[test]
    fn an_operator_reaches_only_where_a_patch_could_implement_it() {
        assert!(
            !anchor_supports_operator(
                "oracle-short-circuit",
                "let observed = observe(&self.target)?.ok_or_else(|| {"
            ),
            "a closure's empty parameter list is not a disjunction"
        );
        assert!(anchor_supports_operator(
            "oracle-short-circuit",
            "if c != b && m != c {"
        ));
        assert!(
            anchor_supports_operator("oracle-short-circuit", "if bl.len() != ol.len() || x {"),
            "a disjunction has an operand to its left"
        );
        assert!(
            !anchor_supports_operator(
                "wrong-holder-selection",
                "let mut txn = self.store.begin()?;"
            ),
            "reading the only store there is cannot be redirected"
        );
        for anchor in WRONG_HOLDER_ANCHORS {
            assert!(anchor_supports_operator("wrong-holder-selection", anchor));
        }
    }

    /// Blind pass 1 at `ec045588` (A01): every generated source held at most
    /// one coarse block, so the hash relation had nothing to relate and a
    /// constant hash passed 100,000 cases.
    #[test]
    fn the_generated_corpus_reaches_more_than_one_coarse_block() {
        // `boundary-offset` renders two blocks with identical text, which is
        // the arm a constant hash satisfies and a text-blind one does not.
        let source = "alpha\n\nalpha\n";
        let coarse = liminal_cst::coarse_parse(source);
        assert!(coarse.blocks.len() >= 2, "a blank line makes two blocks");
        let texts: Vec<&str> = coarse
            .blocks
            .iter()
            .map(|block| {
                let start = usize::try_from(block.range.start).expect("fits");
                let end = usize::try_from(block.range.end).expect("fits");
                &source[start..end]
            })
            .collect();
        assert_eq!(texts.len(), 2);
        assert_eq!(
            texts[0].trim(),
            texts[1].trim(),
            "the two carry the same text"
        );
        verify_coarse_scan(source, &coarse).expect("equal text hashing alike is accepted");
    }

    /// Blind pass 1 at `ec045588` (A06): `chmod` names a path and was listed;
    /// `fchmod` names a descriptor and was not, so changing the locked
    /// corpus's mode through an open descriptor produced no write candidate.
    #[test]
    fn a_write_through_a_descriptor_is_still_a_write() {
        let trace = concat!(
            "100 openat(AT_FDCWD, \"conformance/corpora/heldout/case.txt\", O_RDONLY) = 7\n",
            "100 fchmod(7, 0666)                     = 0\n",
            "100 +++ exited with 0 +++\n",
        );
        let scan = scan_scope_trace(std::io::Cursor::new(trace.as_bytes())).expect("scan");
        assert!(
            scan.locked_write_candidates
                .iter()
                .any(|path| path.contains("heldout/case.txt")),
            "the descriptor resolves to the file it names: {:?}",
            scan.locked_write_candidates
        );

        // A descriptor never opened resolves to nothing rather than to
        // something else.
        let orphan = "100 fchmod(99, 0666)                    = 0\n100 +++ exited with 0 +++\n";
        let scan = scan_scope_trace(std::io::Cursor::new(orphan.as_bytes())).expect("scan");
        assert!(scan.locked_write_candidates.is_empty());
    }

    /// AM-17.10's primary killer is chosen by how the operator is observed.
    #[test]
    fn the_derived_primary_matches_the_operators_observation_kind() {
        let packet = packet_from_repo();
        let by_id: BTreeMap<&str, &Test> =
            packet.tests.iter().map(|t| (t.id.as_str(), t)).collect();
        let mut checked = 0usize;
        for mutant in &packet.mutants {
            let observed = MUTANT_OBSERVATION_KIND
                .iter()
                .find(|(operator, _)| *operator == mutant.operator)
                .map(|(_, kind)| *kind)
                .expect("every operator names its observation");
            let defends_with_that_kind = packet.tests.iter().any(|test| {
                test.requirements.contains(&mutant.requirement)
                    && test.evidence.iter().any(|kind| kind == observed)
            });
            if !defends_with_that_kind {
                continue;
            }
            let primary = by_id[mutant.killing_tests[0].as_str()];
            assert!(
                primary.evidence.iter().any(|kind| kind == observed),
                "{}: operator {:?} is observed by {observed}, primary {} is {:?}",
                mutant.id,
                mutant.operator,
                primary.id,
                primary.evidence
            );
            checked += 1;
        }
        assert!(checked > 20, "only {checked} mutants exercised the rule");
    }

    /// Blind pass 1 at 91b54842 (A01): `Rng::word` reaches one- and
    /// two-character tokens, and the survival check skipped them entirely.
    #[test]
    fn a_short_token_must_survive_formatting_too() {
        let source = "alpha q beta\n";
        let coarse = liminal_cst::coarse_parse(source);
        let dropped = "alpha beta\n";
        let err = independent_oracle_source_cst(source, source, dropped, dropped, "q", &coarse)
            .expect_err("a formatter that drops a one-character token loses content");
        assert!(
            err.to_string().contains("dropped the document's content"),
            "refused for the wrong reason: {err}"
        );
        independent_oracle_source_cst(source, source, source, source, "q", &coarse)
            .expect("a formatter that keeps the token is accepted");

        // Blind pass 1 at `6b36bbb9` (A09): a token of punctuation has no
        // alphanumeric run, so losing it raised no missing-word failure.
        let marked = "alpha _ beta\n";
        let marked_coarse = liminal_cst::coarse_parse(marked);
        let stripped = "alpha beta\n";
        let err =
            independent_oracle_source_cst(marked, marked, stripped, stripped, "_", &marked_coarse)
                .expect_err("punctuation content is content");
        assert!(
            err.to_string().contains("punctuation content"),
            "refused for the wrong reason: {err}"
        );
        // A list marker lowered into structure is the formatter working.
        let listed = "- alpha\n";
        let listed_coarse = liminal_cst::coarse_parse(listed);
        independent_oracle_source_cst(listed, listed, "alpha\n", "alpha\n", "-", &listed_coarse)
            .expect("lowering a list marker is not losing content");
    }

    /// Review of `e490671`: braces inside a comment or a string are text, and
    /// a bare `);` terminates a call rather than continuing a signature.
    #[test]
    fn test_code_detection_and_signature_screening_read_only_structure() {
        let text = "fn prod() {\n    let s = \"}}}\"; // }\n}\n#[cfg(test)]\nmod tests {\n    fn t() {\n        assert!(x);\n    }\n}\n";
        assert!(
            !line_is_inside_test_code(text, 2),
            "a string's braces are not structure"
        );
        assert!(
            line_is_inside_test_code(text, 7),
            "the assertion is inside the test module"
        );
        assert!(
            !line_is_inside_test_code(text, 1),
            "production is not test code"
        );

        assert_eq!(
            SourceScanner::default().structural("let s = \"}\"; // {"),
            "let s = ; "
        );
        assert!(
            SourceScanner::default()
                .structural("fn fmt(&self, f: &mut Formatter<'_>) -> Result {")
                .contains('{'),
            "a lifetime is not an unterminated character literal"
        );
        assert!(
            !SourceScanner::default()
                .structural("let brace = '}';")
                .contains('}')
        );
        assert!(
            !SourceScanner::default()
                .structural("let brace = b'{';")
                .contains('{')
        );
        assert!(
            !SourceScanner::default()
                .structural("let esc = '\\'';")
                .contains('\'')
        );
        assert!(
            !SourceScanner::default()
                .structural("let nl = '\\n'; let u = '\\u{7d}';")
                .contains('}')
        );
        assert_eq!(
            SourceScanner::default().structural("let s = r#\"// }\"#; {"),
            "let s = ; {"
        );
        assert!(
            !SourceScanner::default()
                .structural("let s = br\"}\";")
                .contains('}')
        );
        assert!(!anchor_is_mutable_line(") -> Result<Basis, Error> {"));
        assert!(!anchor_is_mutable_line(") {"));
        assert!(
            anchor_is_mutable_line(");"),
            "a call's terminator is behaviour, not a signature"
        );
    }

    /// Blind pass 1 at e09ae5e (A07): a parent that chdir'd into the locked
    /// corpus and then forked left the child unanchored, so the child's
    /// relative write named nothing forbidden and resolved against nothing.
    #[test]
    fn a_forked_child_inherits_the_working_directory_it_was_given() {
        let (_s, root, row) = scope_fixture("canaries");
        fs::create_dir_all(root.join("conformance/corpora/heldout")).expect("mkdir");
        let exit = "4242 +++ exited with 0 +++\n";
        let mut bad = row.clone();
        rewrite_scope_trace(
            &root,
            &mut bad,
            &format!(
                "4242 chdir(\"{root}/conformance/corpora/heldout\") = 0\n\
                 4242 clone(child_stack=NULL, flags=CLONE_CHILD_CLEARTID) = 4243\n\
                 4243 openat(AT_FDCWD, \"x.md\", O_WRONLY|O_CREAT, 0644) = 3\n{exit}"
            ),
        );
        let err = verify_corpus_scope_replays(&root, std::slice::from_ref(&bad), &[])
            .expect_err("a forked child writing into the locked corpus");
        assert!(
            err.to_string().contains("wrote to the locked corpus"),
            "{err}"
        );

        // A descriptor the parent opened is inherited too.
        let mut by_fd = row.clone();
        rewrite_scope_trace(
            &root,
            &mut by_fd,
            &format!(
                "4242 openat(AT_FDCWD, \"{root}/conformance/corpora/heldout\", O_RDONLY|O_DIRECTORY) = 7\n\
                 4242 clone(child_stack=NULL) = 4244\n\
                 4244 openat(7, \"x.md\", O_WRONLY|O_CREAT, 0644) = 8\n{exit}"
            ),
        );
        let err = verify_corpus_scope_replays(&root, std::slice::from_ref(&by_fd), &[])
            .expect_err("a child writing through an inherited directory fd");
        assert!(
            err.to_string().contains("wrote to the locked corpus"),
            "{err}"
        );

        // A clone that failed creates no child to inherit anything.
        assert_eq!(
            clone_child_pid("4242 clone(child_stack=NULL) = -1 EAGAIN"),
            None
        );
        assert_eq!(
            clone_child_pid("4242 clone(child_stack=NULL) = 4243"),
            Some(4243)
        );
        assert_eq!(
            clone_child_pid("4242 openat(AT_FDCWD, \"x\", O_RDONLY) = 3"),
            None
        );
    }

    /// Blind pass 1 at aa00d41 (A07): a `chdir` into the locked corpus made the
    /// relative write that followed nameless, and resolution against the
    /// repository root put it somewhere harmless.
    #[test]
    fn a_write_after_chdir_into_the_locked_corpus_is_refused() {
        let (_s, root, row) = scope_fixture("canaries");
        fs::create_dir_all(root.join("conformance/corpora/heldout")).expect("mkdir");
        let exit = "4242 +++ exited with 0 +++\n";
        let mut bad = row.clone();
        rewrite_scope_trace(
            &root,
            &mut bad,
            &format!(
                "4242 chdir(\"{root}/conformance/corpora/heldout\") = 0\n\
                 4242 openat(AT_FDCWD, \"x.md\", O_WRONLY|O_CREAT, 0644) = 3\n{exit}"
            ),
        );
        let err = verify_corpus_scope_replays(&root, std::slice::from_ref(&bad), &[])
            .expect_err("a relative write after chdir into the corpus");
        assert!(
            err.to_string().contains("wrote to the locked corpus"),
            "{err}"
        );

        // A chdir somewhere harmless leaves the same write harmless.
        let mut fine = row.clone();
        rewrite_scope_trace(
            &root,
            &mut fine,
            &format!(
                "4242 chdir(\"{root}/fuzz\") = 0\n\
                 4242 openat(AT_FDCWD, \"x.md\", O_WRONLY|O_CREAT, 0644) = 3\n{exit}"
            ),
        );
        verify_corpus_scope_replays(&root, std::slice::from_ref(&fine), &[])
            .expect("a write outside the locked corpus is fine");

        // A failed chdir moves nothing.
        let mut failed = row.clone();
        rewrite_scope_trace(
            &root,
            &mut failed,
            &format!(
                "4242 chdir(\"{root}/conformance/corpora/heldout\") = -1 ENOENT (No such file)\n\
                 4242 openat(AT_FDCWD, \"x.md\", O_WRONLY|O_CREAT, 0644) = 3\n{exit}"
            ),
        );
        verify_corpus_scope_replays(&root, std::slice::from_ref(&failed), &[])
            .expect("a chdir that failed moved no process");
    }

    /// Blind pass 1 at aa00d41 (A02): the independence scan matched literal
    /// paths, so an aliased import of the production parser evaded it.
    /// The gate's own oracles, written independently, so a scratch fixture
    /// exercising the query oracle is not refused for the five that live in
    /// this file and are not part of what the fixture is testing.
    fn write_independent_gate_oracles(root: &Utf8Path) {
        let file = root.join("crates/liminal-xtask/src/haq.rs");
        fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
        let mut text = String::new();
        for name in [
            "independent_oracle_source_cst",
            "independent_oracle_interchange",
            "independent_oracle_source_transform",
            "independent_oracle_source_repair",
            "independent_oracle_source_invalidation",
        ] {
            use std::fmt::Write as _;
            writeln!(
                text,
                "fn {name}(input: &str) -> usize {{\n    input.len()\n}}"
            )
            .expect("write to a String");
        }
        fs::write(&file, text).expect("write");
    }

    #[test]
    fn an_aliased_production_call_in_an_oracle_is_refused() {
        let scratch = liminal_scratch::ScratchDir::new("haq-oracle-alias").expect("scratch");
        let root = scratch.path().to_owned();
        write_independent_gate_oracles(&root);
        let file = root.join("crates/liminal-query/src/lib.rs");
        fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
        fs::write(
            &file,
            "use liminal_source::paragraph::parse as p;\n\
             fn incremental_paragraph_oracle(input: &str) -> Vec<u8> {\n    p(input)\n}\n",
        )
        .expect("write");
        let err = verify_oracle_independence(&root).expect_err("an aliased production call");
        assert!(err.to_string().contains("p("), "{err}");
    }

    /// Blind pass 1 at aa00d41 (A10): mutation runs filter by a test's leaf
    /// name, so two tests sharing a leaf let a namesake certify the kill.
    #[test]
    fn a_killing_test_whose_leaf_is_ambiguous_is_refused() {
        let (_scratch, root) = scratch_git_repo("haq-leaf");
        let mut packet = read_packet(&repo_root()).expect("packet");
        packet.tests.truncate(1);
        packet.tests[0].name = "laws::twinned".to_owned();
        packet.mutants.truncate(1);
        packet.mutants[0].killing_tests = vec![packet.tests[0].id.clone()];
        for (index, dir) in ["conformance/tests", "crates/a/src"].iter().enumerate() {
            fs::create_dir_all(root.join(dir)).expect("mkdir");
            fs::write(
                root.join(dir).join(format!("t{index}.rs")),
                "#[test]\nfn twinned() {}\n",
            )
            .expect("write");
        }
        let err = verify_killing_test_leaves_are_unambiguous(&root, &packet)
            .expect_err("two tests share the leaf the run filters on");
        assert!(err.to_string().contains("must be unique"), "{err}");
        fs::remove_file(root.join("crates/a/src/t1.rs")).expect("remove");
        verify_killing_test_leaves_are_unambiguous(&root, &packet).expect("one leaf, one test");
    }

    /// Review of c455cdf: a brace group mangled the alias, and a string
    /// constant containing a digit looked like a threshold.
    #[test]
    fn use_aliases_and_threshold_constants_are_read_precisely() {
        assert_eq!(
            use_aliases("use std::fs::rename as mv;"),
            vec![("rename".to_owned(), "mv".to_owned())]
        );
        assert_eq!(
            use_aliases("use std::fs::{rename as mv, remove_file};"),
            vec![("rename".to_owned(), "mv".to_owned())]
        );
        assert_eq!(
            use_aliases("use liminal_source::paragraph::{parse as p, other as o};"),
            vec![
                ("parse".to_owned(), "p".to_owned()),
                ("other".to_owned(), "o".to_owned())
            ]
        );
        assert_eq!(
            use_aliases("use a::{b::{rename as mv}};"),
            vec![("rename".to_owned(), "mv".to_owned())],
            "a nested group reads the same as a flat one"
        );
        assert!(use_aliases("use std::io::Write as _;").is_empty());
        assert!(use_aliases("let x = 1;").is_empty());

        // A number is a threshold; a string that merely contains one is not.
        assert!(anchor_is_mutable_line("pub const MAX_NESTING: u16 = 256;"));
        assert!(anchor_is_mutable_line(
            "pub const TOMBSTONE: RelationFlags = RelationFlags(1);"
        ));
        assert!(!anchor_is_mutable_line("pub const TAG: &str = \"v2\";"));
        assert!(!anchor_is_mutable_line(
            "pub const REPAIR_STALE_BASIS: &str = \"JUR053\";"
        ));
        assert!(!anchor_is_mutable_line("const ALL: [IntentState; 7] = ["));
    }

    /// Blind pass 1 at aa00d41 (A05): the census matched `fs::rename(`, so a
    /// `use std::fs::rename as mv;` renamed the durable transition out of the
    /// surface. Symbols are matched by name, and an aliasing `use` adds its
    /// alias for that file.
    #[test]
    fn an_aliased_durable_call_is_still_a_durable_transition() {
        let (_scratch, root) = scratch_git_repo("haq-durable-alias");
        let src = root.join("crates/store/src");
        fs::create_dir_all(&src).expect("mkdir");
        fs::write(
            src.join("lib.rs"),
            "use std::fs::{rename as mv, remove_file};\npub fn save() { mv(\"a\", \"b\").unwrap(); }\n",
        )
        .expect("write");
        for args in [
            vec!["add", "-A"],
            vec!["commit", "--quiet", "-m", "aliased rename"],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(&root)
                    .args(&args)
                    .status()
                    .expect("git")
                    .success()
            );
        }
        let sites = durable_transition_sites(&root).expect("census");
        assert_eq!(
            sites.get("crates/store/src/lib.rs").copied(),
            Some(1),
            "the aliased call is the one durable transition: {sites:?}"
        );
    }

    /// Blind pass 1 at 612cbcc (A06): production code after an earlier test
    /// module must still be scanned; the module itself must not be.
    #[test]
    fn the_concurrency_scan_resumes_after_a_test_module() {
        let scratch = liminal_scratch::ScratchDir::new("haq-conc2").expect("scratch");
        let root = scratch.path().to_owned();
        let src = root.join("crates/w/src");
        fs::create_dir_all(&src).expect("mkdir");
        fs::write(
            src.join("lib.rs"),
            "pub fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() { std::thread::spawn(|| {}); }\n}\npub fn b() { std::thread::spawn(|| {}); }\n#[cfg(test)]\nuse std::sync::Mutex;\npub struct S { inner: std::sync::RwLock<u8> }\n",
        )
        .expect("write");
        let scan = scan_concurrency_primitives(&root).expect("scan");
        assert_eq!(
            scan.execution,
            vec!["crates/w/src/lib.rs:6:::spawn".to_owned()]
        );
        assert_eq!(scan.sync, vec!["crates/w/src/lib.rs:9:RwLock<".to_owned()]);
    }

    /// Blind pass 1 at 612cbcc (A07): each accepted interchange category builds
    /// the transaction shape it names, counted independently in the JSON.
    #[test]
    fn interchange_cases_carry_the_shape_their_category_names() {
        let mut rng = Rng(11);
        for (category, nodes, relations) in [
            ("single-node", 1, 0),
            ("single-edge", 2, 1),
            ("dag", 4, 4),
            ("wide", 9, 8),
            ("deep", 12, 11),
            ("large-payload", 1, 0),
        ] {
            let (txn, shape) = interchange_transaction(category, &mut rng, "seed", false);
            assert_eq!(
                (shape.nodes, shape.relations),
                (nodes, relations),
                "{category}"
            );
            let once = serde_json::to_vec(&txn).expect("encodes");
            let decoded: liminal_graph::Transaction =
                serde_json::from_slice(&once).expect("decodes");
            let twice = serde_json::to_vec(&decoded).expect("encodes");
            independent_oracle_interchange(&txn, &decoded, &once, &twice, shape).expect(category);
            let wrong = InterchangeShape {
                nodes: nodes + 1,
                ..shape
            };
            independent_oracle_interchange(&txn, &decoded, &once, &twice, wrong)
                .expect_err("a shape the JSON does not carry");
        }
        let (_, big) = interchange_transaction("large-payload", &mut rng, "seed", false);
        assert!(big.payload_bytes_at_least >= 65_536);
    }

    /// Blind pass 1 at 612cbcc (A10): sixteen seeds of one class no longer
    /// satisfy the manifest; every class must be declared by a seed's name.
    #[test]
    fn a_seed_set_must_span_every_class_by_name() {
        let (_scratch, root) = scratch_git_repo("haq-seeds");
        let dir = root.join("fuzz/corpus/t");
        fs::create_dir_all(&dir).expect("mkdir");
        let commit_all = |message: &str| {
            for args in [vec!["add", "-A"], vec!["commit", "--quiet", "-m", message]] {
                let status = Command::new("git")
                    .current_dir(&root)
                    .args(&args)
                    .status()
                    .expect("git");
                assert!(status.success(), "git {args:?} failed");
            }
        };
        for i in 0..16 {
            fs::write(dir.join(format!("{i:02x}-deadbeef.bin")), [i]).expect("seed");
        }
        commit_all("same-class seeds");
        // Sixteen seeds declaring no class at all: since A03 the valid class
        // is declared rather than defaulted, so it is the first one missing.
        let err = verify_seed_classes(&root, "t", &dir).expect_err("no class declared");
        assert!(err.to_string().contains("no valid seed"), "{err}");
        // Blind pass 1 at `6b36bbb9` (A05): these four used to carry identical
        // bytes, which is the fabricated diversity the finding names — four
        // classes spanned by one seed copied four times. The fixture said so
        // before the gate did.
        for (index, name) in [
            "boundary.bin",
            "truncated.bin",
            "invalid.bin",
            "hostile.bin",
        ]
        .into_iter()
        .enumerate()
        {
            fs::write(dir.join(name), format!("class {index}")).expect("seed");
        }
        // The valid class is declared by name like every other (A03).
        fs::write(dir.join("00-single-token.bin"), b"declared valid").expect("seed");
        commit_all("classed seeds");
        verify_seed_classes(&root, "t", &dir).expect("every class declared");

        // Copying a seed under another class's name adds a name, not a class.
        fs::write(dir.join("hostile-copy.bin"), b"class 0").expect("seed");
        commit_all("a copied seed");
        let err =
            verify_seed_classes(&root, "t", &dir).expect_err("two seeds carrying identical bytes");
        assert!(err.to_string().contains("identical bytes"), "{err}");
        fs::remove_file(dir.join("hostile-copy.bin")).expect("remove");
        commit_all("drop the copy");

        // Blind pass 1 at `0a0b4b4b` (A03): a name that asserts a byte
        // property must exhibit it. Five committed seeds named
        // `12-invalid-utf8.bin` were valid UTF-8 — their intended `0xff 0xfe`
        // had been UTF-8 encoded on the way to disk, which is the one thing
        // that makes those bytes valid.
        fs::write(dir.join("12-invalid-utf8.bin"), "perfectly valid text").expect("seed");
        commit_all("a seed that lies about its bytes");
        let err =
            verify_seed_classes(&root, "t", &dir).expect_err("a valid payload named invalid-utf8");
        assert!(err.to_string().contains("and its bytes are not"), "{err}");
        fs::write(dir.join("12-invalid-utf8.bin"), [0xff, 0xfe, b'x']).expect("seed");
        commit_all("a seed that tells the truth");
        verify_seed_classes(&root, "t", &dir).expect("bytes matching the name");

        // Blind pass 1 at `ec045588` (A03): `valid` was whatever matched no
        // negative token, so a seed under an unrecognised name witnessed the
        // well-formed class by default. It must claim the class by name.
        fs::remove_file(dir.join("00-single-token.bin")).expect("remove the valid seed");
        fs::write(dir.join("mystery.bin"), b"unrecognised name").expect("seed");
        commit_all("no seed declares the valid class");
        let err = verify_seed_classes(&root, "t", &dir).expect_err("no declared valid seed");
        assert!(err.to_string().contains("no valid seed"), "{err}");
        fs::write(dir.join("01-single-token.bin"), b"declared valid").expect("seed");
        commit_all("a seed that declares it");
        verify_seed_classes(&root, "t", &dir).expect("every class declared by name");
    }

    /// Blind pass 1 at f360e90 (A03): equal length let a duplicated step
    /// stand in for a missing one.
    #[test]
    fn an_order_that_duplicates_one_step_for_another_is_refused() {
        let mut rng = Rng(9);
        let ids: Vec<liminal_id::RepairStepId> = (0..3)
            .map(|_| liminal_id::RepairStepId::from_uuid(rng.uuid()))
            .collect();
        let faithful = vec![ids[0], ids[1], ids[2]];
        independent_oracle_source_repair(&faithful, &ids, &[(0, 1)], &faithful).expect("faithful");
        let duplicated = vec![ids[0], ids[1], ids[1]];
        let err = independent_oracle_source_repair(&duplicated, &ids, &[(0, 1)], &duplicated)
            .expect_err("B twice, C never");
        assert!(
            err.to_string().contains("dropped, duplicated or invented"),
            "{err}"
        );
    }

    /// Blind pass 1 at f360e90 (A01): repair categories build what they name
    /// and the refusal categories are refused by the ordering itself.
    #[test]
    fn repair_cases_carry_the_shape_their_category_names() {
        let mut rng = Rng(3);
        let mut seen = BTreeMap::new();
        for _ in 0..4_000 {
            let case = case_repair(&mut rng).expect("total");
            let (category, kind) = match &case {
                Case::Accepted { category, .. } => (*category, "accepted"),
                Case::Negative { category, .. } => (*category, "negative"),
                Case::Discarded { category, .. } => (*category, "discarded"),
            };
            seen.entry((category, kind)).or_insert(0usize);
            *seen.get_mut(&(category, kind)).expect("just inserted") += 1;
        }
        for category in ["missing-step", "truncated-intent", "cycle"] {
            assert!(
                seen.contains_key(&(category, "negative")),
                "{category} must be refused: {seen:?}"
            );
            assert!(
                !seen.contains_key(&(category, "accepted")),
                "{category} must never be accepted"
            );
        }
        for category in [
            "graph-step",
            "two-step",
            "duplicate-ack",
            "poststate",
            "hostile",
        ] {
            assert!(
                seen.contains_key(&(category, "accepted")),
                "{category} must be ordered: {seen:?}"
            );
        }
    }

    /// Blind pass 1 at f360e90 (A02): a renamed published field survives a
    /// symmetric serde round trip; the closed key set does not let it.
    #[test]
    fn the_interchange_oracle_pins_the_published_field_names() {
        let mut rng = Rng(5);
        let (txn, shape) = interchange_transaction("single-edge", &mut rng, "seed", false);
        let once = serde_json::to_vec(&txn).expect("encodes");
        let renamed = String::from_utf8(once.clone())
            .expect("utf8")
            .replacen("\"revision\"", "\"rev\"", 1)
            .into_bytes();
        let err = independent_oracle_interchange(&txn, &txn, &renamed, &renamed, shape)
            .expect_err("a renamed field");
        assert!(err.to_string().contains("published schema"), "{err}");
    }

    /// Blind pass 1 at f360e90 (A04): a re-export is a declaration, not a
    /// line an operator can mutate.
    #[test]
    fn a_re_export_is_not_a_mutable_anchor() {
        assert!(!anchor_is_mutable_line(
            "pub use view::{SourceBasis, SourceLoadError};"
        ));
        assert!(!anchor_is_mutable_line("pub(crate) use crate::x::Y;"));
        assert!(anchor_is_mutable_line(
            "if actual_hash != basis.content_hash {"
        ));
    }

    /// Blind pass 1 at f360e90 (A06): an aliased thread module spawns all the same.
    #[test]
    fn an_aliased_spawn_is_concurrent_execution() {
        let scratch = liminal_scratch::ScratchDir::new("haq-conc3").expect("scratch");
        let root = scratch.path().to_owned();
        let src = root.join("crates/w/src");
        fs::create_dir_all(&src).expect("mkdir");
        fs::write(
            src.join("lib.rs"),
            "use std::thread as th;\npub fn go() { th::spawn(|| {}); }\n",
        )
        .expect("write");
        let scan = scan_concurrency_primitives(&root).expect("scan");
        assert_eq!(
            scan.execution,
            vec!["crates/w/src/lib.rs:2:::spawn".to_owned()]
        );
    }

    /// Blind pass 1 at f360e90 (A07): a write through a directory fd opened on
    /// the locked corpus names no fragment; the fd is remembered.
    #[test]
    fn a_write_through_a_locked_directory_fd_is_refused() {
        let (_s, root, row) = scope_fixture("canaries");
        fs::create_dir_all(root.join("conformance/corpora/heldout")).expect("mkdir");
        let exit = "4242 +++ exited with 0 +++\n";
        let mut bad = row.clone();
        rewrite_scope_trace(
            &root,
            &mut bad,
            &format!(
                "4242 openat(AT_FDCWD, \"{root}/conformance/corpora/heldout\", O_RDONLY|O_DIRECTORY) = 7\n\
                 4242 openat(7, \"x.md\", O_WRONLY|O_CREAT, 0644) = 8\n{exit}"
            ),
        );
        let err = verify_corpus_scope_replays(&root, std::slice::from_ref(&bad), &[])
            .expect_err("a dirfd-relative write into the locked corpus");
        assert!(
            err.to_string().contains("wrote to the locked corpus"),
            "{err}"
        );
        // The unfinished/resumed split reaches the same verdict.
        let mut split = row.clone();
        rewrite_scope_trace(
            &root,
            &mut split,
            &format!(
                "4242 openat(AT_FDCWD, \"{root}/conformance/corpora/heldout\", O_RDONLY|O_DIRECTORY <unfinished ...>\n\
                 4243 openat(AT_FDCWD, \"{root}/Cargo.toml\", O_RDONLY) = 3\n\
                 4242 <... openat resumed>) = 7\n\
                 4242 openat(7, \"x.md\", O_WRONLY|O_CREAT, 0644) = 8\n{exit}"
            ),
        );
        let err = verify_corpus_scope_replays(&root, std::slice::from_ref(&split), &[])
            .expect_err("the same write across an unfinished open");
        assert!(
            err.to_string().contains("wrote to the locked corpus"),
            "{err}"
        );
    }

    /// Blind pass 1 at f360e90 (A08/A09): an undeclared waiver inside an
    /// attempt is refused, and the record's raw digest must be the retained
    /// answer's.
    #[test]
    fn a_review_record_binds_its_retained_raw_answer_and_refuses_waivers() {
        // Built from the fixture, not from committed evidence: a record from
        // an earlier lane lacks today's required fields, so parsing one would
        // fail for a reason this test is not about (review of 2bcc6cb).
        let mut value =
            serde_json::to_value(review_record(1, "openai", "codex")).expect("fixture record");
        value["attempts"][0]["waiver"] = serde_json::json!("*");
        let err = serde_json::from_value::<ReviewRecord>(value.clone())
            .expect_err("a waiver field inside an attempt");
        assert!(err.to_string().contains("unknown field"), "{err}");
        // ...and a record with no provider receipt at all is refused outright.
        value["attempts"][0]
            .as_object_mut()
            .expect("attempt object")
            .remove("waiver");
        value
            .as_object_mut()
            .expect("record object")
            .remove("provider_receipt");
        let err = serde_json::from_value::<ReviewRecord>(value).expect_err("no receipt");
        assert!(err.to_string().contains("provider_receipt"), "{err}");
        let scratch = liminal_scratch::ScratchDir::new("haq-raw").expect("scratch");
        let root = scratch.path().to_owned();
        let record = root.join("r.json");
        fs::write(&record, "{}").expect("record");
        let err = verify_review_raw_response(&root, &record, &"a".repeat(64))
            .expect_err("no retained answer");
        assert!(err.to_string().contains("retained"), "{err}");
        fs::write(root.join("r.raw.txt"), "answer").expect("raw");
        let digest = format!("{:x}", Sha256::digest(b"answer"));
        verify_review_raw_response(&root, &record, &digest).expect("the retained answer hashes");
        let err = verify_review_raw_response(&root, &record, &"a".repeat(64))
            .expect_err("a substituted digest");
        assert!(err.to_string().contains("raw_response_sha256"), "{err}");
    }

    /// Blind pass 1 at 78c8f9b (A12): a relative write climbing to the locked
    /// corpus from a subdirectory refuses on its name alone.
    #[test]
    fn a_relative_write_that_climbs_into_the_locked_corpus_is_refused() {
        let (_s, root, row) = scope_fixture("canaries");
        let exit = "4242 +++ exited with 0 +++\n";
        let mut bad = row.clone();
        rewrite_scope_trace(
            &root,
            &mut bad,
            &format!(
                "4242 openat(AT_FDCWD, \"../conformance/corpora/heldout/x.md\", O_WRONLY|O_CREAT, 0644) = 3\n{exit}"
            ),
        );
        let err = verify_corpus_scope_replays(&root, std::slice::from_ref(&bad), &[])
            .expect_err("a climbing relative write");
        assert!(
            err.to_string().contains("wrote to the locked corpus"),
            "{err}"
        );
    }

    /// Blind pass 1 at 78c8f9b (A02): the independent oracle must not reach
    /// the production path it judges, and the tripwire says which symbol.
    #[test]
    fn an_oracle_that_calls_the_production_parser_is_refused() {
        verify_oracle_independence(&repo_root()).expect("the committed oracle is independent");
        let scratch = liminal_scratch::ScratchDir::new("haq-oracle").expect("scratch");
        let root = scratch.path().to_owned();
        write_independent_gate_oracles(&root);
        let file = root.join("crates/liminal-query/src/lib.rs");
        fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
        fs::write(&file, "fn incremental_paragraph_oracle(input: &str) -> Vec<u8> {\n    paragraph::parse(input)\n}\n").expect("write");
        let err = verify_oracle_independence(&root).expect_err("coupled oracle");
        assert!(err.to_string().contains("paragraph::parse"), "{err}");
    }

    /// Blind pass 1 at aa00d41 (A09): nothing outside this runner showed that
    /// two distinct reviewers answered. A receipt must be the backend's own
    /// shape, and a codex transcript must be retained and hash to its claim.
    #[test]
    fn a_review_record_carries_its_provider_receipt() {
        let (_scratch, path) = fixture_record_path();
        let record = review_record(1, "openai", "codex");
        verify_review_provider_receipt(&record, &path).expect("the fixture receipt verifies");

        let mut wrong_backend = record.clone();
        wrong_backend.reviewer.backend = "mimo-direct".to_owned();
        let err = verify_review_provider_receipt(&wrong_backend, &path)
            .expect_err("a receipt from another backend");
        assert!(err.to_string().contains("backend"), "{err}");

        let mut forged = record.clone();
        forged.provider_receipt.transcript_sha256 = "a".repeat(64);
        let err = verify_review_provider_receipt(&forged, &path)
            .expect_err("a digest the retained transcript does not have");
        assert!(err.to_string().contains("transcript_sha256"), "{err}");

        let mut truncated = record.clone();
        truncated.provider_receipt.transcript_bytes += 1;
        let err = verify_review_provider_receipt(&truncated, &path)
            .expect_err("a length the transcript does not have");
        assert!(err.to_string().contains("bytes"), "{err}");

        // No transcript retained at all.
        fs::remove_file(path.with_extension("session.jsonl")).expect("remove");
        let err = verify_review_provider_receipt(&record, &path).expect_err("no transcript");
        assert!(
            err.to_string().contains("retained beside its record"),
            "{err}"
        );

        // A MiMo receipt must carry an id, a model, a clock and billed usage.
        let mimo = |usage: &str, created: i64| -> ReviewRecord {
            let mut row = review_record(2, "mimo", "mimo-direct");
            row.provider_receipt = serde_json::from_str(&format!(
                r#"{{"backend":"mimo-direct","created":{created},"provider_model":"mimo-v2.5-pro","response_id":"chatcmpl-x","usage":{usage}}}"#
            ))
            .expect("receipt parses");
            row
        };
        verify_review_provider_receipt(
            &mimo(
                r#"{"prompt_tokens":3,"completion_tokens":9,"total_tokens":12}"#,
                1,
            ),
            &path,
        )
        .expect("a complete mimo receipt");
        let err = verify_review_provider_receipt(
            &mimo(
                r#"{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}"#,
                1,
            ),
            &path,
        )
        .expect_err("a receipt that bills nothing");
        assert!(err.to_string().contains("bills nothing"), "{err}");
        let err = verify_review_provider_receipt(
            &mimo(
                r#"{"prompt_tokens":3,"completion_tokens":9,"total_tokens":12}"#,
                0,
            ),
            &path,
        )
        .expect_err("a receipt without the provider's clock");
        assert!(err.to_string().contains("clock"), "{err}");

        // A backend that produces no receipt at all cannot qualify.
        let mut lamu = review_record(1, "openai", "lamu");
        lamu.provider_receipt =
            serde_json::from_str(r#"{"backend":"lamu"}"#).expect("receipt parses");
        let err = verify_review_provider_receipt(&lamu, &path).expect_err("no receipt");
        assert!(err.to_string().contains("outside this runner"), "{err}");
    }

    /// Blind pass 1 at 78c8f9b (A11): editing an attempt under an intact
    /// binding must break the binding.
    #[test]
    fn the_integrity_binding_covers_the_structured_claims() {
        let scratch = liminal_scratch::ScratchDir::new("haq-claims").expect("scratch");
        let a = scratch.path().join("a.json");
        let b = scratch.path().join("b.json");
        fs::write(&a, r#"{"attempts":[{"id":"A1","observed_result":"x"}],"findings":[],"independently_reproduced":[]}"#).expect("a");
        fs::write(&b, r#"{"attempts":[{"id":"A1","observed_result":"y"}],"findings":[],"independently_reproduced":[]}"#).expect("b");
        assert_ne!(
            review_claims_sha256(&a).expect("a"),
            review_claims_sha256(&b).expect("b")
        );
        let c = scratch.path().join("c.json");
        fs::write(&c, "{ \"independently_reproduced\": [], \"findings\": [],\n \"attempts\": [ {\"observed_result\":\"x\", \"id\":\"A1\"} ] }").expect("c");
        assert_eq!(
            review_claims_sha256(&a).expect("a"),
            review_claims_sha256(&c).expect("c")
        );
    }

    /// Blind pass 1 at aa00d41 (A08): a ruling matched on class and file alone
    /// cleared an unrelated claim. It must answer the claim it was written for
    /// and no other.
    #[test]
    fn a_ruling_clears_only_the_claim_it_answers() {
        let (_scratch, root) = scratch_git_repo("haq-ruling-claim");
        fs::create_dir_all(root.join("docs/execution/rulings")).expect("mkdir");
        fs::create_dir_all(root.join("conformance/haqp")).expect("mkdir");
        fs::write(root.join(RULING_SIGNERS), "brian ssh-ed25519 AAAA\n").expect("signers");
        fs::write(
            root.join("docs/execution/rulings/R-001.md"),
            "---\nid: R-001\nattack_class: corpus leakage\ntarget: crates/liminal-xtask/src/haq.rs\nclaim_requires: read access\nclaim_excludes: chdir\nstatus: ruled\n---\n\nReads are by design.\n",
        )
        .expect("ruling");
        let ruling = &rulings(&root).expect("parse")[0];
        assert_eq!(ruling.claim_requires, vec!["read access".to_owned()]);
        assert_eq!(ruling.claim_excludes, vec!["chdir".to_owned()]);
        let answered = "unauthorized read access remains accepted";
        let unrelated = "chdir state is not tracked; the later relative write resolves incorrectly";
        let record = |prose: &str| {
            format!(
                r#"{{"attempts":[{{"id":"A7","attack_class":"corpus leakage","target":"crates/liminal-xtask/src/haq.rs:1","attempt":"x","observed_result":"{prose}","classification":"verified_defect","independently_reproduced":true,"resolved":false}}],"findings":[],"independently_reproduced":[]}}"#
            )
        };
        // Without a verifying signature neither is cleared; the claim filter is
        // proved on the ruling's own predicate.
        let prose_matches = |prose: &str| {
            let lower = prose.to_ascii_lowercase();
            ruling
                .claim_requires
                .iter()
                .all(|phrase| lower.contains(phrase.as_str()))
                && !ruling
                    .claim_excludes
                    .iter()
                    .any(|phrase| lower.contains(phrase.as_str()))
        };
        assert!(prose_matches(answered), "the claim it answers");
        assert!(
            !prose_matches(unrelated),
            "an unrelated claim in the same class and file"
        );
        // A08: the required phrase occurs inside a sentence that is about
        // something else; an excluded phrase says so.
        assert!(
            !prose_matches("read access is fine; the chdir state is not tracked"),
            "a claim that mentions the ruled phrase while being about something else"
        );
        let path = root.join("r.json");
        fs::write(&path, record(unrelated)).expect("record");
        assert_eq!(
            effective_unresolved_findings_repo(&root, &path).expect("count"),
            1,
            "an unrelated claim stands"
        );
        // An empty claim_requires is refused outright.
        fs::write(
            root.join("docs/execution/rulings/R-001.md"),
            "---\nid: R-001\nattack_class: corpus leakage\ntarget: crates/liminal-xtask/src/haq.rs\nclaim_requires:  \nstatus: ruled\n---\n\nToo broad.\n",
        )
        .expect("ruling");
        let err = rulings(&root).expect_err("an empty claim filter");
        assert!(
            err.to_string()
                .contains("clears defects it was never shown"),
            "{err}"
        );
    }

    /// Rulings (grilling decision 6): a draft ruling clears nothing; a ruling
    /// only counts when its commit verifies against the pinned signers.
    #[test]
    fn an_unsigned_ruling_clears_nothing() {
        let (_scratch, root) = scratch_git_repo("haq-ruling");
        fs::create_dir_all(root.join("docs/execution/rulings")).expect("mkdir");
        fs::write(
            root.join("docs/execution/rulings/R-999-test.md"),
                                    "---\nid: R-999\nattack_class: corpus leakage\ntarget: crates/liminal-xtask/src/haq.rs\nclaim_requires: read access\nstatus: draft\n---\n\nA draft.\n",
        )
        .expect("ruling");
        let parsed = rulings(&root).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert!(
            !ruling_in_force(&root, &parsed[0]).expect("check"),
            "a draft is not in force"
        );
        // Marked ruled but committed unsigned: still not in force.
        fs::create_dir_all(root.join("conformance/haqp")).expect("mkdir");
        fs::write(root.join(RULING_SIGNERS), "brian ssh-ed25519 AAAA\n").expect("signers");
        fs::write(
            root.join("docs/execution/rulings/R-999-test.md"),
                        "---\nid: R-999\nattack_class: corpus leakage\ntarget: crates/liminal-xtask/src/haq.rs\nclaim_requires: read access\nstatus: ruled\n---\n\nRuled, unsigned.\n",
        )
        .expect("ruling");
        for args in [
            vec!["add", "-A"],
            vec!["commit", "--quiet", "-m", "unsigned ruling"],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(&root)
                    .args(&args)
                    .status()
                    .expect("git")
                    .success()
            );
        }
        let ruled = &rulings(&root).expect("parse")[0];
        assert!(
            !ruling_in_force(&root, ruled).expect("check"),
            "an unsigned commit is not in force"
        );
        // The effective count ignores a draft: a verified reproduced finding stays unresolved.
        let record = root.join("r.json");
        fs::write(&record, r#"{"attempts":[{"id":"A7","attack_class":"corpus leakage","target":"crates/liminal-xtask/src/haq.rs:1","classification":"verified_defect","independently_reproduced":true,"resolved":false}],"findings":[],"independently_reproduced":[]}"#).expect("record");
        assert_eq!(
            effective_unresolved_findings_repo(&root, &record).expect("count"),
            1
        );
    }

    /// F-44 ruling (2026-09-05): a stage may read the locked corpus -- the SLO
    /// graduation test measures on it by design -- but may never modify the
    /// repository's copy: opens with a writing flag, unlink, and the target of
    /// a rename all refuse, through aliases, and a relative write refuses
    /// conservatively. Copies under a build root are outside the repository.
    /// The carved fuzz binaries keep the full rule (see the union test).
    #[test]
    fn a_stage_may_read_the_locked_corpus_but_never_write_it() {
        let (_s, root, row) = scope_fixture("canaries");
        fs::create_dir_all(root.join("conformance/corpora/heldout")).expect("mkdir");
        fs::write(root.join("conformance/corpora/heldout/x.md"), "locked").expect("seed");
        let exit = "4242 +++ exited with 0 +++\n";
        let accept = |trace: String, why: &str| {
            let mut ok = row.clone();
            rewrite_scope_trace(&root, &mut ok, &trace);
            verify_corpus_scope_replays(&root, std::slice::from_ref(&ok), &[])
                .unwrap_or_else(|e| panic!("{why}: {e}"));
            verify_corpus_scope_rows(&root, &scope_audit(vec![ok]))
                .unwrap_or_else(|e| panic!("{why}: {e}"));
        };
        let refuse = |trace: String, why: &str| {
            let mut bad = row.clone();
            rewrite_scope_trace(&root, &mut bad, &trace);
            let err =
                verify_corpus_scope_replays(&root, std::slice::from_ref(&bad), &[]).expect_err(why);
            assert!(
                err.to_string().contains("wrote to the locked corpus"),
                "{why}: {err}"
            );
            let err = verify_corpus_scope_rows(&root, &scope_audit(vec![bad])).expect_err(why);
            assert!(
                err.to_string().contains("wrote to the locked corpus"),
                "{why}: {err}"
            );
        };
        accept(
            format!(
                "4242 openat(AT_FDCWD, \"{root}/conformance/corpora/heldout/x.md\", O_RDONLY|O_CLOEXEC) = 3\n{exit}"
            ),
            "a read is the graduation test's by design",
        );
        accept(
            format!(
                "4242 newfstatat(AT_FDCWD, \"{root}/conformance/corpora/heldout/x.md\", {{st_mode=S_IFREG|0644}}, 0) = 0\n{exit}"
            ),
            "git's index refresh stats every tracked path",
        );
        accept(
            format!(
                "4242 openat(AT_FDCWD, \"fuzz/corpus/cst_parse/01-empty.md\", O_RDONLY) = 3\n{exit}"
            ),
            "a stage helper's relative seed read is unresolvable, not a refusal",
        );
        accept(
            format!("4242 openat(7, \"../nvidia0\", O_RDONLY) = 8\n{exit}"),
            "a dirfd-relative open by reviewer tooling is unresolvable, not an escape",
        );

        accept(
            format!(
                "4242 openat(AT_FDCWD, \"/var/tmp/liminal-haqp-build/abc/conformance/corpora/heldout/x.md\", O_WRONLY|O_CREAT|O_TRUNC, 0644) = 3\n{exit}"
            ),
            "a copy under a build root is outside the repository",
        );
        refuse(
            format!(
                "4242 openat(AT_FDCWD, \"{root}/conformance/corpora/heldout/x.md\", O_WRONLY|O_CREAT|O_TRUNC, 0644) = 3\n{exit}"
            ),
            "an open for writing",
        );
        refuse(
            format!("4242 unlink(\"{root}/conformance/corpora/heldout/x.md\") = 0\n{exit}"),
            "an unlink",
        );
        refuse(
            format!(
                "4242 rename(\"{root}/tmp.md\", \"{root}/conformance/corpora/heldout/y.md\") = 0\n{exit}"
            ),
            "a rename INTO the corpus (the second path)",
        );
        refuse(
            format!(
                "4242 openat(AT_FDCWD, \"conformance/corpora/heldout/x.md\", O_WRONLY|O_APPEND) = 3\n{exit}"
            ),
            "a relative write, judged root-relative",
        );
        // Through an alias: a symlink elsewhere in the tree that resolves inside.
        std::os::unix::fs::symlink(root.join("conformance/corpora"), root.join("alias"))
            .expect("alias");
        refuse(
            format!("4242 openat(AT_FDCWD, \"{root}/alias/heldout/x.md\", O_RDWR) = 3\n{exit}"),
            "a write through an alias",
        );
    }

    /// The stage relaxations of F-44 do not reach a fuzz binary's trace: a
    /// relative corpus path is still the campaign's defect, and a relative
    /// path climbing out of the root is still an escape.
    #[test]
    fn a_fuzz_binary_trace_keeps_the_strict_relative_path_reading() {
        let (_s, root, _row) = scope_fixture("fuzz");
        let exit = "4242 +++ exited with 0 +++\n";
        let relative_seed = format!(
            "4242 openat(AT_FDCWD, \"fuzz/corpus/cst_parse/01-empty.md\", O_RDONLY) = 3\n{exit}"
        );
        let err = resolved_corpus_paths_digest(
            &root,
            root.as_str(),
            relative_seed.as_bytes(),
            "binary",
            CorpusAccess::IfPresent,
        )
        .expect_err("a fuzz binary's relative corpus path is the campaign's defect");
        assert!(err.to_string().contains("cwd/dirfd"), "{err}");
        let dirfd_relative = format!("4242 openat(7, \"../nvidia0\", O_RDONLY) = 8\n{exit}");
        let err = resolved_corpus_paths_digest(
            &root,
            root.as_str(),
            dirfd_relative.as_bytes(),
            "binary",
            CorpusAccess::IfPresent,
        )
        .expect_err("a fuzz binary climbing out of the root is refused");
        assert!(err.to_string().contains("escapes"), "{err}");
    }

    /// A non-fuzz scope with NO corpus access is fine; the `fuzz` scope with
    /// none is a defect. Before the ruling the first case was impossible to
    /// satisfy, which is why six of eight scopes could never pass.
    #[test]
    fn corpus_resolution_is_required_only_of_the_fuzz_scope() {
        let (_s, root, _) = scope_fixture("ci");
        let trace = format!(
            "1 openat(AT_FDCWD, \"{root}/Cargo.toml\", O_RDONLY) = 3\n1 +++ exited with 0 +++\n"
        );
        resolved_corpus_paths_digest(
            &root,
            root.as_str(),
            trace.as_bytes(),
            "ci",
            CorpusAccess::IfPresent,
        )
        .expect("a scope that opened no corpus path resolves to an empty set");
        let err = resolved_corpus_paths_digest(
            &root,
            root.as_str(),
            trace.as_bytes(),
            "fuzz",
            CorpusAccess::Required,
        )
        .expect_err("a fuzz run that opened no corpus path fuzzed nothing");
        assert!(
            err.to_string().contains("no resolvable fuzz corpus path"),
            "{err}"
        );
    }

    /// Presence is the closed set of eight lanes, exactly.
    #[test]
    fn scope_presence_requires_all_eight_lanes_and_no_others() {
        let all = [
            "ci",
            "canaries",
            "generated",
            "crash",
            "replay",
            "mutation",
            "reviews",
            "fuzz",
        ];
        let mk = |names: &[&str]| scope_audit(names.iter().map(|n| scope_fixture(n).2).collect());
        verify_corpus_scope_presence(&mk(&all)).expect("all eight");
        let err = verify_corpus_scope_presence(&mk(&all[..7])).expect_err("seven");
        assert!(err.to_string().contains("does not cover lane"), "{err}");
        verify_corpus_scope_presence(&scope_audit(Vec::new())).expect_err("none");
    }

    /// The wrapper reads concurrency.json and delegates; replacing it with
    /// Ok(()) survived because canary C32 exercises the RECORD function, never
    /// the read. A scratch root whose file contradicts the packet proves the
    /// wrapper actually reads it.
    #[test]
    fn concurrency_evidence_is_read_from_disk_not_assumed() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        verify_concurrency_evidence(&root, &packet)
            .expect("the committed not-applicable declaration must verify");

        let scratch = liminal_scratch::ScratchDir::new("haq-concurrency").expect("scratch");
        let sroot = scratch.path().to_owned();
        fs::create_dir_all(sroot.join("conformance/haqp/evidence")).expect("mkdir");
        let err = verify_concurrency_evidence(&sroot, &packet)
            .expect_err("no concurrency.json at all must be refused");
        assert!(
            err.to_string()
                .contains("concurrent schedule evidence required"),
            "{err}"
        );

        let mut doctored: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/evidence/concurrency.json")).expect("read"),
        )
        .expect("parse");
        doctored["mode"] = serde_json::Value::String("executed".to_owned());
        fs::write(
            sroot.join("conformance/haqp/evidence/concurrency.json"),
            serde_json::to_vec(&doctored).expect("serialize"),
        )
        .expect("write");
        let err = verify_concurrency_evidence(&sroot, &packet)
            .expect_err("a file whose mode contradicts the packet must be refused");
        assert!(err.to_string().contains("concurrency mode"), "{err}");
    }

    /// Canary evidence must equal a fresh replay from the FIXED commit's own
    /// packet and Markdown. Deleting the check survived: nothing compared the
    /// committed record to anything. The replay is a real `git show` plus the
    /// real canary suite, so this costs about a second.
    #[test]
    fn canary_evidence_must_match_a_replay_from_the_fixed_commit() {
        let root = repo_root();
        let head = git_text(&root, &["rev-parse", "HEAD"]).expect("HEAD");
        let mut provenance = scope_provenance();
        provenance.fixed_commit = head;
        let recorded: Vec<CanaryEvidence> = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/evidence/canaries.json")).expect("read"),
        )
        .expect("parse");
        verify_canary_evidence_replay(&root, &provenance, &recorded)
            .expect("committed canary evidence must replay identically from HEAD");

        let mut altered = recorded.clone();
        altered[0].caught = !altered[0].caught;
        let err = verify_canary_evidence_replay(&root, &provenance, &altered)
            .expect_err("a record that disagrees with its own replay must be refused");
        assert!(err.to_string().contains("differs from replay"), "{err}");

        let mut unknown = provenance.clone();
        unknown.fixed_commit = "0".repeat(40);
        let err = verify_canary_evidence_replay(&root, &unknown, &recorded)
            .expect_err("a fixed commit git cannot show must be refused");
        assert!(err.to_string().contains("git show"), "{err}");
    }

    /// The crash fault matrix is re-derived by an independent run of the
    /// crash-evidence binary and must equal the committed record. Deleting the
    /// check survived. The replay is real (~12s); the refusal is earned by a
    /// packet whose declared boundary set the replay cannot match.
    #[test]
    fn crash_evidence_must_match_an_independent_replay() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        verify_crash_replay(&root, &packet)
            .expect("committed crash evidence must equal an independent replay");

        let mut extra = packet.clone();
        let mut ghost = extra.crash_boundaries[0].clone();
        ghost.boundary = "ilrp/ghost_boundary".to_owned();
        extra.crash_boundaries.push(ghost);
        let err = verify_crash_replay(&root, &extra)
            .expect_err("a declared boundary the replay never exercised must be refused");
        assert!(
            err.to_string().to_ascii_lowercase().contains("boundary"),
            "must refuse on the boundary set: {err}"
        );
    }

    /// mutants.json is bound to the packet's provenance, the lockfile, the
    /// runner and the packet's own disposition claims. Deleting the check
    /// survived: the committed packet has no provenance, so the real tree can
    /// only exercise the refusal, never the acceptance -- which is why the
    /// accept case is built in scratch with a provenance that matches.
    #[test]
    fn mutant_evidence_is_bound_to_provenance_lockfile_and_packet() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        let err = verify_mutant_evidence(&root, &packet)
            .expect_err("a packet without provenance cannot claim mutant evidence");
        assert!(err.to_string().contains("provenance"), "{err}");

        // Scratch root: the real mutants.json and Cargo.lock, and a packet whose
        // provenance names the commit the evidence was produced at.
        let scratch = liminal_scratch::ScratchDir::new("haq-mutant-evidence").expect("scratch");
        let sroot = scratch.path().to_owned();
        fs::create_dir_all(sroot.join("conformance/haqp/evidence")).expect("mkdir");
        fs::copy(root.join("Cargo.lock"), sroot.join("Cargo.lock")).expect("lockfile");
        let mut evidence: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/evidence/mutants.json")).expect("evidence"),
        )
        .expect("parse");
        // The committed evidence predates this Cargo.lock (zstd was added to the
        // xtask after it was produced). The fixture binds evidence to the scratch
        // lockfile so the accept case is a real accept, not a hedge that swallows
        // a lockfile refusal -- and so the doctorings below reach THEIR check
        // instead of tripping over the lockfile first.
        evidence["lockfile_blake3"] = serde_json::Value::String(hex_digest(
            &fs::read(sroot.join("Cargo.lock")).expect("lock"),
        ));
        fs::write(
            sroot.join("conformance/haqp/evidence/mutants.json"),
            serde_json::to_vec(&evidence).expect("serialize"),
        )
        .expect("write");
        let produced_at = evidence["source_commit"]
            .as_str()
            .expect("source_commit")
            .to_owned();
        let mut bound = packet.clone();
        let mut prov = scope_provenance();
        prov.fixed_commit = produced_at.clone();
        prov.commit = produced_at;
        prov.lockfile_blake3 = hex_digest(&fs::read(root.join("Cargo.lock")).expect("lock"));
        bound.provenance = Some(prov);
        let rewrite = |mutate: &dyn Fn(&mut serde_json::Value)| {
            let mut v = evidence.clone();
            mutate(&mut v);
            fs::write(
                sroot.join("conformance/haqp/evidence/mutants.json"),
                serde_json::to_vec(&v).expect("serialize"),
            )
            .expect("write");
        };
        verify_mutant_evidence(&sroot, &bound)
            .expect("evidence bound to this provenance, lockfile and packet must verify");

        rewrite(&|v| v["lockfile_blake3"] = serde_json::Value::String("0".repeat(64)));
        let err = verify_mutant_evidence(&sroot, &bound).expect_err("lockfile drift");
        assert!(err.to_string().contains("lockfile"), "{err}");

        rewrite(&|v| v["schema_version"] = serde_json::Value::String("haqp-mutants-v0".to_owned()));
        let err = verify_mutant_evidence(&sroot, &bound).expect_err("wrong schema");
        assert!(err.to_string().contains("schema_version"), "{err}");

        rewrite(&|v| v["source_commit"] = serde_json::Value::String("0".repeat(40)));
        let err = verify_mutant_evidence(&sroot, &bound).expect_err("evidence from another commit");
        assert!(err.to_string().contains("source_commit"), "{err}");

        rewrite(&|v| v["runner"] = serde_json::Value::String("someone else".to_owned()));
        let err = verify_mutant_evidence(&sroot, &bound).expect_err("unknown runner");
        assert!(err.to_string().contains("runner"), "{err}");
    }

    /// F-41: a not-ready row is the absence of a claim. It is accepted for a
    /// predeclared mutant, counted toward nothing, and refused the moment it
    /// carries a result or belongs to a mutant that claims a disposition.
    #[test]
    fn a_not_ready_row_is_accepted_only_as_the_absence_of_a_claim() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        let declared = packet
            .mutants
            .iter()
            .map(|m| (m.id.as_str(), m))
            .collect::<BTreeMap<_, _>>();
        let evidence: MutantEvidence = serde_json::from_slice(
            &fs::read(root.join("conformance/haqp/evidence/mutants.json")).expect("evidence"),
        )
        .expect("parse");
        let row = evidence.rows[0].clone();
        assert_eq!(
            row.status, "not-ready",
            "the committed 1a evidence is not-ready"
        );

        let (mut seen, mut evaluated) = (BTreeSet::new(), BTreeSet::new());
        verify_mutant_evidence_row(&row, &declared, &mut seen, &mut evaluated)
            .expect("a not-ready row for a predeclared mutant is fine");
        assert!(
            evaluated.is_empty(),
            "not-ready counts toward no evaluated set"
        );

        let mut claims = row.clone();
        claims.status = "killed".to_owned();
        let err = verify_mutant_evidence_row(
            &claims,
            &declared,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
        )
        .expect_err("a predeclared mutant may not carry an evaluated status");
        assert!(err.to_string().contains("has evaluated evidence"), "{err}");

        let mut leaks = row.clone();
        leaks.exit_codes = vec![1];
        let err = verify_mutant_evidence_row(
            &leaks,
            &declared,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
        )
        .expect_err("not-ready yet carrying results");
        assert!(err.to_string().contains("carries results"), "{err}");

        let mut promoted = packet.mutants[0].clone();
        promoted.disposition = "killed".to_owned();
        let one = BTreeMap::from([(promoted.id.as_str(), &promoted)]);
        let err =
            verify_mutant_evidence_row(&row, &one, &mut BTreeSet::new(), &mut BTreeSet::new())
                .expect_err("a mutant that claims killed cannot be backed by a not-ready row");
        assert!(err.to_string().contains("is not ready"), "{err}");
    }

    /// F-42. A real rebuild at the canonical path. The first call carries a
    /// deliberately wrong digest so the refusal proves the build ran and the
    /// digest is compared; the binary it produced is then hashed and fed back,
    /// and the second call must ACCEPT -- which is also the reproducibility
    /// claim itself, measured. The committed proof is not used as the accept
    /// oracle on purpose: it was built in-tree and cannot match, which is the
    /// verdict the lane will (correctly) hand it until the campaign reruns.
    #[test]
    fn sanitizer_replay_reproduces_the_canonical_build_and_refuses_the_rest() {
        let root = repo_root();
        let head = git_text(&root, &["rev-parse", "HEAD"]).expect("HEAD");
        let mut provenance = scope_provenance();
        provenance.fixed_commit = head.clone();
        let (canon_root, canon_flags) = sanitizer_build_canon(&head);
        let row = fuzz_row("cst_parse", 30);
        let proof = |digest: &str, root: &str, flags: &str| SanitizerProof {
            build_command: "cargo +nightly fuzz build -s address cst_parse".to_owned(),
            build_root: root.to_owned(),
            build_rustflags: flags.to_owned(),
            binary: "conformance/haqp/evidence/binaries/cst_parse".to_owned(),
            binary_blake3: digest.to_owned(),
            runtime_probe: "conformance/haqp/evidence/probes/cst_parse.log".to_owned(),
            runtime_probe_blake3: "0".repeat(64),
            runtime_probe_exit_code: 0,
            instrumentation_flags: vec!["-fsanitize=address".to_owned()],
            build_log: "conformance/haqp/evidence/build-logs/cst_parse.log".to_owned(),
            build_log_blake3: "0".repeat(64),
            source_commit: head.clone(),
            source_tree: "0".repeat(40),
            build_result: "pass".to_owned(),
        };

        // Closed registry: a proof recorded at any other path or with other
        // flags is refused before a single compile.
        let err = verify_sanitizer_build_replay(
            &root,
            &row,
            &proof(&"0".repeat(64), "/elsewhere", &canon_flags),
            &provenance,
        )
        .expect_err("non-canonical build root");
        assert!(err.to_string().contains("build_root"), "{err}");
        let err = verify_sanitizer_build_replay(
            &root,
            &row,
            &proof(&"0".repeat(64), canon_root.as_str(), "-C opt-level=3"),
            &provenance,
        )
        .expect_err("non-canonical flags");
        assert!(err.to_string().contains("build_rustflags"), "{err}");

        // Real build; wrong digest must be refused AFTER building.
        let err = verify_sanitizer_build_replay(
            &root,
            &row,
            &proof(&"0".repeat(64), canon_root.as_str(), &canon_flags),
            &provenance,
        )
        .expect_err("a digest that is not the rebuilt binary's");
        assert!(err.to_string().contains("digest"), "{err}");

        // Hash what that build produced and feed it back: must accept, and the
        // second build must reproduce the first (cached, but re-verified).
        let built = find_named_files(&canon_root.join("fuzz/target"), "cst_parse")
            .expect("find")
            .into_iter()
            .find(|p| p.file_name() == Some("cst_parse"))
            .expect("the replay built a binary");
        let digest = blake3::hash(&fs::read(&built).expect("read built"))
            .to_hex()
            .to_string();
        verify_sanitizer_build_replay(
            &root,
            &row,
            &proof(&digest, canon_root.as_str(), &canon_flags),
            &provenance,
        )
        .expect("a proof carrying the canonical build's own digest must verify");

        // A target that does not exist cannot be built, and the failure is the
        // build's, not a guess.
        let mut ghost = fuzz_row("no_such_target", 30);
        ghost.sanitizer = "address".to_owned();
        let err = verify_sanitizer_build_replay(
            &root,
            &ghost,
            &proof(&digest, canon_root.as_str(), &canon_flags),
            &provenance,
        )
        .expect_err("unknown fuzz target");
        assert!(err.to_string().contains("build failed"), "{err}");
    }

    /// The cargo-home remap source is the campaign machine's $HOME; a proof from
    /// another machine must still be accepted when its bytes are identical, while
    /// a different BUILD-ROOT remap -- which does shape the bytes -- must not.
    #[test]
    fn build_rustflags_are_compared_with_the_cargo_home_source_normalized() {
        let (_root, canon) = sanitizer_build_canon("0123456789abcdef0123456789abcdef01234567");
        let from_ci = canon
            .split_whitespace()
            .map(|f| {
                if f.ends_with("=/cargo") {
                    "--remap-path-prefix=/home/ci/.cargo=/cargo".to_owned()
                } else {
                    f.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert_ne!(
            from_ci, canon,
            "the fixture must actually differ in the home source"
        );
        assert_eq!(
            normalize_build_rustflags(&from_ci),
            normalize_build_rustflags(&canon)
        );

        let other_root = canon.replace("=/liminal", "=/elsewhere");
        assert_ne!(
            normalize_build_rustflags(&other_root),
            normalize_build_rustflags(&canon),
            "a different build-root remap changes the bytes and must not normalize away"
        );
    }

    /// M17.5 F-31 / P1-A08: the scenarios block was written by the fault lane
    /// and read by nothing.
    #[test]
    fn crash_scenarios_are_verified_not_merely_carried() {
        let bytes = fs::read(repo_root().join("conformance/haqp/evidence/crash.json"))
            .expect("committed crash evidence");
        let recorded: CrashEvidence = serde_json::from_slice(&bytes).expect("parse");
        let declared: BTreeSet<String> = recorded
            .boundaries
            .iter()
            .map(|row| row.boundary.clone())
            .collect();
        verify_crash_rows(&recorded, &declared).expect("the committed matrix must pass");

        let mut failed = recorded.clone();
        failed.scenarios[0].result = "fail".to_owned();
        verify_crash_rows(&failed, &declared)
            .expect_err("a scenario reporting failure must not pass");

        let mut non_idempotent = recorded.clone();
        non_idempotent.boundaries[0].recovery_pairs[0].second_recovery_digest = "00".repeat(32);
        verify_crash_rows(&non_idempotent, &declared)
            .expect_err("a second recovery digest that changes must be rejected");

        let mut inert = recorded.clone();
        inert.scenarios[0].faults_injected = 0;
        verify_crash_rows(&inert, &declared)
            .expect_err("a scenario that injected nothing exercised nothing");

        let mut stray = recorded.clone();
        stray.scenarios[0]
            .boundaries
            .insert("ilrp/not_registered".to_owned());
        verify_crash_rows(&stray, &declared)
            .expect_err("a scenario may not exercise an unregistered boundary");

        let mut miscounted = recorded.clone();
        miscounted.scenarios[0].faults_injected = 1;
        verify_crash_rows(&miscounted, &declared)
            .expect_err("fewer faults than distinct boundaries is impossible");

        let mut empty = recorded;
        empty.scenarios.clear();
        verify_crash_rows(&empty, &declared).expect_err("an empty fault matrix proves nothing");
    }

    #[test]
    fn crash_injection_binding_accepts_closed_finalize_pair() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        let bytes =
            fs::read(root.join("conformance/haqp/evidence/crash.json")).expect("crash evidence");
        let recorded: CrashEvidence = serde_json::from_slice(&bytes).expect("parse crash evidence");
        verify_crash_injection_bindings(&packet, &recorded)
            .expect("closed finalization before/after pair must bind");
    }

    #[test]
    fn corpus_resolution_rejects_relative_paths_without_cwd_receipt() {
        let root = repo_root();
        let trace_root = root.as_str();
        let trace = "1 openat(AT_FDCWD, \"fuzz/corpus/cst_parse\", O_RDONLY) = 3\n";
        let err = verify_trace_corpus_resolution(
            &root,
            trace_root,
            trace.as_bytes(),
            &"0".repeat(64),
            "relative-corpus-test",
            CorpusAccess::Required,
        )
        .expect_err("relative corpus path must not be rebased on trace root");
        assert!(err.to_string().contains("authenticated cwd/dirfd"), "{err}");
    }

    #[test]
    fn deleted_corpus_entry_fails_closed_without_symlink_history() {
        let scratch =
            liminal_scratch::ScratchDir::new("haq-deleted-corpus-entry").expect("scratch");
        let corpus = scratch.join("fuzz/corpus");
        fs::create_dir_all(&corpus).expect("corpus root");
        let missing = corpus.join("target").join("deleted-after-open");
        let err = canonicalize_trace_path(missing.as_std_path())
            .expect_err("a deleted corpus entry cannot prove historical symlink target");
        assert!(err.to_string().contains("symlink history"), "{err}");
    }

    // ── AM-17.6: the metadata child may lift ONE #[ignore] and nothing else ──
    // Widening the path allowlist alone would let gate LOGIC change in the
    // child, which is the whole thing verify_provenance exists to prevent.

    fn gate_diff_verdict(diff: &str) -> Result<()> {
        let mut removed = Vec::new();
        let mut added = Vec::new();
        for line in diff.lines() {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if let Some(rest) = line.strip_prefix('-') {
                removed.push(rest.trim().to_owned());
            } else if let Some(rest) = line.strip_prefix('+') {
                added.push(rest.trim().to_owned());
            }
        }
        anyhow::ensure!(added.is_empty(), "added {} line(s): {added:?}", added.len());
        anyhow::ensure!(
            removed.len() == 1,
            "removed {} line(s): {removed:?}",
            removed.len()
        );
        require_eq(
            "metadata child's lifted attribute",
            removed[0].as_str(),
            PHASE0_GATE_IGNORE,
        )
    }

    #[test]
    fn the_child_may_lift_exactly_the_qualification_gate() {
        gate_diff_verdict(&format!("--- a\n+++ b\n-{PHASE0_GATE_IGNORE}\n"))
            .expect("lifting exactly that attribute is the one permitted change");
    }

    #[test]
    fn the_child_may_not_lift_a_different_ignore() {
        gate_diff_verdict("--- a\n+++ b\n-#[ignore = \"Phase 1: something else\"]\n")
            .expect_err("only the qualification gate's own attribute may be lifted");
    }

    #[test]
    fn the_child_may_not_add_or_change_gate_code() {
        gate_diff_verdict(&format!(
            "--- a\n+++ b\n-{PHASE0_GATE_IGNORE}\n+assert!(true);\n"
        ))
        .expect_err("the child may not add gate code alongside the lift");
        gate_diff_verdict(&format!(
            "--- a\n+++ b\n-{PHASE0_GATE_IGNORE}\n-let packet = doctored();\n"
        ))
        .expect_err("the child may not remove gate code alongside the lift");
        gate_diff_verdict("--- a\n+++ b\n")
            .expect_err("a child that touches the file without lifting anything is unexplained");
    }

    /// The constant must match the attribute actually in the tree, or the
    /// check passes vacuously against a file it no longer describes.
    #[test]
    fn the_expected_gate_attribute_exists_in_the_tree() {
        let text = fs::read_to_string(repo_root().join(PHASE0_GATE_FILE))
            .expect("the phase0 gate file must exist");
        assert!(
            text.contains(PHASE0_GATE_IGNORE),
            "the qualification gate's #[ignore] is not the string AM-17.6 pins; \
             either it was already lifted or its wording drifted"
        );
    }

    // ── M17.5 F-33: inapplicable mutants ──────────────────────────────────
    // 36 of the packet's 65 mutants are anchored to declarations. ADR-0021
    // defers §3 to 1b, so this refuses THERE — but it must refuse, or M24
    // rediscovers it after Phase 1 is built.

    #[test]
    fn a_declaration_line_can_hold_no_mutation() {
        for declaration in [
            "pub fn relations_from(",
            "    fn slot_map(input: &str) -> BTreeMap<String, String> {",
            "pub struct NodeFlags(pub u16);",
            "    Federated {",
            "impl Formatter for MarkdownFormatter {",
            "use std::collections::BTreeMap;",
        ] {
            assert!(
                !anchor_is_mutable_line(declaration),
                "{declaration:?} declares a name; there is nothing on it to mutate"
            );
        }
    }

    /// The narrowing is only correct if what it now ACCEPTS is genuinely
    /// mutable. A first version matched operators to line tokens and falsely
    /// accused all three of these.
    #[test]
    fn real_mutation_sites_are_accepted() {
        for site in [
            "        NodeFlags(self.0 | other.0)",
            "        if rev == inner.state.head {",
            "        for entry in entries.flatten() {",
            "            if fail_after.is_some_and(|limit| committed >= limit) {",
            "        let mut sorted = items.clone(); sorted.sort();",
        ] {
            assert!(
                anchor_is_mutable_line(site),
                "{site:?} is a real mutation site and must not be refused"
            );
        }
    }

    /// F-33 is CLOSED. This used to assert the opposite -- that exactly 36
    /// declared mutants were anchored to declarations -- because the defect was
    /// disclosed and deferred rather than fixed. A blind reviewer then
    /// rediscovered it at six coordinates on 2026-09-02 and §6 refused the
    /// campaign, which is what a disclosed-but-unfixed defect earns.
    ///
    /// The anchors were repaired, so the invariant to hold now is the positive
    /// one: every declared mutant names a line where its declared operator can
    /// genuinely be applied. Without this, the plan can silently rot back --
    /// a refactor that moves a line turns a live anchor into a signature again,
    /// and nothing would notice until a reviewer did.
    #[test]
    fn every_declared_mutant_anchors_a_line_its_operator_can_mutate() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        verify_mutant_anchors_support_operators(&root, &packet).expect(
            "every declared mutant must anchor a mutable line; re-anchor the plan \
             rather than disclosing the gap again (M17.5 F-33)",
        );
        // The distribution matters as much as the anchors: §3 refuses any single
        // operator supplying more than a quarter of the denominator.
        let mut per_operator = BTreeMap::<&str, usize>::new();
        for mutant in &packet.mutants {
            *per_operator.entry(mutant.operator.as_str()).or_default() += 1;
        }
        let cap = packet.mutants.len() / 4;
        for (operator, count) in &per_operator {
            assert!(
                *count <= cap,
                "operator {operator} supplies {count} of {} mutants; §3 caps one \
                 operator at {cap}",
                packet.mutants.len()
            );
        }
    }

    // ── M17.5 F-34: blind pass 1 A07 and A11 ──────────────────────────────

    /// A11: renaming a LABEL is not skipping a durable transition.
    #[test]
    fn a_label_rename_is_not_a_skipped_durable_transition() {
        let before = r#"self.commit_intent(id, intent, &format!("ack:{step_id}"), origin)?;"#;
        let patch = |after: &str| MutantPatch {
            file: "crates/liminal-jurisdiction/src/ilrp.rs".to_owned(),
            before: before.to_owned(),
            after: after.to_owned(),
        };
        verify_mutant_operator_patch(
            "skipped-durable-transition",
            &patch(r#"self.commit_intent(id, intent, &format!("step:{step_id}"), origin)?;"#),
        )
        .expect_err("the call still runs and persistence is intact; only the label moved");
        // Removing the CALL is the real thing, and must still be accepted.
        verify_mutant_operator_patch(
            "skipped-durable-transition",
            &patch("// durable commit removed"),
        )
        .expect("deleting the durable call IS a skipped transition");
    }

    /// A07: the locked corpus must have exactly one name, or the audit's
    /// substring filter is sound only by luck.
    #[test]
    fn the_locked_corpus_has_no_second_name() {
        verify_locked_corpus_has_no_aliases(&repo_root())
            .expect("no alias may reach conformance/corpora/heldout");
    }

    // ── M17.5 F-35: verifiers that had never executed ─────────────────────
    // Mutation found `verify_recovery_proof_presence`, `verify_crash_replay`
    // and `verify_canary_evidence_replay` all surviving replacement with
    // `Ok(())`. They sit in verify_qualified_repo AFTER the
    // `qualification_state` check, which has always bailed first — so the whole
    // tail of the qualified gate was dead code to the suite. Same shape as
    // F-30's repo-level gates, one layer deeper.

    #[test]
    fn recovery_proof_presence_reads_the_committed_matrix() {
        let (recorded, _) = crash_evidence_from_repo();
        verify_recovery_proof_presence(&recorded)
            .expect("the committed crash matrix must carry recovery proofs");

        // Emptiness is NOT this function's job — it iterates the pairs and
        // checks each one's digests, so zero pairs is vacuously fine here.
        // `verify_recovery_pairs` owns that, requiring one pair per exercised
        // occurrence. Asserting it against the wrong function was my error;
        // both are pinned below rather than one being "fixed" to cover the
        // other.
        let mut hollow = recorded.clone();
        for row in &mut hollow.boundaries {
            for pair in &mut row.recovery_pairs {
                pair.second_effect_digest.clear();
            }
        }
        verify_recovery_proof_presence(&hollow)
            .expect_err("a recovery pair missing a digest proves no duplicate recovery");

        let mut stripped = recorded;
        for row in &mut stripped.boundaries {
            row.recovery_pairs.clear();
        }
        for row in &stripped.boundaries {
            verify_recovery_pairs(row)
                .expect_err("zero pairs for an exercised boundary proves no recovery");
        }
    }

    #[test]
    fn crash_replay_reads_the_committed_evidence() {
        let root = repo_root();
        let packet = read_packet(&root).expect("packet");
        // Whatever the verdict, it must REACH one rather than be dead code.
        let verdict = verify_crash_replay(&root, &packet);
        if let Err(error) = verdict {
            let message = error.to_string();
            assert!(
                !message.contains("No such file"),
                "crash replay must find its inputs, not vanish: {message}"
            );
        }
    }
}
