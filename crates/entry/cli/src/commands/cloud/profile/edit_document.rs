//! The raw profile document `cloud profile edit` writes back.
//!
//! An edit works on the profile as written, never on an interpolated copy:
//! prompts show each field's authored text (a `${VAR}` placeholder included),
//! and only a field whose answer differs from that text is written into the
//! overlay (never the shared `base.yaml`). Every other value, placeholders
//! included, is saved exactly as authored, so a profile whose variables exist
//! only on its target can be edited on any host. Before writing, the result is
//! loaded as a [`ManagedProfile`]: a profile with no unresolved value must load
//! whole, and every edited field must resolve.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_yaml::{Mapping, Value};
use systemprompt_manifest::profile::ManagedProfile;
use systemprompt_manifest::profile::overlay::write_overlay_document;

use crate::shared::yaml_document::read_yaml_file;

/// The authored YAML of one profile and the fields an edit changed.
#[derive(Debug)]
pub struct ProfileEdit {
    path: PathBuf,
    overlay: Value,
    view: ManagedProfile,
    changed: Vec<String>,
}

impl ProfileEdit {
    pub fn open(path: &Path) -> Result<Self> {
        let view = ManagedProfile::load(path)
            .with_context(|| format!("Failed to load profile: {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            overlay: read_yaml_file(path)?,
            view,
            changed: Vec::new(),
        })
    }

    pub fn text(&self, key_path: &str) -> Result<String> {
        let authored = self.view.raw_section::<Value>(key_path)?;
        Ok(match authored {
            Some(Value::String(text)) => text,
            Some(Value::Number(number)) => number.to_string(),
            Some(Value::Bool(flag)) => flag.to_string(),
            Some(other) => serde_yaml::to_string(&other)?.trim_end().to_owned(),
            None => String::new(),
        })
    }

    pub fn set(&mut self, key_path: &str, answer: &str, value: Value) -> Result<bool> {
        if answer == self.text(key_path)? {
            return Ok(false);
        }
        set_at(&mut self.overlay, key_path, value);
        self.changed.push(key_path.to_owned());
        Ok(true)
    }

    #[must_use]
    pub fn changed(&self) -> &[String] {
        &self.changed
    }

    pub fn save(&self) -> Result<()> {
        if self.changed.is_empty() {
            return Ok(());
        }
        let content =
            serde_yaml::to_string(&self.overlay).context("Failed to serialize profile")?;
        let edited = ManagedProfile::from_yaml(&content, &self.path)
            .with_context(|| format!("Edited profile is invalid: {}", self.path.display()))?;
        if edited.unresolved().is_empty() {
            edited
                .profile()
                .with_context(|| format!("Edited profile is invalid: {}", self.path.display()))?;
        }
        for key_path in &self.changed {
            edited
                .section::<Value>(key_path)
                .with_context(|| format!("Edited field `{key_path}` is invalid"))?;
        }
        write_overlay_document(&self.overlay, &self.path)
            .with_context(|| format!("Failed to write {}", self.path.display()))
    }
}

fn set_at(root: &mut Value, key_path: &str, value: Value) {
    let mut node = root;
    for key in key_path.split('.') {
        if !node.is_mapping() {
            *node = Value::Mapping(Mapping::new());
        }
        let Value::Mapping(map) = node else {
            return;
        };
        node = map
            .entry(Value::String(key.to_owned()))
            .or_insert(Value::Null);
    }
    *node = value;
}

pub fn parse_number<T>(answer: &str, what: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    answer
        .trim()
        .parse()
        .with_context(|| format!("Invalid {what}: {answer}"))
}
