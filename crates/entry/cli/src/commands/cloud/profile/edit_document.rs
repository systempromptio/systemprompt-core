//! The raw profile document `cloud profile edit` writes back.
//!
//! Prompts and flags edit the interpolated [`Profile`] (so the operator sees
//! real values), but saving that struct would materialise every `${VAR}`
//! placeholder, collapse `${VAR:-default}` to its default and replace
//! relative paths with resolved ones. [`ProfileDocument`] instead holds the
//! file as authored and receives only the leaves the edit changed; the result
//! is validated by parsing a separately interpolated copy before it is
//! written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_yaml::Value;
use systemprompt_manifest::Profile;

use crate::commands::admin::config::config_section::read_yaml_file;

/// The authored YAML of one profile, edited by difference.
#[derive(Debug)]
pub struct ProfileDocument {
    path: PathBuf,
    raw: Value,
}

impl ProfileDocument {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            raw: read_yaml_file(path)?,
        })
    }

    pub fn apply_changes(&mut self, before: &Profile, after: &Profile) -> Result<()> {
        let before = serde_yaml::to_value(before).context("Failed to serialize profile")?;
        let after = serde_yaml::to_value(after).context("Failed to serialize profile")?;
        merge_changed_leaves(&mut self.raw, &before, &after);
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let content = serde_yaml::to_string(&self.raw).context("Failed to serialize profile")?;
        Profile::from_yaml(&content, &self.path)
            .with_context(|| format!("Edited profile is invalid: {}", self.path.display()))?;
        std::fs::write(&self.path, content)
            .with_context(|| format!("Failed to write {}", self.path.display()))
    }
}

fn merge_changed_leaves(raw: &mut Value, before: &Value, after: &Value) {
    let (Some(before_map), Some(after_map), Some(raw_map)) = (
        before.as_mapping(),
        after.as_mapping(),
        raw.as_mapping_mut(),
    ) else {
        if before != after {
            *raw = after.clone();
        }
        return;
    };

    for (key, after_value) in after_map {
        let Some(before_value) = before_map.get(key) else {
            raw_map.insert(key.clone(), after_value.clone());
            continue;
        };
        if before_value == after_value {
            continue;
        }
        if let Some(slot) = raw_map.get_mut(key) {
            merge_changed_leaves(slot, before_value, after_value);
        } else {
            raw_map.insert(key.clone(), after_value.clone());
        }
    }
    for key in before_map.keys() {
        if !after_map.contains_key(key) {
            raw_map.remove(key);
        }
    }
}
