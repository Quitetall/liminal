//! The five trace importers/generators (M11 Algorithm B; AM-11.3).
//!
//! Each generator produces one complete NDJSON trace (header first, D11.5
//! schema) as a `String`; the `tracegen` bin is a thin CLI over these fns.
//! Everything is deterministic: seeded M07 machinery, fixed `at` schedules,
//! pinned hermetic git env — the same invocation twice yields identical
//! bytes (required for dev-corpus reproducibility and the heldout lock).

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use rand::SeedableRng;
use rand::rngs::SmallRng;

use crate::identity::gitenv;
use crate::identity::ops::{self, Operation};
use crate::identity::strategy::BaseWorld;
use crate::identity::{self};
use crate::trace::{
    CauseCategory, Consent, EditRange, GitOpKind, TraceEvent, TraceSetup, TraceSetupFile,
};

/// Common per-trace options every generator takes.
#[derive(Debug, Clone)]
pub struct GenOptions {
    /// The profile the trace's header declares (Algorithm D: dev corpus is
    /// populated per profile).
    pub profile: String,
    /// The header `trace_id` (the bin derives it from the output file stem).
    pub trace_id: String,
}

/// Serialize events into NDJSON (one line per event, trailing newline).
fn ndjson(events: &[TraceEvent]) -> anyhow::Result<String> {
    let mut out = String::new();
    for event in events {
        out.push_str(&serde_json::to_string(event)?);
        out.push('\n');
    }
    Ok(out)
}

/// Build the mandatory line-1 header.
fn header(opts: &GenOptions, source: String, setup: TraceSetup) -> TraceEvent {
    TraceEvent::TraceHeader {
        version: 1,
        trace_id: opts.trace_id.clone(),
        source,
        capture_tool: "tracegen".into(),
        consent: Consent::Synthetic,
        profile: opts.profile.clone(),
        setup,
    }
}

