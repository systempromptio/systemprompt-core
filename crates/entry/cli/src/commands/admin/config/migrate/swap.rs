//! The swap phase of `config migrate`: staged files replace the live ones,
//! and any failure puts every replaced file back.
//!
//! Each live file is first renamed to a backup beside it, then its staged
//! copy is renamed into place; a file the plan drops is only moved to its
//! backup. When a step fails, every completed step is undone in reverse and
//! the remaining staged copies are removed, so the tree is either fully
//! migrated or as it was. Backups are deleted only once every step
//! succeeded. If undoing itself fails, the error names each file still
//! wrong and the backup that holds its old content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::anyhow;

/// The renames and removals the swap performs, so a caller can supply them.
pub trait FileSwap {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove(&self, path: &Path) -> io::Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StdFileSwap;

impl FileSwap for StdFileSwap {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
}

pub(super) struct Step {
    pub target: PathBuf,
    pub staged: Option<PathBuf>,
    pub backup: PathBuf,
}

struct Done<'a> {
    step: &'a Step,
    backed_up: bool,
    placed: bool,
}

pub(super) fn swap(fs_ops: &dyn FileSwap, steps: &[Step]) -> anyhow::Result<Vec<PathBuf>> {
    let mut done: Vec<Done<'_>> = Vec::new();
    for (position, step) in steps.iter().enumerate() {
        let mut current = Done {
            step,
            backed_up: false,
            placed: false,
        };
        let outcome = apply_step(fs_ops, &mut current);
        done.push(current);
        if let Err(cause) = outcome {
            discard_staged(fs_ops, &steps[position + 1..]);
            return Err(roll_back(fs_ops, &done, &step.target, cause));
        }
    }
    for item in &done {
        if item.backed_up
            && let Err(e) = fs_ops.remove(&item.step.backup)
        {
            tracing::warn!(path = %item.step.backup.display(), error = %e, "backup not removed");
        }
    }
    Ok(steps.iter().map(|s| s.target.clone()).collect())
}

fn apply_step(fs_ops: &dyn FileSwap, current: &mut Done<'_>) -> io::Result<()> {
    let step = current.step;
    if step.target.symlink_metadata().is_ok() {
        fs_ops.rename(&step.target, &step.backup)?;
        current.backed_up = true;
    }
    if let Some(staged) = &step.staged {
        fs_ops.rename(staged, &step.target)?;
        current.placed = true;
    }
    Ok(())
}

fn discard_staged(fs_ops: &dyn FileSwap, steps: &[Step]) {
    for staged in steps.iter().filter_map(|s| s.staged.as_ref()) {
        if let Err(e) = fs_ops.remove(staged) {
            tracing::warn!(path = %staged.display(), error = %e, "staged file not removed");
        }
    }
}

fn undo(fs_ops: &dyn FileSwap, item: &Done<'_>) -> io::Result<()> {
    let step = item.step;
    if item.backed_up {
        fs_ops.rename(&step.backup, &step.target)?;
    } else if item.placed {
        fs_ops.remove(&step.target)?;
    }
    if !item.placed
        && let Some(staged) = &step.staged
        && let Err(e) = fs_ops.remove(staged)
    {
        tracing::warn!(path = %staged.display(), error = %e, "staged file not removed");
    }
    Ok(())
}

fn roll_back(
    fs_ops: &dyn FileSwap,
    done: &[Done<'_>],
    failed: &Path,
    cause: io::Error,
) -> anyhow::Error {
    let stranded: Vec<String> = done
        .iter()
        .rev()
        .filter_map(|item| {
            undo(fs_ops, item).err().map(|e| {
                format!(
                    "{} (old content kept at {}: {e})",
                    item.step.target.display(),
                    item.step.backup.display()
                )
            })
        })
        .collect();
    if stranded.is_empty() {
        return anyhow!(cause).context(format!(
            "the migration was not applied; replacing {} failed and every file was restored, so \
             nothing was changed",
            failed.display()
        ));
    }
    anyhow!(cause).context(format!(
        "the migration failed at {} and these files could not be restored: [{}]. Move each \
         backup back over its file, then re-run the migration",
        failed.display(),
        stranded.join("; ")
    ))
}
