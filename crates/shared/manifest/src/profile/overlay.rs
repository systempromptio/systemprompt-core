//! Writing a profile edit back to its overlay, never to the base.
//!
//! A setter loads the layered profile, mutates a typed field and hands the
//! result here. Only the leaves that differ from what is on disk are written
//! into the raw overlay document, so the overlay keeps its own keys and
//! `${VAR}` references, and the shared base is never touched. A new overlay
//! gets every key that differs from the base.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_yaml::{Mapping, Value};

use super::layered::{load_base_layer, read_layer};
use super::{Profile, ProfileError, ProfileResult};

pub fn apply_changes(overlay: &mut Value, before: &Value, after: &Value) {
    if before == after {
        return;
    }
    let (Value::Mapping(before_map), Value::Mapping(after_map)) = (before, after) else {
        *overlay = after.clone();
        return;
    };
    if !overlay.is_mapping() {
        *overlay = Value::Mapping(Mapping::new());
    }
    let Value::Mapping(overlay_map) = overlay else {
        return;
    };
    for (key, after_value) in after_map {
        let before_value = before_map.get(key).cloned().unwrap_or(Value::Null);
        let slot = overlay_map.entry(key.clone()).or_insert(Value::Null);
        apply_changes(slot, &before_value, after_value);
    }
    for (key, before_value) in before_map {
        if !after_map.contains_key(key) {
            overlay_map.insert(key.clone(), cleared(before_value));
        }
    }
}

fn cleared(before: &Value) -> Value {
    match before {
        Value::Sequence(_) => Value::Sequence(Vec::new()),
        Value::Mapping(_) => Value::Mapping(Mapping::new()),
        _ => Value::Null,
    }
}

fn strip_equal_to_base(value: Value, base: &Value) -> Option<Value> {
    if &value == base {
        return None;
    }
    match (value, base) {
        (Value::Mapping(map), Value::Mapping(base_map)) => {
            let kept: Mapping = map
                .into_iter()
                .filter_map(|(key, child)| match base_map.get(&key) {
                    Some(base_child) => strip_equal_to_base(child, base_child).map(|c| (key, c)),
                    None => Some((key, child)),
                })
                .collect();
            Some(Value::Mapping(kept))
        },
        (value, _) => Some(value),
    }
}

fn to_value(profile: &Profile) -> ProfileResult<Value> {
    serde_yaml::to_value(profile).map_err(ProfileError::SerializeYaml)
}

fn write_document(
    document: &Value,
    overlay_path: &Path,
    header: Option<&str>,
) -> ProfileResult<()> {
    let yaml = serde_yaml::to_string(document).map_err(ProfileError::SerializeYaml)?;
    let content = header.map_or_else(|| yaml.clone(), |h| format!("{h}\n\n{yaml}"));
    if let Some(parent) = overlay_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ProfileError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(overlay_path, content).map_err(|source| ProfileError::WriteFile {
        path: overlay_path.to_path_buf(),
        source,
    })
}

impl Profile {
    pub fn overlay_document(&self, overlay_path: &Path) -> ProfileResult<Value> {
        if !overlay_path.is_file() {
            return self.new_overlay_document(overlay_path);
        }
        let after = to_value(self)?;
        let before = to_value(&Self::load_from_path(overlay_path)?)?;
        let raw = read_layer(overlay_path)?;
        let mut overlay: Value =
            serde_yaml::from_str(&raw).map_err(|source| ProfileError::ParseYaml {
                path: overlay_path.to_path_buf(),
                source,
            })?;
        apply_changes(&mut overlay, &before, &after);
        Ok(overlay)
    }

    pub fn new_overlay_document(&self, overlay_path: &Path) -> ProfileResult<Value> {
        let after = to_value(self)?;
        let base = load_base_layer(overlay_path)?.unwrap_or(Value::Null);
        Ok(strip_equal_to_base(after, &base).unwrap_or_else(|| Value::Mapping(Mapping::new())))
    }

    pub fn save_overlay(&self, overlay_path: &Path, header: Option<&str>) -> ProfileResult<()> {
        write_document(&self.overlay_document(overlay_path)?, overlay_path, header)
    }

    pub fn create_overlay(&self, overlay_path: &Path, header: Option<&str>) -> ProfileResult<()> {
        write_document(
            &self.new_overlay_document(overlay_path)?,
            overlay_path,
            header,
        )
    }
}
