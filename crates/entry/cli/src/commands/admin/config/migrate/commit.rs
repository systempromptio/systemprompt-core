//! All-or-nothing replacement of the live services tree by a staged one.
//!
//! [`commit`] copies every staged file to a temporary sibling of its target
//! first; a failure there removes the temporaries and leaves the tree as it
//! was. Only once all of them exist does [`swap`](super::swap) replace the live
//! files, rolling every replacement back if one step fails. The whole commit
//! holds the run's interrupt shield, so a forced exit waits for it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use super::swap::{FileSwap, Step, swap};
use crate::runner::interrupt::InterruptShield;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChangeKind {
    Write,
    Remove,
}

#[derive(Debug)]
pub(super) struct Change {
    pub relative: PathBuf,
    pub kind: ChangeKind,
}

struct Staged {
    temp: PathBuf,
    target: PathBuf,
}

pub(super) fn commit(
    staging: &Path,
    services: &Path,
    changes: &[Change],
    shield: &InterruptShield,
    fs_ops: &dyn FileSwap,
) -> Result<Vec<PathBuf>> {
    let _critical = shield.hold();
    let mut staged: Vec<Staged> = Vec::new();
    for (index, change) in changes.iter().enumerate() {
        if change.kind == ChangeKind::Remove {
            continue;
        }
        let target = services.join(&change.relative);
        match stage(&staging.join(&change.relative), &target, index) {
            Ok(temp) => staged.push(Staged { temp, target }),
            Err(e) => {
                discard(&staged);
                return Err(e.context("the migration was not applied; nothing was changed"));
            },
        }
    }
    let steps: Vec<Step> = staged
        .iter()
        .map(|item| (item.target.clone(), Some(item.temp.clone())))
        .chain(
            changes
                .iter()
                .filter(|c| c.kind == ChangeKind::Remove)
                .map(|c| (services.join(&c.relative), None)),
        )
        .enumerate()
        .map(|(index, (target, staged))| Step {
            backup: sibling(&target, index, "bak"),
            target,
            staged,
        })
        .collect();
    swap(fs_ops, &steps)
}

fn sibling(target: &Path, index: usize, suffix: &str) -> PathBuf {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
    dir.join(format!(
        ".{name}.migrate-{}-{index}.{suffix}",
        std::process::id()
    ))
}

fn stage(source: &Path, target: &Path, index: usize) -> Result<PathBuf> {
    let body = fs::read(source).with_context(|| format!("Failed to read {}", source.display()))?;
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let temp = sibling(target, index, "tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .with_context(|| format!("Failed to stage {}", target.display()))?;
    if let Err(e) = file.write_all(&body).and_then(|()| file.sync_all()) {
        discard(&[Staged {
            temp,
            target: target.to_path_buf(),
        }]);
        return Err(anyhow!(e).context(format!("Failed to stage {}", target.display())));
    }
    Ok(temp)
}

fn discard(staged: &[Staged]) {
    for item in staged {
        if let Err(e) = fs::remove_file(&item.temp) {
            tracing::warn!(path = %item.temp.display(), error = %e, "staged file not removed");
        }
    }
}
