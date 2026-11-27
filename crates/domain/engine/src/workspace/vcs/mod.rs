//! Git facts read straight from the repository files: the current branch, the
//! head commit and a dirty flag, without spawning git.
//!
//! The dirty check compares each index entry's recorded size and mtime with the
//! file on disk and looks for files that are neither tracked nor ignored. It
//! does not read objects, so a change that is staged but leaves the working
//! tree identical to the index is not reported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod index;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use std::{fs, io};

use ignore::WalkBuilder;

use crate::workspace::paths::{fold, path_for_key, rel_key_under};
use index::{IndexEntry, MODE_GITLINK, MODE_SYMLINK};

const MAX_SYMREF_DEPTH: usize = 5;
const SHA1_LEN: usize = 20;
const SHA256_LEN: usize = 32;

/// Failures reading a repository.
#[derive(Debug, thiserror::Error)]
pub enum VcsError {
    #[error("could not read {}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("the git index is corrupt")]
    CorruptIndex,
    #[error("git index version {0} is not supported")]
    UnsupportedIndex(u32),
}

/// Where a repository is and what it has checked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcsInfo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
}

/// Repository facts plus the dirty flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcsStatus {
    pub info: VcsInfo,
    pub dirty: bool,
}

impl VcsInfo {
    pub fn discover(dir: &Path) -> Result<Option<Self>, VcsError> {
        for candidate in dir.ancestors() {
            let dot_git = candidate.join(".git");
            let Ok(metadata) = fs::symlink_metadata(&dot_git) else {
                continue;
            };
            let git_dir = if metadata.is_dir() {
                dot_git
            } else if metadata.is_file() {
                let text = read_text(&dot_git)?;
                let Some(target) = text.trim().strip_prefix("gitdir:") else {
                    continue;
                };
                candidate.join(target.trim())
            } else {
                continue;
            };
            if !git_dir.join("HEAD").is_file() {
                continue;
            }
            return Self::read(candidate.to_path_buf(), git_dir).map(Some);
        }
        Ok(None)
    }

    pub fn status(dir: &Path) -> Result<Option<VcsStatus>, VcsError> {
        let Some(info) = Self::discover(dir)? else {
            return Ok(None);
        };
        let dirty = info.is_dirty()?;
        Ok(Some(VcsStatus { info, dirty }))
    }

    pub const fn is_detached(&self) -> bool {
        self.branch.is_none() && self.head.is_some()
    }

    pub fn is_dirty(&self) -> Result<bool, VcsError> {
        let index_path = self.git_dir.join("index");
        let entries = match fs::read(&index_path) {
            Ok(bytes) => index::parse(&bytes, self.hash_len())?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(source) => {
                return Err(VcsError::Io {
                    path: index_path,
                    source,
                });
            },
        };
        let mut tracked: HashSet<String> = HashSet::with_capacity(entries.len());
        let mut submodules: HashSet<String> = HashSet::new();
        for entry in &entries {
            if entry.kind() == MODE_GITLINK {
                submodules.insert(fold(&entry.path));
                continue;
            }
            tracked.insert(fold(&entry.path));
            if self.entry_changed(entry) {
                tracing::trace!(path = %entry.path, "tracked file differs from the index");
                return Ok(true);
            }
        }
        Ok(self.first_untracked(&tracked, &submodules).is_some())
    }

    fn read(root: PathBuf, git_dir: PathBuf) -> Result<Self, VcsError> {
        let common_dir = fs::read_to_string(git_dir.join("commondir"))
            .map_or_else(|_| git_dir.clone(), |text| git_dir.join(text.trim()));
        let head_text = read_text(&git_dir.join("HEAD"))?;
        let head_text = head_text.trim();
        let (branch, head) = head_text.strip_prefix("ref:").map_or_else(
            || (None, is_object_id(head_text).then(|| head_text.to_owned())),
            |reference| {
                let reference = reference.trim();
                let branch = reference
                    .strip_prefix("refs/heads/")
                    .unwrap_or(reference)
                    .to_owned();
                (Some(branch), resolve_ref(&git_dir, &common_dir, reference))
            },
        );
        Ok(Self {
            root,
            git_dir,
            common_dir,
            branch,
            head,
        })
    }

    fn hash_len(&self) -> usize {
        let path = self.common_dir.join("config");
        let config = match fs::read_to_string(&path) {
            Ok(config) => config,
            Err(error) => {
                tracing::debug!(path = %path.display(), %error, "no readable repository config");
                return SHA1_LEN;
            },
        };
        let sha256 = config.lines().any(|line| {
            let line = line.trim().to_ascii_lowercase();
            line.starts_with("objectformat") && line.contains("sha256")
        });
        if sha256 { SHA256_LEN } else { SHA1_LEN }
    }

    fn entry_changed(&self, entry: &IndexEntry) -> bool {
        if entry.stage != 0 {
            return true;
        }
        if entry.assume_valid || entry.skip_worktree {
            return false;
        }
        let Some(path) = path_for_key(&self.root, &entry.path) else {
            return false;
        };
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            return true;
        };
        if entry.kind() == MODE_SYMLINK {
            return false;
        }
        if metadata.is_dir() {
            return true;
        }
        // Why: git records sizes and seconds truncated to 32 bits.
        let size = (metadata.len() & u64::from(u32::MAX)) as u32;
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|since| (since.as_secs() & u64::from(u32::MAX)) as u32);
        size != entry.size || mtime.is_some_and(|secs| secs != entry.mtime_secs)
    }

    fn first_untracked(
        &self,
        tracked: &HashSet<String>,
        submodules: &HashSet<String>,
    ) -> Option<String> {
        let root = self.root.clone();
        let skip = submodules.clone();
        let mut builder = WalkBuilder::new(&self.root);
        builder
            .hidden(false)
            .parents(false)
            .ignore(false)
            .git_ignore(true)
            .git_exclude(true)
            .git_global(false)
            .require_git(false)
            .follow_links(false)
            .filter_entry(move |entry| {
                if entry.file_name().eq_ignore_ascii_case(".git") {
                    return false;
                }
                rel_key_under(&root, entry.path()).is_none_or(|key| !skip.contains(&fold(&key)))
            });
        builder
            .build()
            .flatten()
            .filter(|entry| entry.file_type().is_some_and(|kind| !kind.is_dir()))
            .filter_map(|entry| rel_key_under(&self.root, entry.path()))
            .find(|key| !tracked.contains(&fold(key)))
            .inspect(|key| tracing::trace!(path = %key, "untracked file"))
    }
}

fn read_text(path: &Path) -> Result<String, VcsError> {
    fs::read_to_string(path).map_err(|source| VcsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn resolve_ref(git_dir: &Path, common_dir: &Path, reference: &str) -> Option<String> {
    let mut name = reference.to_owned();
    for _ in 0..MAX_SYMREF_DEPTH {
        let loose = [git_dir, common_dir]
            .iter()
            .filter_map(|dir| path_for_key(dir, &name))
            .find_map(|path| fs::read_to_string(path).ok());
        match loose {
            Some(text) => {
                let text = text.trim();
                match text.strip_prefix("ref:") {
                    Some(next) => next.trim().clone_into(&mut name),
                    None => return is_object_id(text).then(|| text.to_owned()),
                }
            },
            None => return packed_ref(common_dir, &name),
        }
    }
    None
}

fn packed_ref(common_dir: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(common_dir.join("packed-refs")).ok()?;
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with('^'))
        .find_map(|line| {
            let (id, reference) = line.split_once(' ')?;
            (reference.trim() == name && is_object_id(id)).then(|| id.to_owned())
        })
}

fn is_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}
