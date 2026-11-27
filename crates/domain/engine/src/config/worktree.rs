//! The project root: the nearest ancestor holding `.git`, found by looking at
//! the file system only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

/// The root of the project containing a working directory, and its git
/// directory when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub root: PathBuf,
    pub git_dir: Option<PathBuf>,
}

impl Worktree {
    pub fn detect(cwd: &Path) -> Self {
        for dir in cwd.ancestors() {
            let dot_git = dir.join(".git");
            let Ok(meta) = fs::metadata(&dot_git) else {
                continue;
            };
            if meta.is_dir() {
                return Self {
                    root: dir.to_path_buf(),
                    git_dir: Some(dot_git),
                };
            }
            if meta.is_file() {
                let git_dir = linked_git_dir(dir, &dot_git).unwrap_or(dot_git);
                return Self {
                    root: dir.to_path_buf(),
                    git_dir: Some(git_dir),
                };
            }
        }
        Self {
            root: cwd.to_path_buf(),
            git_dir: None,
        }
    }

    pub const fn is_git(&self) -> bool {
        self.git_dir.is_some()
    }

    pub fn chain(&self, cwd: &Path) -> Vec<PathBuf> {
        if !cwd.starts_with(&self.root) {
            return vec![cwd.to_path_buf()];
        }
        let mut chain: Vec<PathBuf> = Vec::new();
        for dir in cwd.ancestors() {
            chain.push(dir.to_path_buf());
            if dir == self.root {
                break;
            }
        }
        chain.reverse();
        chain
    }
}

fn linked_git_dir(dir: &Path, dot_git: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(dot_git).ok()?;
    let target = text
        .lines()
        .find_map(|line| line.strip_prefix("gitdir:"))?
        .trim();
    if target.is_empty() {
        return None;
    }
    let target = Path::new(target);
    Some(if target.is_absolute() {
        target.to_path_buf()
    } else {
        dir.join(target)
    })
}
