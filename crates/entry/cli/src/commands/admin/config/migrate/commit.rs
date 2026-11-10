//! All-or-nothing replacement of the live services tree by a staged one.
//!
//! [`commit`] copies every staged file to a temporary sibling of its target
//! first; only once all of them exist are they renamed into place, and the
//! files the plan drops are removed last. A failure while staging removes the
//! temporaries and leaves the tree as it was. A rename or removal that fails
//! part-way names the files already replaced and the ones still old.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

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

pub(super) fn commit(staging: &Path, services: &Path, changes: &[Change]) -> Result<Vec<PathBuf>> {
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
    let removals: Vec<PathBuf> = changes
        .iter()
        .filter(|c| c.kind == ChangeKind::Remove)
        .map(|c| services.join(&c.relative))
        .collect();
    let order: Vec<PathBuf> = staged
        .iter()
        .map(|s| s.target.clone())
        .chain(removals.iter().cloned())
        .collect();

    for (position, item) in staged.iter().enumerate() {
        if let Err(e) = fs::rename(&item.temp, &item.target) {
            discard(&staged[position..]);
            return Err(partial(&order, position, e));
        }
    }
    for (offset, target) in removals.iter().enumerate() {
        if let Err(e) = fs::remove_file(target) {
            return Err(partial(&order, staged.len() + offset, e));
        }
    }
    Ok(order)
}

fn stage(source: &Path, target: &Path, index: usize) -> Result<PathBuf> {
    let body = fs::read(source).with_context(|| format!("Failed to read {}", source.display()))?;
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let name = target
        .file_name()
        .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
    let temp = dir.join(format!(
        ".{name}.migrate-{}-{index}.tmp",
        std::process::id()
    ));
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

fn partial(order: &[PathBuf], failed_at: usize, cause: std::io::Error) -> anyhow::Error {
    let list = |paths: &[PathBuf]| {
        paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    anyhow!(cause).context(format!(
        "the migration was applied only in part: {} failed. Replaced: [{}]. Still in the old \
         layout: [{}]. Restore the services tree from version control, or fix the cause and \
         re-run the migration",
        order[failed_at].display(),
        list(&order[..failed_at]),
        list(&order[failed_at..]),
    ))
}