/// The kebab-case wire name of a [`GitOpKind`] (used inside `cause_key =
/// "<op>@<at>"`).
fn kind_wire(kind: GitOpKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Run one hermetic-repo git command and return trimmed stdout, bailing on
/// failure (`gitenv::git` applies the pinned M07 environment).
fn git_out(repo: &Utf8Path, args: &[&str]) -> anyhow::Result<String> {
    let out = gitenv::git(repo, args).output()?;
    anyhow::ensure!(
        out.status.success(),
        "git {args:?} failed in {repo}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// `tracegen git <op-script> <out>` (Algorithm B): replay an M07 git op
/// script in the hermetic runbook, then walk `git log --reverse`. The base
/// commit's tree becomes `setup.files` (the workspace the trace replays
/// against); each subsequent commit is step `k` (1-based) at
/// `at = k × 60_000`, emitting one `git_op` plus one `file_change_external`
/// per changed file carrying the SAME cause pair (the byte delivery of the
/// already-counted op — Algorithm A's double-count guard keys on this).
///
/// `op_script` is `<m7-git-op>` or `<m7-git-op>@<seed>` (default seed =
/// the first configured corpus seed); the header records it verbatim as
/// `source: "m7-script:<op-script>"`, `consent: "synthetic"`.
///
/// # Errors
/// Unknown/non-git op, a git failure, or a below-floor `git` (D07.6).
pub fn git_trace(op_script: &str, opts: &GenOptions) -> anyhow::Result<String> {
    let cfg = identity::Config::load()?;
    gitenv::assert_min_git(&cfg.min_git)?;

    let (op_name, seed) = match op_script.split_once('@') {
        Some((name, seed)) => (name, seed.parse::<u64>()?),
        None => (
            op_script,
            *cfg.seeds
                .first()
                .ok_or_else(|| anyhow::anyhow!("empty corpus seeds"))?,
        ),
    };
    let op = Operation::from_name(op_name)?;
    anyhow::ensure!(
        op.is_git(),
        "tracegen git requires a git op (git_merge|git_rebase|git_cherry_pick), got {op_name:?}"
    );

    let world = BaseWorld::build(&cfg.template);
    let mut rng = SmallRng::seed_from_u64(seed);
    let (_out, repo) = gitenv::run_git_op_with_repo(op, &world, seed, &mut rng, &cfg)?;

    // Walk the produced history oldest-first.
    let shas: Vec<String> = git_out(&repo, &["log", "--reverse", "--format=%H", "HEAD"])?
        .lines()
        .map(str::to_owned)
        .collect();
    anyhow::ensure!(!shas.is_empty(), "empty git history in {repo}");

    // Base commit -> setup.files (full tree).
    let mut setup_files = Vec::new();
    for path in git_out(&repo, &["ls-tree", "-r", "--name-only", &shas[0]])?.lines() {
        let contents = git_out(&repo, &["show", &format!("{}:{path}", shas[0])])?;
        setup_files.push(TraceSetupFile {
            path: path.to_owned(),
            contents: format!("{contents}\n"),
        });
    }

    let script_kind = match op {
        Operation::GitMerge => GitOpKind::Merge,
        Operation::GitRebase => GitOpKind::Rebase,
        Operation::GitCherryPick => GitOpKind::CherryPick,
        _ => unreachable!("is_git checked above"),
    };

    let mut events = vec![header(
        opts,
        format!("m7-script:{op_script}"),
        TraceSetup {
            files: setup_files,
            graph: Vec::new(),
        },
    )];

    for (k, sha) in shas.iter().enumerate().skip(1) {
        let at = (k as u64) * 60_000;
        let parents = git_out(&repo, &["log", "-1", "--format=%P", sha])?;
        let parent_count = parents.split_whitespace().count();
        let kind = if parent_count >= 2 {
            GitOpKind::Merge
        } else if k == shas.len() - 1 && script_kind != GitOpKind::Merge {
            // The replayed commit of a rebase/cherry-pick script (a linear
            // history's last commit; a skipped-empty pick simply never
            // appears in the walk).
            script_kind
        } else {
            GitOpKind::Commit
        };
        let cause_key = format!("{}@{at}", kind_wire(kind));

        // Changed files vs the FIRST parent (a clean side-taking merge may
        // legitimately change nothing vs main — an empty file list then
        // honestly contributes zero ops).
        let files: Vec<String> = git_out(&repo, &["diff", "--name-only", &format!("{sha}^"), sha])?
            .lines()
            .map(str::to_owned)
            .collect();

        events.push(TraceEvent::GitOp {
            at,
            op: kind,
            files: files.clone(),
            cause_category: CauseCategory::Git,
            cause_key: cause_key.clone(),
        });
        for path in &files {
            let contents = git_out(&repo, &["show", &format!("{sha}:{path}")])?;
            events.push(TraceEvent::FileChangeExternal {
                at,
                path: path.clone(),
                contents: format!("{contents}\n"),
                cause_category: CauseCategory::Git,
                cause_key: cause_key.clone(),
            });
        }
    }

    ndjson(&events)
}

/// `tracegen session <session.toml> <out>` (Algorithm B): expand a
/// scenario-style TOML (the M02 `ScenarioScript` shape: `[setup]` files +
/// buffers, `[[step]]` with kinds `buffer_edit`/`save`) into
/// `session_open` / `buffer_open` / `buffer_edit` / `save` /
/// `session_close` with an `at` schedule — each step advances `at` by its
/// own `dt` field (integer ms), default 500. One session per client, id =
/// the client name. Any other step kind fails loudly — the session
/// generator's vocabulary is exactly the five event kinds the order lists.
///
/// # Errors
/// Unparsable TOML, an unknown step kind, or `find` text absent from the
/// tracked buffer.
// Long because it expands the full open/buffer/edit/save/close vocabulary
// in one linear pass — splitting the schedule state across helpers would
// obscure the `at` bookkeeping.
#[allow(clippy::too_many_lines)]
pub fn session_trace(toml_path: &Utf8Path, opts: &GenOptions) -> anyhow::Result<String> {
    let script = liminal_daemon::scenario::ScenarioScript::load(toml_path)?;

    let setup = TraceSetup {
        files: script
            .setup
            .files
            .iter()
            .map(|f| TraceSetupFile {
                path: f.path.clone(),
                contents: f.text.clone(),
            })
            .collect(),
        graph: script.setup.graph.clone(),
    };
    let mut events = vec![header(
        opts,
        format!("session-script:{}", opts.trace_id),
        setup,
    )];

    // One session per client, in first-appearance order.
    let mut clients: Vec<String> = Vec::new();
    for buf in &script.setup.buffers {
        if !clients.contains(&buf.client) {
            clients.push(buf.client.clone());
        }
    }
    anyhow::ensure!(
        !clients.is_empty(),
        "{toml_path}: no [[setup.buffer]] entries"
    );

    let mut at: u64 = 0;
    for client in &clients {
        events.push(TraceEvent::SessionOpen {
            at,
            session: client.clone(),
            client: client.clone(),
        });
    }

    // Open one buffer per setup entry; track its text for splice synthesis.
    let mut buffer_ids: BTreeMap<(String, String), String> = BTreeMap::default();
    let mut buffer_text: BTreeMap<(String, String), String> = BTreeMap::default();
    for (i, buf) in script.setup.buffers.iter().enumerate() {
        at += 500;
        let id = format!("b{}", i + 1);
        let base = script
            .setup
            .files
            .iter()
            .find(|f| f.path == buf.path)
            .map(|f| f.text.clone())
            .ok_or_else(|| anyhow::anyhow!("{toml_path}: buffer over unknown file {}", buf.path))?;
        events.push(TraceEvent::BufferOpen {
            at,
            session: buf.client.clone(),
            client: buf.client.clone(),
            buffer: id.clone(),
            path: buf.path.clone(),
        });
        let key = (buf.client.clone(), buf.path.clone());
        buffer_ids.insert(key.clone(), id);
        buffer_text.insert(key, base);
    }

    for step in &script.steps {
        let dt = step
            .extra
            .get("dt")
            .and_then(toml::Value::as_integer)
            .map_or(500u64, |v| u64::try_from(v).unwrap_or(500));
        at += dt;
        let sfield = |name: &str| -> anyhow::Result<&str> {
            step.extra
                .get(name)
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("{toml_path}: {} missing {name}", step.kind))
        };
        match step.kind.as_str() {
            "buffer_edit" => {
                let key = (sfield("client")?.to_owned(), sfield("path")?.to_owned());
                let (find, replace) = (sfield("find")?, sfield("replace")?);
                let buffer = buffer_ids
                    .get(&key)
                    .ok_or_else(|| anyhow::anyhow!("{toml_path}: edit on unopened buffer {key:?}"))?
                    .clone();
                let text = buffer_text.get_mut(&key).expect("id and text maps agree");
                let pos = text
                    .find(find)
                    .ok_or_else(|| anyhow::anyhow!("{toml_path}: find text absent: {find:?}"))?;
                events.push(TraceEvent::BufferEdit {
                    at,
                    session: key.0.clone(),
                    buffer,
                    range: EditRange {
                        start: pos as u64,
                        end: (pos + find.len()) as u64,
                    },
                    insert: replace.to_owned(),
                });
                *text = format!("{}{}{}", &text[..pos], replace, &text[pos + find.len()..]);
            }
            "save" => {
                let key = (sfield("client")?.to_owned(), sfield("path")?.to_owned());
                let buffer = buffer_ids
                    .get(&key)
                    .ok_or_else(|| anyhow::anyhow!("{toml_path}: save on unopened buffer {key:?}"))?
                    .clone();
                events.push(TraceEvent::Save {
                    at,
                    session: key.0.clone(),
                    buffer,
                });
            }
            other => anyhow::bail!(
                "{toml_path}: step kind {other:?} is outside the session generator's \
                 open/buffer/edit/save/close vocabulary (Algorithm B)"
            ),
        }
    }

    for client in &clients {
        at += 500;
        events.push(TraceEvent::SessionClose {
            at,
            session: client.clone(),
        });
    }

    ndjson(&events)
}

/// The M07 op-7 deterministic formatter over one whole document: parse into
/// M03 blocks, greedily re-wrap each block SOURCE (marker included) to
/// width 72 with whitespace collapsed, join with one blank line, single
/// trailing newline — byte-identical to `identity::ops`'s
/// `formatter_rewrite` operation applied to the same text.
#[must_use]
pub fn format_document(text: &str) -> String {
    let blocks = liminal_source::paragraph::parse(text);
    let body = blocks
        .iter()
        .map(|b| ops::wrap72(&ops::block_source(&b.text, b.id.as_deref())))
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("{body}\n")
}

/// `tracegen formatter <fixture-dir> <out>` (Algorithm B): deterministic
/// rewrites in the M07 op-7 vocabulary. Every regular file in the fixture
/// dir (sorted by name) enters `setup.files` with its original bytes and
/// yields one `formatter_rewrite` event at `at = k × 1000` (k = 1-based)
/// carrying the fully formatted contents, `tool: "wrap72"`, and cause pair
/// `("formatter", "wrap72@<at>")`.
///
/// # Errors
/// IO failures reading the fixture dir.
pub fn formatter_trace(dir: &Utf8Path, opts: &GenOptions) -> anyhow::Result<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .flatten()
        .filter_map(|e| {
            let p = Utf8PathBuf::from_path_buf(e.path()).ok()?;
            p.is_file()
                .then(|| p.file_name().unwrap_or_default().to_owned())
        })
        .collect();
    names.sort();
    anyhow::ensure!(!names.is_empty(), "{dir}: empty formatter fixture dir");

    let mut setup_files = Vec::new();
    let mut rewrites = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let original = std::fs::read_to_string(dir.join(name))?;
        let at = ((i as u64) + 1) * 1000;
        setup_files.push(TraceSetupFile {
            path: name.clone(),
            contents: original.clone(),
        });
        rewrites.push(TraceEvent::FormatterRewrite {
            at,
            path: name.clone(),
            contents: format_document(&original),
            tool: "wrap72".into(),
            cause_category: CauseCategory::Formatter,
            cause_key: format!("wrap72@{at}"),
        });
    }

    let mut events = vec![header(
        opts,
        format!("formatter:{}", dir.file_name().unwrap_or("fixture")),
        TraceSetup {
            files: setup_files,
            graph: Vec::new(),
        },
    )];
    events.extend(rewrites);
    ndjson(&events)
}

