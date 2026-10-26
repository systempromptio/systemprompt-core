//! Layered profile loading: `profiles/base.yaml` under
//! `profiles/<env>/profile.yaml`.
//!
//! The base holds everything environments share; the overlay holds only what
//! differs. Maps merge key by key with the overlay winning; a sequence or a
//! scalar in the overlay replaces the base value whole, so two lists are never
//! concatenated. A missing base is an overlay-only profile; a missing overlay
//! is an error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde_yaml::Value;

use super::{Profile, ProfileError, ProfileResult};
use crate::env::{interpolate, read_env_optional};

pub const BASE_PROFILE_FILE: &str = "base.yaml";
pub const OVERLAY_PROFILE_FILE: &str = "profile.yaml";

#[must_use]
pub fn merge_yaml(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Mapping(mut base_map), Value::Mapping(overlay_map)) => {
            for (key, overlay_value) in overlay_map {
                let merged = match base_map.remove(&key) {
                    Some(base_value) => merge_yaml(base_value, overlay_value),
                    None => overlay_value,
                };
                base_map.insert(key, merged);
            }
            Value::Mapping(base_map)
        },
        (_, overlay) => overlay,
    }
}

#[must_use]
pub fn base_path_for(overlay_path: &Path) -> Option<PathBuf> {
    if overlay_path.file_name().and_then(|n| n.to_str()) != Some(OVERLAY_PROFILE_FILE) {
        return None;
    }
    overlay_path
        .parent()
        .and_then(Path::parent)
        .map(|profiles_dir| profiles_dir.join(BASE_PROFILE_FILE))
}

pub(super) fn parse_layer(content: &str, path: &Path) -> ProfileResult<Value> {
    let content = interpolate(content, &|name| read_env_optional(name));
    let value: Value =
        serde_yaml::from_str(&content).map_err(|source| ProfileError::ParseYaml {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(match value {
        Value::Null => Value::Mapping(serde_yaml::Mapping::new()),
        other => other,
    })
}

pub(super) fn read_layer(path: &Path) -> ProfileResult<String> {
    std::fs::read_to_string(path).map_err(|source| ProfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn load_base_layer(overlay_path: &Path) -> ProfileResult<Option<Value>> {
    let Some(base_path) = base_path_for(overlay_path) else {
        return Ok(None);
    };
    if !base_path.is_file() {
        return Ok(None);
    }
    let content = read_layer(&base_path)?;
    parse_layer(&content, &base_path).map(Some)
}

impl Profile {
    pub fn load_layered(profiles_dir: &Path, env: &str) -> ProfileResult<Self> {
        let overlay_path = profiles_dir.join(env).join(OVERLAY_PROFILE_FILE);
        Self::load_from_path(&overlay_path)
    }

    pub fn load_from_path(overlay_path: &Path) -> ProfileResult<Self> {
        if !overlay_path.is_file() {
            return Err(ProfileError::MissingOverlay {
                path: overlay_path.to_path_buf(),
            });
        }
        let content = read_layer(overlay_path)?;
        Self::from_yaml(&content, overlay_path)
    }

    pub fn from_yaml(content: &str, overlay_path: &Path) -> ProfileResult<Self> {
        let base = load_base_layer(overlay_path)?;
        let overlay = parse_layer(content, overlay_path)?;
        Self::from_layers(base, overlay, overlay_path)
    }

    pub fn from_layers(
        base: Option<Value>,
        overlay: Value,
        overlay_path: &Path,
    ) -> ProfileResult<Self> {
        super::legacy::reject_legacy_file_name(overlay_path)?;
        let merged = match base {
            Some(base) => merge_yaml(base, overlay),
            None => overlay,
        };
        super::legacy::reject_legacy_shape(&merged, overlay_path)?;
        super::derived::reject_stated_derived_fields(&merged, overlay_path)?;

        let mut profile: Self =
            serde_yaml::from_value(merged).map_err(|source| ProfileError::ParseYaml {
                path: overlay_path.to_path_buf(),
                source,
            })?;

        let profile_dir =
            overlay_path
                .parent()
                .ok_or_else(|| ProfileError::InvalidProfilePath {
                    path: overlay_path.to_path_buf(),
                })?;
        profile.paths.resolve_relative_to(profile_dir);
        profile.derive_fields()?;

        if let Some(secrets) = profile.secrets.as_ref() {
            secrets.validate()?;
        }
        Ok(profile)
    }
}
