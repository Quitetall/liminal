//! Self-deleting temporary directories (M17.5 F-12).
//!
//! Every test, harness and throwaway store in this workspace used to build its
//! own scratch path with `std::env::temp_dir().join(format!(...))` and never
//! remove it — 27 such sites across 21 files, producing about 90 distinct
//! directory prefixes. One `cargo test --workspace --all-targets` leaked **250
//! directories, roughly 400 MB**, and nothing ever cleaned them up.
//!
//! That is not untidiness. Three consequences made it load-bearing:
//!
//! 1. **It blocked the mutation campaign.** `cargo mutants --workspace` reports
//!    2915 mutants and runs the suite once per mutant: 2915 × 400 MB ≈ 1.1 TB of
//!    scratch. The campaign exhausts any filesystem long before it finishes, and
//!    the HAQP-1 kill-rate measurement cannot be taken without it.
//! 2. **It took `just ci` down twice**, with `No space left on device` on tests
//!    that pass in isolation. A suite whose result depends on how many times it
//!    has been run before is the same defect class as F-11.
//! 3. `/tmp` is tmpfs here, so the leak is resident in RAM.
//!
//! # Retention policy
//!
//! Deleting scratch unconditionally would throw away exactly the evidence a
//! failed replay needs, which is presumably why nothing deleted it. So:
//!
//! - **the test failed** — retained. [`ScratchDir::drop`] checks
//!   [`std::thread::panicking`], so a panicking test keeps its whole file world
//!   for post-mortem. This is the common case for debugging and needs no flag,
//!   no foresight, and no rerun.
//! - **`LIMINAL_KEEP_SCRATCH` is set** — retained, for watching a *passing* run.
//! - **otherwise** — deleted.
//!
//! Nothing that could be wanted from a failure is discarded, and success does
//! not accumulate. Directory paths are always printed when retained, so a
//! failing run says where its evidence went.
//!
//! # Known limit
//!
//! `Drop` does not run when a process is killed or aborts. The crash-injection
//! suite deliberately raises `SIGABRT`, so those few directories still leak by
//! design — a handful per run, against the 250 this removes. Cleaning them would
//! require the parent to own the path, which is a larger change than F-12 needs.

use std::sync::atomic::{AtomicUsize, Ordering};

use camino::{Utf8Path, Utf8PathBuf};

/// Set to any non-empty value to keep scratch directories from passing runs.
pub const KEEP_ENV: &str = "LIMINAL_KEEP_SCRATCH";

/// Distinguishes directories created within one process in the same
/// millisecond; the pid and timestamp separate processes from each other.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A temporary directory that removes itself unless the test failed.
///
/// Derefs to [`Utf8Path`], so it can be passed anywhere a path is expected.
/// Keep the value alive for as long as the directory is needed: dropping it is
/// what deletes the directory.
#[derive(Debug)]
pub struct ScratchDir {
    path: Utf8PathBuf,
    keep: bool,
}

impl ScratchDir {
    /// Create a fresh scratch directory tagged with `label`.
    ///
    /// # Errors
    /// If the directory cannot be created.
    pub fn new(label: &str) -> std::io::Result<Self> {
        let base = Utf8PathBuf::from_path_buf(std::env::temp_dir()).map_err(|path| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("temp dir is not UTF-8: {}", path.display()),
            )
        })?;
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = base.join(format!(
            "liminal-{label}-{}-{unique}-{nanos:x}",
            std::process::id()
        ));
        // A previous run that aborted before `Drop` could have left this exact
        // path behind only if pid, counter and nanosecond all collided; clear it
        // anyway so a stale world can never be mistaken for a fresh one.
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path)?;
        Ok(Self { path, keep: false })
    }

    /// The directory's path.
    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Retain this directory even if the run succeeds.
    pub fn keep(&mut self) {
        self.keep = true;
    }

    /// Give up ownership, returning the path and leaving the directory behind.
    ///
    /// For the rare caller that must outlive the guard. Reintroduces the leak
    /// for that one directory, so prefer holding the [`ScratchDir`].
    #[must_use]
    pub fn into_path(mut self) -> Utf8PathBuf {
        self.keep = true;
        self.path.clone()
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // A panicking test keeps its evidence. This is the whole reason the
        // directories were never deleted in the first place, and it costs
        // nothing to preserve.
        if self.keep || std::thread::panicking() || std::env::var_os(KEEP_ENV).is_some() {
            eprintln!("scratch retained: {}", self.path);
            return;
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

impl std::ops::Deref for ScratchDir {
    type Target = Utf8Path;
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl std::fmt::Display for ScratchDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

impl AsRef<Utf8Path> for ScratchDir {
    fn as_ref(&self) -> &Utf8Path {
        &self.path
    }
}

impl AsRef<std::path::Path> for ScratchDir {
    fn as_ref(&self) -> &std::path::Path {
        self.path.as_std_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_successful_run_leaves_nothing_behind() {
        let path = {
            let dir = ScratchDir::new("selftest-clean").expect("create");
            std::fs::write(dir.join("world.txt"), b"content").expect("write");
            assert!(dir.path().is_dir());
            dir.path().to_owned()
        };
        assert!(
            !path.exists(),
            "scratch survived a passing run at {path}; the F-12 leak is back"
        );
    }

    /// The retention half. If this stopped holding, a failing test would delete
    /// the file world needed to diagnose it — which is why nothing deleted
    /// scratch before F-12, and the property that makes deletion safe.
    #[test]
    fn a_failing_run_keeps_its_evidence() {
        let captured = std::sync::Mutex::new(None);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dir = ScratchDir::new("selftest-panic").expect("create");
            std::fs::write(dir.join("world.txt"), b"evidence").expect("write");
            *captured.lock().expect("lock") = Some(dir.path().to_owned());
            panic!("deliberate failure, so `Drop` runs while panicking");
        }));
        assert!(result.is_err(), "the closure must panic");

        let path = captured
            .lock()
            .expect("lock")
            .clone()
            .expect("the scratch path was recorded before the panic");
        assert!(
            path.is_dir(),
            "scratch at {path} was deleted while the thread was panicking, \
             discarding the evidence a failed replay exists to provide"
        );
        std::fs::remove_dir_all(&path).expect("clean up the selftest");
    }

    #[test]
    fn distinct_calls_get_distinct_directories() {
        let a = ScratchDir::new("selftest-unique").expect("a");
        let b = ScratchDir::new("selftest-unique").expect("b");
        assert_ne!(a.path(), b.path());
    }
}