/// `tracegen offline <out>` (Algorithm B): the fixed offline/online shape —
/// `holder_unavailable` → an edit + save (a durable declared-transient
/// Overlay, D05.2) → `clock_advance` PAST the escalate window (the
/// ExternalFileProfile's 86 400 s, M05 schema) → `holder_available`. Drives
/// the transient-draft/aging path the third slo test asserts. Explicit
/// session open/close bracket the whole span (D11.2: the bracket wins, so
/// the 25 h jump does not split the session).
///
/// # Errors
/// Serialization only.
pub fn offline_trace(opts: &GenOptions) -> anyhow::Result<String> {
    let base = "The draft paragraph awaiting sync. {#p-draft}\n";
    let edited = "The draft paragraph awaiting sync, edited while offline. {#p-draft}\n";
    // The splice covering the change: replace the whole text (start 0, end
    // len) — byte-exact and robust.
    let events = vec![
        header(
            opts,
            format!("offline-script:{}", opts.trace_id),
            TraceSetup {
                files: vec![TraceSetupFile {
                    path: "notes.md".into(),
                    contents: base.into(),
                }],
                graph: Vec::new(),
            },
        ),
        TraceEvent::SessionOpen {
            at: 0,
            session: "offline".into(),
            client: "phone".into(),
        },
        TraceEvent::BufferOpen {
            at: 500,
            session: "offline".into(),
            client: "phone".into(),
            buffer: "b1".into(),
            path: "notes.md".into(),
        },
        TraceEvent::HolderUnavailable {
            at: 1000,
            holder: "notes.md".into(),
            cause_category: CauseCategory::Holder,
            cause_key: "notes.md@1000".into(),
        },
        TraceEvent::BufferEdit {
            at: 1500,
            session: "offline".into(),
            buffer: "b1".into(),
            range: EditRange {
                start: 0,
                end: base.len() as u64,
            },
            insert: edited.into(),
        },
        TraceEvent::Save {
            at: 2000,
            session: "offline".into(),
            buffer: "b1".into(),
        },
        // 90 000 s = 25 h — past the 86 400 s (24 h) escalate window.
        TraceEvent::ClockAdvance { at: 90_000_000 },
        TraceEvent::HolderAvailable {
            at: 90_000_500,
            holder: "notes.md".into(),
        },
        TraceEvent::SessionClose {
            at: 90_001_000,
            session: "offline".into(),
        },
    ];
    ndjson(&events)
}

