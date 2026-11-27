//! Turning parsed file operations into planned writes: every path resolved,
//! every file read and every hunk applied in memory, so nothing is written
//! unless the whole patch applies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::path::PathBuf;

use super::apply::apply_hunks;
use super::parse::{FileOp, Hunk};
use crate::tools::contract::ToolContext;
use crate::tools::error::ToolError;
use crate::tools::files::read_existing;
use crate::tools::path::{display_path, resolve};
use crate::tools::text::TextFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Change {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug)]
pub(super) struct Planned {
    pub(super) change: Change,
    pub(super) path: PathBuf,
    pub(super) shown: String,
    pub(super) target: Option<(PathBuf, String)>,
    pub(super) old: String,
    pub(super) new: String,
    pub(super) bytes: Vec<u8>,
}

impl Planned {
    pub(super) fn shown_after(&self) -> &str {
        self.target
            .as_ref()
            .map_or(self.shown.as_str(), |(_, shown)| shown.as_str())
    }

    pub(super) fn summary(&self) -> String {
        match (&self.change, &self.target) {
            (Change::Added, _) => format!("A {}", self.shown),
            (Change::Deleted, _) => format!("D {}", self.shown),
            (Change::Modified, Some((_, to))) => format!("R {} -> {to}", self.shown),
            (Change::Modified, None) => format!("M {}", self.shown),
        }
    }
}

pub(super) async fn plan(ops: Vec<FileOp>, ctx: &ToolContext) -> Result<Vec<Planned>, ToolError> {
    let mut plans: Vec<Planned> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for op in ops {
        let planned = match op {
            FileOp::Add { path, content } => plan_add(&path, content, ctx).await?,
            FileOp::Delete { path } => plan_delete(&path, ctx).await?,
            FileOp::Update {
                path,
                rename_to,
                hunks,
            } => plan_update(&path, rename_to.as_deref(), &hunks, ctx).await?,
        };
        let claimed =
            std::iter::once(&planned.path).chain(planned.target.as_ref().map(|(target, _)| target));
        for path in claimed {
            if !seen.insert(path.clone()) {
                return Err(ToolError::invalid(format!(
                    "{} appears more than once in the patch",
                    display_path(path, &ctx.worktree)
                )));
            }
        }
        plans.push(planned);
    }
    Ok(plans)
}

async fn plan_add(path: &str, content: String, ctx: &ToolContext) -> Result<Planned, ToolError> {
    let path = resolve(&ctx.cwd, path);
    let shown = display_path(&path, &ctx.worktree);
    if tokio::fs::try_exists(&path).await.unwrap_or(false) {
        return Err(ToolError::invalid(format!(
            "{shown} already exists; use an update section to change it"
        )));
    }
    let bytes = content.clone().into_bytes();
    Ok(Planned {
        change: Change::Added,
        path,
        shown,
        target: None,
        old: String::new(),
        new: content,
        bytes,
    })
}

async fn plan_delete(path: &str, ctx: &ToolContext) -> Result<Planned, ToolError> {
    let path = resolve(&ctx.cwd, path);
    let shown = display_path(&path, &ctx.worktree);
    let bytes = read_existing(&path)
        .await?
        .ok_or_else(|| ToolError::invalid(format!("cannot delete {shown}: file not found")))?;
    let old = String::from_utf8_lossy(&bytes).into_owned();
    Ok(Planned {
        change: Change::Deleted,
        path,
        shown,
        target: None,
        old,
        new: String::new(),
        bytes: Vec::new(),
    })
}

async fn plan_update(
    path: &str,
    rename_to: Option<&str>,
    hunks: &[Hunk],
    ctx: &ToolContext,
) -> Result<Planned, ToolError> {
    let path = resolve(&ctx.cwd, path);
    let shown = display_path(&path, &ctx.worktree);
    let bytes = read_existing(&path)
        .await?
        .ok_or_else(|| ToolError::invalid(format!("cannot update {shown}: file not found")))?;
    let file = TextFile::decode(&bytes).ok_or_else(|| {
        ToolError::invalid(format!("{shown} is not UTF-8 text and cannot be patched"))
    })?;
    let new = if hunks.is_empty() {
        file.body.clone()
    } else {
        apply_hunks(&file.body, hunks, &shown).map_err(|err| ToolError::malformed("", err))?
    };
    let target = match rename_to {
        Some(to) => {
            let target = resolve(&ctx.cwd, to);
            let target_shown = display_path(&target, &ctx.worktree);
            if target != path && tokio::fs::try_exists(&target).await.unwrap_or(false) {
                return Err(ToolError::invalid(format!(
                    "cannot rename {shown} to {target_shown}: it already exists"
                )));
            }
            Some((target, target_shown))
        },
        None => None,
    };
    let encoded = file.encode(&new);
    Ok(Planned {
        change: Change::Modified,
        path,
        shown,
        target,
        old: file.body,
        new,
        bytes: encoded,
    })
}
