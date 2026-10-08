//! `env.SLASH_COMMAND_TOOL_CHAR_BUDGET` for Claude Code: the skill listing
//! budget a marketplace declares.
//!
//! Claude Code lists every installed skill in one tool description and cuts
//! descriptions once the listing passes its character budget (8,000 by
//! default), after which the model stops loading the truncated skills on its
//! own. A marketplace declares the budget it needs
//! (`claude_code.skill_listing_budget_chars`); the bridge writes the largest
//! one across the manifest's marketplaces into the settings files it owns on
//! every sync. The value it wrote is recorded in a sidecar so it can be taken
//! back out when no marketplace declares a budget; a larger value the user set
//! is kept, and a value the bridge did not write is never removed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    io_error, managed_settings_path, read_or_empty, render, standalone_settings_path, write_atomic,
};
use crate::gateway::manifest::SignedManifest;
use crate::install::mdm::MdmError;

pub const BUDGET_ENV: &str = "SLASH_COMMAND_TOOL_CHAR_BUDGET";

const SIDECAR: &str = "claude-code-skill-budget.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RecordedBudget {
    value: String,
}

#[must_use]
pub fn budget_for(manifest: &SignedManifest) -> Option<u32> {
    manifest
        .marketplaces
        .iter()
        .filter_map(|m| m.claude_code.as_ref()?.skill_listing_budget_chars)
        .filter(|chars| *chars > 0)
        .max()
}

// JSON: Claude Code `settings.json` — foreign config, unknown keys must be
// preserved.
#[must_use]
pub fn splice_budget(
    root: &mut serde_json::Map<String, serde_json::Value>,
    previously_ours: Option<&str>,
    budget: Option<u32>,
) -> bool {
    if root.get("env").is_some_and(|env| !env.is_object()) {
        return false;
    }
    let existing = root
        .get("env")
        .and_then(|env| env.get(BUDGET_ENV))
        .map(|value| value.as_str().map(str::to_owned).unwrap_or_default());
    let ours = existing.is_some() && existing.as_deref() == previously_ours;
    match budget {
        Some(chars) => {
            let user_value_covers = !ours
                && existing
                    .as_deref()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .is_some_and(|value| value >= u64::from(chars));
            if user_value_covers {
                return false;
            }
            let env = root
                .entry("env".to_owned())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let Some(env) = env.as_object_mut() {
                env.insert(
                    BUDGET_ENV.to_owned(),
                    serde_json::Value::String(chars.to_string()),
                );
            }
            true
        },
        None if ours => {
            if let Some(env) = root
                .get_mut("env")
                .and_then(serde_json::Value::as_object_mut)
            {
                env.remove(BUDGET_ENV);
                if env.is_empty() {
                    root.remove("env");
                }
            }
            true
        },
        None => false,
    }
}

fn sidecar_path() -> Option<PathBuf> {
    crate::config::paths::bridge_metadata_dir().map(|d| d.join(SIDECAR))
}

// Why: the sidecar is the only record of the value the bridge wrote; a read
// or parse failure must surface, or that value is orphaned in the user's
// settings rather than replaced.
fn read_sidecar() -> Result<Option<String>, MdmError> {
    let Some(path) = sidecar_path() else {
        return Ok(None);
    };
    let body = read_or_empty(&path)?;
    if body.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<RecordedBudget>(&body)
        .map(|recorded| Some(recorded.value))
        .map_err(|source| MdmError::Json { path, source })
}

fn write_sidecar(budget: Option<u32>) -> Result<(), MdmError> {
    let Some(chars) = budget else {
        return remove_sidecar();
    };
    let path = sidecar_path().ok_or(MdmError::Resolve("the bridge metadata directory"))?;
    let recorded = RecordedBudget {
        value: chars.to_string(),
    };
    let body = serde_json::to_string_pretty(&recorded).map_err(|source| MdmError::Json {
        path: path.clone(),
        source,
    })?;
    write_atomic(&path, &format!("{body}\n"))
}

// JSON: Claude Code `settings.json` — foreign config, unknown keys must be
// preserved.
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

fn splice_file(
    path: &Path,
    previously: Option<&str>,
    budget: Option<u32>,
    require_helper: bool,
) -> Result<Option<String>, MdmError> {
    let Some(mut root) = read_json_object(path)? else {
        return Ok(None);
    };
    if require_helper && !root.contains_key("apiKeyHelper") {
        return Ok(None);
    }
    if !splice_budget(&mut root, previously, budget) {
        return Ok(None);
    }
    write_atomic(path, &render(root, path)?)?;
    Ok(Some(format!("wrote: {} ({BUDGET_ENV})", path.display())))
}

pub(crate) fn apply_skill_budget(budget: Option<u32>) -> Result<Vec<String>, MdmError> {
    let previously = read_sidecar()?;
    let standalone =
        standalone_settings_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    let settings = managed_settings_path().ok_or(MdmError::Resolve("the managed settings path"))?;
    // Why: only a settings file the bridge already configured (it holds our
    // apiKeyHelper) is ours to add the budget to; a stranger's file is left
    // alone.
    let lines = [
        splice_file(&standalone, previously.as_deref(), budget, false)?,
        splice_file(&settings, previously.as_deref(), budget, true)?,
    ]
    .into_iter()
    .flatten()
    .collect();
    write_sidecar(budget)?;
    Ok(lines)
}

// JSON: Claude Code `settings.json` — foreign config, unknown keys must be
// preserved.
pub(super) fn strip_owned_budget(
    root: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), MdmError> {
    let Some(ours) = read_sidecar()? else {
        return Ok(());
    };
    if !splice_budget(root, Some(&ours), None) {
        tracing::debug!(
            target: "bridge::claude-code-settings",
            "skill listing budget left in place: not the value the bridge wrote"
        );
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
