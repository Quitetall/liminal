//! Env-armed crash injection at ILRP durable boundaries (R4 §10).
//!
//! Arming: `LIMINAL_CRASHPOINT=<point>[:<occurrence>]`, e.g.
//! `ilrp/after_ack:2` aborts on the second acknowledgement boundary.
//! Occurrence counting (1-based) handles per-step boundaries without name
//! explosion.
//!
//! Every hit — armed or not — is appended to the file named by
//! `LIMINAL_CRASH_TRACE` (O_APPEND, one boundary name per line) so the harness
//! can (a) derive the crash matrix from a baseline run's trace and (b) prove a
//! crash test actually reached its boundary: a crash test whose point never
//! fired is a false pass.
//!
//! Death is `std::process::abort()`: a real SIGABRT with no unwinding, no
//! destructors, no user-space buffer flushes — fsync honesty is actually
//! tested.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::sync::Mutex;

use liminal_jurisdiction::{CrashInjector, CrashPoint};

/// Environment variable that arms a crash: `<point>[:<occurrence>]`.
pub const CRASHPOINT_ENV: &str = "LIMINAL_CRASHPOINT";
/// Environment variable naming the hit-trace file.
pub const CRASH_TRACE_ENV: &str = "LIMINAL_CRASH_TRACE";

/// The real injector used by `liminald`.
#[derive(Debug)]
pub struct EnvCrashInjector {
    armed: Option<(String, u64)>,
    trace_path: Option<String>,
    hits: Mutex<Vec<(&'static str, u64)>>,
}

impl EnvCrashInjector {
    /// Read arming state from the environment.
    #[must_use]
    pub fn from_env() -> Self {
        let armed = std::env::var(CRASHPOINT_ENV)
            .ok()
            .map(|spec| match spec.split_once(':') {
                Some((name, n)) => {
                    let occurrence = n.parse::<u64>().unwrap_or(1).max(1);
                    (name.to_owned(), occurrence)
                }
                None => (spec, 1),
            });
        Self {
            armed,
            trace_path: std::env::var(CRASH_TRACE_ENV).ok(),
            hits: Mutex::new(Vec::new()),
        }
    }

    /// How many times `point` has fired so far in this process (including the
    /// current hit when called from inside `crash_if_armed`).
    fn bump(&self, point: CrashPoint) -> u64 {
        let mut hits = self
            .hits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let name = point.name();
        let count = hits.iter().filter(|(n, _)| *n == name).count() as u64 + 1;
        hits.push((name, count));
        count
    }

    fn trace(&self, point: CrashPoint, occurrence: u64) {
        if let Some(path) = &self.trace_path
            && let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path)
        {
            // O_APPEND single write; sync so the line survives the abort below.
            let _ = writeln!(f, "{}:{occurrence}", point.name());
            let _ = f.sync_all();
        }
    }
}

impl CrashInjector for EnvCrashInjector {
    fn crash_if_armed(&self, at: CrashPoint) {
        let occurrence = self.bump(at);
        self.trace(at, occurrence);
        if let Some((armed_name, armed_occurrence)) = &self.armed
            && armed_name == at.name()
            && *armed_occurrence == occurrence
        {
            // Real death at the durable boundary: no unwinding, no flushes.
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unarmed_injector_traces_without_dying() {
        // The trace is a FILE, so it lives inside a scratch dir that removes
        // itself (M17.5 F-12) rather than being written straight into /tmp.
        let scratch = liminal_scratch::ScratchDir::new("crash-trace").expect("scratch dir");
        let dir = scratch.join("crash-trace.log");
        let injector = EnvCrashInjector {
            armed: None,
            trace_path: Some(dir.to_string()),
            hits: Mutex::new(Vec::new()),
        };
        injector.crash_if_armed(CrashPoint::BeforeIntentCommit);
        injector.crash_if_armed(CrashPoint::AfterIntentCommit);
        injector.crash_if_armed(CrashPoint::BeforeExternalApply);
        injector.crash_if_armed(CrashPoint::AfterExternalApply);
        injector.crash_if_armed(CrashPoint::AfterExternalApply);
        let trace = std::fs::read_to_string(&dir).unwrap();
        assert_eq!(
            trace.lines().collect::<Vec<_>>(),
            vec![
                "ilrp/before_intent_commit:1",
                "ilrp/after_intent_commit:1",
                "ilrp/before_external_apply:1",
                "ilrp/after_external_apply:1",
                "ilrp/after_external_apply:2",
            ],
            "hit trace records every boundary with its occurrence count"
        );
    }
}