/// `tracegen adversarial <m7-op> <seed> <out>` (Algorithm B): one M07
/// identity-torture op applied to the M07 template, replayed as a single
/// `file_change_external { cause_category: "foreign" }` — independent
/// because it is generated by the identity lab, not tuned by profile
/// authors (R4 §2.2).
///
/// # Errors
/// Unknown op name, or M07 machinery failures.
pub fn adversarial_trace(op_name: &str, seed: u64, opts: &GenOptions) -> anyhow::Result<String> {
    let cfg = identity::Config::load()?;
    let op = Operation::from_name(op_name)?;
    let world = BaseWorld::build(&cfg.template);
    let post = ops::apply(op, &world, seed, &cfg)?;

    let at = 60_000;
    let events = vec![
        header(
            opts,
            format!("m7-adversarial:{op_name}@{seed}"),
            TraceSetup {
                files: vec![TraceSetupFile {
                    path: "notes.md".into(),
                    contents: world.file.clone(),
                }],
                graph: Vec::new(),
            },
        ),
        TraceEvent::FileChangeExternal {
            at,
            path: "notes.md".into(),
            contents: post.file,
            cause_category: CauseCategory::Foreign,
            cause_key: format!("{op_name}@{at}"),
        },
    ];
    ndjson(&events)
}

