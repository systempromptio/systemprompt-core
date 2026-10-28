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
        if &before_value == after_value {
            continue;
        }
        let slot = overlay_map
            .entry(key.clone())
            .or_insert_with(|| Value::Mapping(Mapping::new()));
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
    let parent = overlay_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| write_error(parent, source))?;
    let file_name = overlay_path
        .file_name()
        .ok_or_else(|| ProfileError::InvalidProfilePath {
            path: overlay_path.to_path_buf(),
        })?;
    let mut staged_name = std::ffi::OsString::from(".");
    staged_name.push(file_name);
    staged_name.push(format!(".{}.tmp", std::process::id()));
    let staged = parent.join(staged_name);
    if let Err(source) = std::fs::write(&staged, content) {
        discard_staged(&staged);
        return Err(write_error(overlay_path, source));
    }
    std::fs::rename(&staged, overlay_path).map_err(|source| {
        discard_staged(&staged);
        write_error(overlay_path, source)
    })
}

fn discard_staged(staged: &Path) {
    if let Err(error) = std::fs::remove_file(staged)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(path = %staged.display(), error = %error, "staged profile write not removed");
    }
}

fn write_error(path: &Path, source: std::io::Error) -> ProfileError {
    match source.kind() {
        std::io::ErrorKind::ReadOnlyFilesystem | std::io::ErrorKind::PermissionDenied => {
            ProfileError::ReadOnly {
                path: path.to_path_buf(),
                source,
            }
        },
        _ => ProfileError::WriteFile {
            path: path.to_path_buf(),
            source,
        },
    }
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
