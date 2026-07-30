//! Hash-verified staged writes (v4 §7.8 step 2) and file observation.

use std::fs;
use std::io::{self, Write as _};

use camino::{Utf8Path, Utf8PathBuf};
use liminal_id::{ContentHash, PathId, Timestamp};

/// Suffix for staged sibling files. Deterministic (not `tempfile`) so crash
/// recovery can scan for abandoned staging (R4 §7.5 Recover).
const STAGED_SUFFIX: &str = ".lim-staged";

/// An observed file state: the prestate/poststate evidence unit of ILRP file
/// steps (v4 §7.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileObservation {
    /// Observed path.
    pub path: PathId,
    /// BLAKE3 hash of the full contents.
    pub hash: ContentHash,
    /// Length in bytes.
    pub len: u64,
    /// Observation wall time (never part of deterministic replay, v4 §110).
    pub at: Timestamp,
}

/// Observe a file's current state. `Ok(None)` if the file does not exist.
pub fn observe(path: &Utf8Path) -> io::Result<Option<FileObservation>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(FileObservation {
            path: PathId(path.to_owned()),
            hash: ContentHash::of(&bytes),
            len: bytes.len() as u64,
            at: Timestamp::now(),
        })),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// A staged write: content flushed to `<path>.lim-staged`, not yet visible at
/// the target path (v4 §7.8 step 2: "produce staged content, flush staged
/// content, use atomic replacement where the platform supports it").
#[derive(Debug)]
pub struct StagedWrite {
    staged: Utf8PathBuf,
    target: Utf8PathBuf,
    hash: ContentHash,
}

impl StagedWrite {
    /// The staged sibling path.
    #[must_use]
    pub fn staged_path(&self) -> &Utf8Path {
        &self.staged
    }

    /// The target path this staging will replace.
    #[must_use]
    pub fn target_path(&self) -> &Utf8Path {
        &self.target
    }

    /// Hash of the staged content.
    #[must_use]
    pub fn content_hash(&self) -> ContentHash {
        self.hash
    }

    /// Atomically rename onto the target iff the target's current bytes hash to
    /// `expected_pre` (`None` = the target must not exist). This is exactly
    /// ILRP Apply for a file mutation (v4 §7.8 step 2).
    ///
    /// On Unix the replacement is an atomic `rename(2)`. On platforms without
    /// atomic replace over an existing file, a documented remove-then-rename
    /// fallback is used ("atomic replacement where the platform supports it",
    /// v4 §7.8 — never claimed stronger, per §92: "never claim multi-file
    /// atomicity").
    pub fn commit_if(
        self,
        expected_pre: Option<ContentHash>,
    ) -> Result<FileObservation, StageError> {
        let found = observe(&self.target)?.map(|o| o.hash);
        if found != expected_pre {
            return Err(StageError::PrestateMismatch {
                expected: expected_pre,
                found,
            });
        }

        #[cfg(unix)]
        fs::rename(&self.staged, &self.target)?;
        #[cfg(not(unix))]
        {
            if self.target.exists() {
                fs::remove_file(&self.target)?;
            }
            fs::rename(&self.staged, &self.target)?;
        }

        sync_parent_dir(&self.target)?;
        let observed = observe(&self.target)?.ok_or_else(|| {
            StageError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "target vanished after atomic replacement",
            ))
        })?;
        Ok(observed)
    }

    /// Remove the staged file without touching the target.
    pub fn abandon(self) -> io::Result<()> {
        fs::remove_file(&self.staged)
    }
}

/// Stage `contents` next to `path` and flush to disk. The target is untouched.
pub fn stage(path: &Utf8Path, contents: &[u8]) -> io::Result<StagedWrite> {
    let staged = Utf8PathBuf::from(format!("{path}{STAGED_SUFFIX}"));
    let mut f = fs::File::create(&staged)?;
    f.write_all(contents)?;
    f.sync_all()?;
    Ok(StagedWrite {
        staged,
        target: path.to_owned(),
        hash: ContentHash::of(contents),
    })
}

