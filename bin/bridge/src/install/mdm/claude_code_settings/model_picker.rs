//! `modelPicker` rows for Claude Code: the gateway models its own discovery
//! would hide.
//!
//! Claude Code's gateway model discovery keeps only ids containing `claude` or
//! `anthropic`, so every other model the gateway serves — Gemini, Vertex — is
//! reachable with `--model <id>` yet invisible in `/model`. The bridge learns
//! the catalog from the bridge profile's provider health on each sync and
//! writes `modelPicker.options` rows into the settings files it owns. Claude
//! ids are left to discovery, which carries their real pricing and labels.
//!
//! This is Claude Code only. Claude Desktop's `inferenceModels` policy must
//! stay Anthropic-only (`crate::install::mdm::policy` enforces it); Desktop
//! breaks on other model families.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    io_error, managed_settings_path, read_or_empty, render, standalone_settings_path, write_atomic,
};
use crate::gateway::types::ProviderHealth;
use crate::install::mdm::MdmError;

mod merge;

pub use merge::merged_picker;

const SIDECAR: &str = "claude-code-model-picker.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickerRow {
    pub id: String,
    pub label: String,
}

#[must_use]
pub fn is_claude_family(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    lower.contains("claude") || lower.contains("anthropic")
}

#[must_use]
pub fn picker_rows(providers: &[ProviderHealth]) -> Vec<PickerRow> {
    let mut ids: Vec<&str> = providers
        .iter()
        .filter(|p| p.configured)
        .flat_map(|p| p.models.iter().map(String::as_str))
        .filter(|id| !is_claude_family(id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter()
        .map(|id| PickerRow {
            id: id.to_owned(),
            label: label_for(id),
        })
        .collect()
}

fn label_for(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for (i, part) in id.split('-').filter(|p| !p.is_empty()).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            out.extend(c.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

fn sidecar_path() -> Option<PathBuf> {
    crate::config::paths::bridge_metadata_dir().map(|d| d.join(SIDECAR))
}

// Why: the sidecar is the only record of which picker rows the bridge wrote;
// a read or parse failure must surface, or those rows are orphaned in the
// user's settings rather than replaced.
fn read_sidecar() -> Result<Vec<String>, MdmError> {
    let Some(path) = sidecar_path() else {
        return Ok(Vec::new());
    };
    let body = read_or_empty(&path)?;
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&body).map_err(|source| MdmError::Json { path, source })
}

fn write_sidecar(ids: &[String]) -> Result<(), MdmError> {
    let path = sidecar_path().ok_or(MdmError::Resolve("the bridge metadata directory"))?;
    let body = serde_json::to_string_pretty(ids).map_err(|source| MdmError::Json {
        path: path.clone(),
        source,
    })?;
    write_atomic(&path, &format!("{body}\n"))
}

fn read_json_object(
    path: &Path,
) -> Result<Option<serde_json::Map<String, serde_json::Value>>, MdmError> {
    let existing = read_or_empty(path)?;
    if existing.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&existing)
        .map(Some)
        .map_err(|source| MdmError::Json {
            path: path.to_path_buf(),
            source,
        })
}

fn splice_rows(
    root: &mut serde_json::Map<String, serde_json::Value>,
    previously_ours: &[String],
    rows: &[PickerRow],
) {
    match merged_picker(root.get("modelPicker"), previously_ours, rows) {
        Some(picker) => {
            root.insert("modelPicker".to_owned(), picker);
        },
        None => {
            root.remove("modelPicker");
        },
    }
}

pub(crate) fn apply_model_picker(rows: &[PickerRow]) -> Result<Vec<String>, MdmError> {
    let previously = read_sidecar()?;
    let mut lines = Vec::new();
    let standalone =
        standalone_settings_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    if let Some(mut root) = read_json_object(&standalone)? {
        splice_rows(&mut root, &previously, rows);
        write_atomic(&standalone, &render(root, &standalone)?)?;
        lines.push(format!(
            "wrote: {} (modelPicker, {} gateway model(s))",
            standalone.display(),
            rows.len()
        ));
    }
    let settings = managed_settings_path().ok_or(MdmError::Resolve("the managed settings path"))?;
    if let Some(mut root) = read_json_object(&settings)?
        && root.contains_key("apiKeyHelper")
    {
        splice_rows(&mut root, &previously, rows);
        write_atomic(&settings, &render(root, &settings)?)?;
        lines.push(format!("wrote: {} (modelPicker)", settings.display()));
    }
    write_sidecar(&rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>())?;
    Ok(lines)
}

pub(super) fn strip_owned_rows(
    root: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), MdmError> {
    let ours = read_sidecar()?;
    if !ours.is_empty() {
        splice_rows(root, &ours, &[]);
    }
    Ok(())
}

pub(super) fn remove_sidecar() -> Result<(), MdmError> {
    let Some(path) = sidecar_path() else {
        return Ok(());
    };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error("remove", &path)(e)),
    }
}
