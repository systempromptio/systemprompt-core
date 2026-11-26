//! Owner-only directories and files for the documents that hold tokens and
//! credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use crate::workspace::error::WorkspaceResult;

fn ensure_private_dir(dir: &Path) -> WorkspaceResult<()> {
    fs::create_dir_all(dir)?;
    let gitignore_path = dir.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(&gitignore_path, "*\n")?;
    }
    Ok(())
}

pub fn write_private_json<T: serde::Serialize>(path: &Path, value: &T) -> WorkspaceResult<()> {
    if let Some(dir) = path.parent() {
        ensure_private_dir(dir)?;
    }
    let content = serde_json::to_string_pretty(value)?;
    systemprompt_config::write_private_atomic(path, content.as_bytes())?;
    Ok(())
}