/// `tracegen import <repo-dir> <out>` (AM-11.5; Algorithm D: "≥1 imported
/// real public-repo history, provenance in header"): walk an EXISTING
/// clone's **first-parent** history oldest-first — the trace models a
/// workspace tracking the default branch, so a merge commit delivers its
/// full mainline diff as ONE `git_op` (side-branch interior commits never
/// touched this workspace's files). Emission mirrors [`git_trace`]: base
/// commit → `setup.files`; commit k → `git_op` + one `file_change_external`
/// per Added/Modified file (same cause pair) at `k × 60_000` ms. Deleted
/// paths stay in the `git_op` files list (they ARE foreign-edit ingestions,
/// Algorithm A) but get no byte delivery — there are no bytes to deliver.
/// Non-UTF-8 blobs are skipped from delivery and setup, with the skip COUNT
/// recorded in the header `source` string (no silent caps). `ext_filter`
/// (lowercased extensions, empty = everything) restricts the imported file
/// set — a corpus-shaping policy choice recorded at assembly time.
///
/// # Errors
/// Git failures, an empty history, or an unwritable diff walk.
#[allow(clippy::too_many_lines)]
pub fn import_trace(
    repo: &Utf8Path,
    source: &str,
    opts: &GenOptions,
    max_commits: Option<usize>,
    ext_filter: &[String],
) -> anyhow::Result<String> {
    use std::fmt::Write as _;
    let in_scope = |path: &str| -> bool {
        ext_filter.is_empty()
            || path
                .rsplit_once('.')
                .is_some_and(|(_, ext)| ext_filter.iter().any(|e| e.eq_ignore_ascii_case(ext)))
    };
    // Raw-bytes `git show`, None for non-UTF-8.
    let show_utf8 = |spec: &str| -> anyhow::Result<Option<String>> {
        let out = std::process::Command::new("git")
            .current_dir(repo)
            .args(["show", spec])
            .output()?;
        anyhow::ensure!(
            out.status.success(),
            "git show {spec} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(String::from_utf8(out.stdout).ok())
    };
    // NUL-separated raw git output: real histories contain filenames with
    // trailing whitespace and non-ASCII bytes (tldr's early history has a
    // committed `ls.md   `), which line-based parsing silently corrupts —
    // `-z` is the only lossless framing. Non-UTF-8 NAMES are skipped and
    // counted like non-UTF-8 contents.
    let git_z = |args: &[&str]| -> anyhow::Result<Vec<String>> {
        let out = std::process::Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()?;
        anyhow::ensure!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(out
            .stdout
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect())
    };

    let mut shas: Vec<String> = git_out(
        repo,
        &["log", "--reverse", "--first-parent", "--format=%H", "HEAD"],
    )?
    .lines()
    .map(str::to_owned)
    .collect();
    anyhow::ensure!(!shas.is_empty(), "empty git history in {repo}");
    if let Some(max) = max_commits {
        shas.truncate(max.max(1));
    }
    let head = shas.last().expect("non-empty").clone();

    let mut skipped_non_utf8 = 0u64;

    // Base commit -> setup.files (full tree, filtered; NUL framing).
    let mut setup_files = Vec::new();
    for path in git_z(&["ls-tree", "-r", "--name-only", "-z", &shas[0]])? {
        if !in_scope(&path) {
            continue;
        }
        match show_utf8(&format!("{}:{path}", shas[0]))? {
            Some(contents) => setup_files.push(TraceSetupFile {
                path: path.clone(),
                contents,
            }),
            None => skipped_non_utf8 += 1,
        }
    }

    let mut events = Vec::new();
    for (k, sha) in shas.iter().enumerate().skip(1) {
        let at = (k as u64) * 60_000;
        let parent_count = git_out(repo, &["log", "-1", "--format=%P", sha])?
            .split_whitespace()
            .count();
        let kind = if parent_count >= 2 {
            GitOpKind::Merge
        } else {
            GitOpKind::Commit
        };
        let cause_key = format!("{}@{at}", kind_wire(kind));

        // Changed vs the first parent, renames as delete+add (--no-renames).
        let mut files = Vec::new();
        let mut deliveries: Vec<(String, String)> = Vec::new();
        // `-z` framing: STATUS NUL PATH NUL … — lossless for dirty names.
        let tokens = git_z(&[
            "diff",
            "--name-status",
            "--no-renames",
            "-z",
            &format!("{sha}^"),
            sha,
        ])?;
        for pair in tokens.chunks_exact(2) {
            let (status, path) = (&pair[0], &pair[1]);
            if !in_scope(path) {
                continue;
            }
            files.push(path.clone());
            if status.starts_with('D') {
                continue; // counted, never delivered — no bytes exist.
            }
            match show_utf8(&format!("{sha}:{path}"))? {
                Some(contents) => deliveries.push((path.clone(), contents)),
                None => skipped_non_utf8 += 1,
            }
        }

        events.push(TraceEvent::GitOp {
            at,
            op: kind,
            files,
            cause_category: CauseCategory::Git,
            cause_key: cause_key.clone(),
        });
        for (path, contents) in deliveries {
            events.push(TraceEvent::FileChangeExternal {
                at,
                path,
                contents,
                cause_category: CauseCategory::Git,
                cause_key: cause_key.clone(),
            });
        }
    }

    let mut full_source = format!(
        "{source} base:{} head:{head} commits:{}",
        shas[0],
        shas.len()
    );
    if skipped_non_utf8 > 0 {
        let _ = write!(full_source, " skipped-non-utf8:{skipped_non_utf8}");
    }
    let mut header_event = header(
        opts,
        full_source,
        TraceSetup {
            files: setup_files,
            graph: Vec::new(),
        },
    );
    if let TraceEvent::TraceHeader { consent, .. } = &mut header_event {
        *consent = Consent::PublicGit;
    }
    let mut all = vec![header_event];
    all.extend(events);
    ndjson(&all)
}
