//! `tracegen` — the five M11 Algorithm B trace importers/generators
//! (AM-11.3; D11.6: a conformance bin, never a `lim` subcommand).
//!
//! ```text
//! tracegen git         <op-script>      <out> [--profile <p>]
//! tracegen session     <session.toml>   <out> [--profile <p>]
//! tracegen formatter   <fixture-dir>    <out> [--profile <p>]
//! tracegen offline                      <out> [--profile <p>]
//! tracegen adversarial <m7-op> <seed>   <out> [--profile <p>]
//! ```
//!
//! `--profile` (default `external-file`) sets the header's `profile` field —
//! Algorithm D populates the dev corpus per profile. The header `trace_id`
//! derives from the output file stem (`x.trace.ndjson` → `x`).

use camino::Utf8PathBuf;
use liminal_conformance::tracegen::{
    GenOptions, adversarial_trace, formatter_trace, git_trace, import_trace, offline_trace,
    session_trace,
};

const USAGE: &str = "usage: tracegen git <op-script> <out> [--profile <p>]
       tracegen session <session.toml> <out> [--profile <p>]
       tracegen formatter <fixture-dir> <out> [--profile <p>]
       tracegen offline <out> [--profile <p>]
       tracegen adversarial <m7-op> <seed> <out> [--profile <p>]
       tracegen import <repo-dir> <out> --source <provenance> [--max-commits <n>] [--ext md,txt] [--profile <p>]";

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // Extract --profile <p> wherever it appears.
    let mut profile = "external-file".to_owned();
    if let Some(i) = args.iter().position(|a| a == "--profile") {
        anyhow::ensure!(i + 1 < args.len(), "--profile requires a value\n{USAGE}");
        profile = args.remove(i + 1);
        args.remove(i);
    }

    let mut source = String::new();
    if let Some(i) = args.iter().position(|a| a == "--source") {
        anyhow::ensure!(i + 1 < args.len(), "--source requires a value\n{USAGE}");
        source = args.remove(i + 1);
        args.remove(i);
    }
    let mut max_commits: Option<usize> = None;
    if let Some(i) = args.iter().position(|a| a == "--max-commits") {
        anyhow::ensure!(
            i + 1 < args.len(),
            "--max-commits requires a value\n{USAGE}"
        );
        max_commits = Some(args.remove(i + 1).parse()?);
        args.remove(i);
    }
    let mut ext_filter: Vec<String> = Vec::new();
    if let Some(i) = args.iter().position(|a| a == "--ext") {
        anyhow::ensure!(i + 1 < args.len(), "--ext requires a value\n{USAGE}");
        ext_filter = args
            .remove(i + 1)
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        args.remove(i);
    }

    let sub = args.first().cloned().unwrap_or_default();
    let positional = &args[1.min(args.len())..];

    let (out_path, ndjson) = match (sub.as_str(), positional) {
        ("git", [script, out]) => (out.clone(), git_trace(script, &opts(out, &profile))?),
        ("session", [toml, out]) => (
            out.clone(),
            session_trace(&Utf8PathBuf::from(toml), &opts(out, &profile))?,
        ),
        ("formatter", [dir, out]) => (
            out.clone(),
            formatter_trace(&Utf8PathBuf::from(dir), &opts(out, &profile))?,
        ),
        ("offline", [out]) => (out.clone(), offline_trace(&opts(out, &profile))?),
        ("adversarial", [op, seed, out]) => (
            out.clone(),
            adversarial_trace(op, seed.parse()?, &opts(out, &profile))?,
        ),
        ("import", [repo, out]) => {
            anyhow::ensure!(
                !source.is_empty(),
                "import requires --source <provenance>\n{USAGE}"
            );
            (
                out.clone(),
                import_trace(
                    &Utf8PathBuf::from(repo),
                    &source,
                    &opts(out, &profile),
                    max_commits,
                    &ext_filter,
                )?,
            )
        }
        _ => anyhow::bail!("{USAGE}"),
    };

    let out = Utf8PathBuf::from(out_path);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out, ndjson)?;
    Ok(())
}

/// Build `GenOptions` from the out path (trace_id = stem minus `.trace`).
fn opts(out: &str, profile: &str) -> GenOptions {
    let stem = Utf8PathBuf::from(out)
        .file_name()
        .unwrap_or("trace")
        .trim_end_matches(".ndjson")
        .trim_end_matches(".trace")
        .to_owned();
    GenOptions {
        profile: profile.to_owned(),
        trace_id: stem,
    }
}
