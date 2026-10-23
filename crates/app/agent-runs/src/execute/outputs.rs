//! The run's output files: what the workspace diff marks added or modified,
//! copied to the run's `outputs/` so they outlive the workspace.
//!
//! At most [`MAX_OUTPUT_FILES`] files and [`MAX_OUTPUT_BYTES`] in total are
//! kept, in diff order; the rest are reported as skipped, never silently
//! dropped. The kept list (path, size, sha256) is written once beside the
//! copies as [`OUTPUTS_MANIFEST`], which [`list`] reads; [`read`] serves a
//! copy by relative path and refuses a path that leaves `outputs/`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::diff::{DiffKind, DiffSummary};
use crate::error::{AgentRunsError, AgentRunsResult};
use crate::layout::RunLayout;

pub const MAX_OUTPUT_FILES: usize = 64;
pub const MAX_OUTPUT_BYTES: u64 = 32 * 1024 * 1024;
pub const OUTPUTS_MANIFEST: &str = "outputs.json";

/// One kept output file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// What [`capture`] kept and what it skipped for the bounds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputCapture {
    pub kept: Vec<OutputFile>,
    pub skipped: Vec<String>,
}

pub async fn capture(
    workspace: &Path,
    diff: &DiffSummary,
    layout: &RunLayout,
) -> AgentRunsResult<OutputCapture> {
    let mut capture = OutputCapture::default();
    let mut total = 0u64;
    for entry in &diff.entries {
        if entry.kind == DiffKind::Deleted {
            continue;
        }
        let Some(relative) = contained(&entry.path) else {
            capture.skipped.push(entry.path.clone());
            continue;
        };
        let source = workspace.join(&relative);
        let bytes = match tokio::fs::read(&source).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source_error) => return Err(layout_error(&source, source_error)),
        };
        let size = bytes.len() as u64;
        if capture.kept.len() >= MAX_OUTPUT_FILES || total + size > MAX_OUTPUT_BYTES {
            capture.skipped.push(entry.path.clone());
            continue;
        }
        let target = layout.outputs.join(&relative);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| layout_error(parent, e))?;
        }
        tokio::fs::write(&target, &bytes)
            .await
            .map_err(|e| layout_error(&target, e))?;
        total += size;
        capture.kept.push(OutputFile {
            path: entry.path.clone(),
            size,
            sha256: hex::encode(Sha256::digest(&bytes)),
        });
    }
    let index = manifest_path(layout);
    let json = serde_json::to_vec(&capture.kept)?;
    tokio::fs::write(&index, json)
        .await
        .map_err(|e| layout_error(&index, e))?;
    Ok(capture)
}

fn manifest_path(layout: &RunLayout) -> PathBuf {
    layout.root.join(OUTPUTS_MANIFEST)
}

pub async fn list(layout: &RunLayout) -> AgentRunsResult<Vec<OutputFile>> {
    let index = manifest_path(layout);
    match tokio::fs::read(&index).await {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(layout_error(&index, error)),
    }
}

pub async fn read(layout: &RunLayout, path: &str) -> AgentRunsResult<Option<Vec<u8>>> {
    let Some(relative) = contained(path) else {
        return Ok(None);
    };
    let target = layout.outputs.join(relative);
    match tokio::fs::symlink_metadata(&target).await {
        Ok(meta) if meta.is_file() => {},
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(layout_error(&target, error)),
    }
    tokio::fs::read(&target)
        .await
        .map(Some)
        .map_err(|e| layout_error(&target, e))
}

fn contained(path: &str) -> Option<PathBuf> {
    let candidate = Path::new(path);
    candidate
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
        .then(|| candidate.to_path_buf())
        .filter(|p| !p.as_os_str().is_empty())
}

fn layout_error(path: &Path, source: std::io::Error) -> AgentRunsError {
    AgentRunsError::Layout {
        path: path.to_path_buf(),
        source,
    }
}
