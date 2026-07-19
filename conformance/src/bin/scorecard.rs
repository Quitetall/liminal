//! `scorecard` — the §7.4 / R4 §2.3 conformance scorecard runner (AM-11.2;
//! D11.6: a conformance bin, never a `lim` subcommand).
//!
//! ```text
//! scorecard --corpus <dir> [--profile <name>]
//! ```
//!
//! Runs the M11 Algorithm C pipeline over `<dir>` for each requested
//! profile (default: both Phase -1 stable profiles) and prints one JSON
//! object per profile: the nine frozen `Scorecard` fields, the R4 §2.3
//! `violations` list, and the side `TransientDraftReport`. Exit 0 = the
//! MEASUREMENT succeeded; threshold violations are printed, not fatal —
//! M11.8 records a below-threshold score rather than suppressing it
//! (v4 §7.4: the profile stays experimental; the score is retained).

use camino::Utf8PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut corpus: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--corpus" => corpus = it.next(),
            "--profile" => profile = it.next(),
            other => anyhow::bail!(
                "unknown arg {other:?}\nusage: scorecard --corpus <dir> [--profile <name>]"
            ),
        }
    }
    let corpus =
        Utf8PathBuf::from(corpus.ok_or_else(|| {
            anyhow::anyhow!("usage: scorecard --corpus <dir> [--profile <name>]")
        })?);

    let profiles: Vec<String> = match profile {
        Some(p) => vec![p],
        None => vec!["external-file".into(), "graph-native".into()],
    };

    for profile in &profiles {
        let (scorecard, drafts) = liminal_conformance::pipeline::run(&corpus, profile)?;
        let violations = scorecard.violations();
        let out = serde_json::json!({
            "scorecard": scorecard,
            "violations": violations,
            "transient_drafts": drafts,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    }
    Ok(())
}