/// Recovery sweep: find abandoned staged files under `root` after a crash
/// (R4 §7.5 Recover). Recursive; ignores unreadable entries rather than
/// aborting the sweep.
pub fn scan_staged(root: &Utf8Path) -> io::Result<Vec<StagedWrite>> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_owned()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                continue; // non-UTF-8 paths are outside the toy's world
            };
            if path.is_dir() {
                stack.push(path);
            } else if let Some(target) = path.as_str().strip_suffix(STAGED_SUFFIX) {
                let hash = match fs::read(&path) {
                    Ok(bytes) => ContentHash::of(&bytes),
                    Err(_) => continue,
                };
                found.push(StagedWrite {
                    staged: path.clone(),
                    target: Utf8PathBuf::from(target),
                    hash,
                });
            }
        }
    }
    Ok(found)
}

fn sync_parent_dir(path: &Utf8Path) -> io::Result<()> {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = path; // directory fsync is not portable off Unix
    Ok(())
}

/// Staged-write failure.
#[derive(Debug, thiserror::Error)]
pub enum StageError {
    /// The target's observed state did not match the expected prestate; the
    /// repair step must not proceed (ILRP never guesses, v4 §7.8 step 5).
    #[error("prestate mismatch: expected {expected:?}, found {found:?}")]
    PrestateMismatch {
        /// Hash the plan expected (`None` = absent).
        expected: Option<ContentHash>,
        /// Hash actually observed (`None` = absent).
        found: Option<ContentHash>,
    },
    /// Underlying I/O failure.
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory that removes itself (M17.5 F-12).
    fn tmp_dir(name: &str) -> liminal_scratch::ScratchDir {
        liminal_scratch::ScratchDir::new(&format!("source-test-{name}")).expect("scratch dir")
    }

    #[test]
    fn stage_then_commit_creates_target_atomically() {
        let dir = tmp_dir("commit");
        let target = dir.join("note.md");
        let staged = stage(&target, b"hello").unwrap();
        assert!(!target.exists(), "staging must not touch the target");
        let obs = staged.commit_if(None).unwrap();
        assert_eq!(obs.hash, ContentHash::of(b"hello"));
        assert_eq!(fs::read(&target).unwrap(), b"hello");
    }

    #[test]
    fn commit_with_wrong_prestate_is_refused() {
        let dir = tmp_dir("prestate");
        let target = dir.join("note.md");
        fs::write(&target, b"original").unwrap();
        let staged = stage(&target, b"replacement").unwrap();
        // Expect an absent file — but one exists: must refuse, never guess.
        let err = staged.commit_if(None).unwrap_err();
        assert!(matches!(err, StageError::PrestateMismatch { .. }));
        assert_eq!(fs::read(&target).unwrap(), b"original", "target untouched");
    }

    #[test]
    fn commit_over_expected_prestate_replaces() {
        let dir = tmp_dir("replace");
        let target = dir.join("note.md");
        fs::write(&target, b"v1").unwrap();
        let staged = stage(&target, b"v2").unwrap();
        let obs = staged.commit_if(Some(ContentHash::of(b"v1"))).unwrap();
        assert_eq!(obs.hash, ContentHash::of(b"v2"));
    }

    #[test]
    fn observe_missing_file_returns_none() {
        let dir = tmp_dir("observe-missing");
        let target = dir.join("missing.md");
        assert_eq!(observe(&target).unwrap(), None);
    }

    #[test]
    fn observe_propagates_non_not_found_errors() {
        let dir = tmp_dir("observe-directory");
        let err = observe(&dir).expect_err("reading a directory must fail");
        assert_eq!(err.kind(), io::ErrorKind::IsADirectory);
    }

    #[test]
    fn abandon_removes_staged_file_without_touching_target() {
        let dir = tmp_dir("abandon");
        let target = dir.join("note.md");
        let staged = stage(&target, b"discarded").unwrap();
        let staged_path = staged.staged_path().to_owned();
        staged.abandon().unwrap();
        assert!(!staged_path.exists());
        assert!(!target.exists());
    }

    #[test]
    fn scan_finds_abandoned_staging() {
        let dir = tmp_dir("scan");
        let target = dir.join("nested").join("note.md");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let _abandoned = stage(&target, b"crashed mid-repair").unwrap();
        let found = scan_staged(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].target_path(), target);
        assert_eq!(
            found[0].content_hash(),
            ContentHash::of(b"crashed mid-repair")
        );
    }
}
