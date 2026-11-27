//! File access on behalf of a tool: reads that tolerate absence, writes that
//! tell the observer before and after, and the shared permission question for
//! edits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::json;

use crate::tools::contract::{PermissionRequest, ToolContext};
use crate::tools::error::ToolError;

pub(crate) async fn read_existing(path: &Path) -> Result<Option<Vec<u8>>, ToolError> {
    match tokio::fs::read(path).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(ToolError::io(
            format!("could not read {}", path.display()),
            err,
        )),
    }
}

pub(crate) async fn write_file(
    ctx: &ToolContext,
    path: &Path,
    bytes: &[u8],
) -> Result<Option<String>, ToolError> {
    if let Some(observer) = &ctx.observer {
        observer.before_write(path).await;
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| ToolError::io(format!("could not create {}", parent.display()), err))?;
    }
    tokio::fs::write(path, bytes)
        .await
        .map_err(|err| ToolError::io(format!("could not write {}", path.display()), err))?;
    tracing::debug!(path = %path.display(), bytes = bytes.len(), "tool wrote file");
    Ok(after_write(ctx, path).await)
}

pub(crate) async fn remove_file(
    ctx: &ToolContext,
    path: &Path,
) -> Result<Option<String>, ToolError> {
    if let Some(observer) = &ctx.observer {
        observer.before_write(path).await;
    }
    tokio::fs::remove_file(path)
        .await
        .map_err(|err| ToolError::io(format!("could not delete {}", path.display()), err))?;
    tracing::debug!(path = %path.display(), "tool deleted file");
    Ok(after_write(ctx, path).await)
}

async fn after_write(ctx: &ToolContext, path: &Path) -> Option<String> {
    match &ctx.observer {
        Some(observer) => observer
            .after_write(path)
            .await
            .filter(|note| !note.trim().is_empty()),
        None => None,
    }
}

pub(crate) fn with_notes(output: String, notes: &[String]) -> String {
    if notes.is_empty() {
        return output;
    }
    let mut out = output;
    for note in notes {
        out.push_str("\n\n");
        out.push_str(note.trim_end());
    }
    out
}

pub(crate) async fn ask_edit(
    ctx: &ToolContext,
    paths: Vec<String>,
    title: String,
    diff: &str,
) -> Result<(), ToolError> {
    ctx.ask(PermissionRequest {
        permission: "edit".to_owned(),
        patterns: paths.clone(),
        always: vec!["*".to_owned()],
        title,
        metadata: json!({ "files": paths, "diff": diff }),
    })
    .await
}
