//! `lim-toy` — a ~20-line argv shim for the conformance crash matrix (AM-2.4).
//!
//! `lim-toy exec <root> <scenario>` / `lim-toy recover <root>`.
//! Delegates to `liminal_daemon::runner::{exec, recover}`.
//!
//! This binary exists because `CARGO_BIN_EXE_*` is only injected for targets
//! owned by the compiling crate (D02.6). The conformance harness needs a
//! binary it can spawn with `env!("CARGO_BIN_EXE_lim-toy")`.

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("exec") => {
            let root = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("usage: lim-toy exec <root> <scenario>"))?;
            let scenario_path = args
                .get(3)
                .ok_or_else(|| anyhow::anyhow!("usage: lim-toy exec <root> <scenario>"))?;
            let root = camino::Utf8PathBuf::from(root);
            let scenario = liminal_daemon::scenario::ScenarioScript::load(
                &camino::Utf8PathBuf::from(scenario_path),
            )?;
            let executor = liminal_daemon::FsExecutor::new(root.clone());
            let crash = liminal_daemon::EnvCrashInjector::from_env();
            liminal_daemon::runner::exec(&root, &scenario, executor, crash)?;
            Ok(())
        }
        Some("recover") => {
            let root = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("usage: lim-toy recover <root>"))?;
            let root = camino::Utf8PathBuf::from(root);
            let outcome = liminal_daemon::runner::recover(&root)?;
            let json = serde_json::to_string(&outcome)?;
            println!("{json}");
            Ok(())
        }
        _ => {
            eprintln!("usage: lim-toy exec <root> <scenario>");
            eprintln!("       lim-toy recover <root>");
            std::process::exit(1);
        }
    }
}
