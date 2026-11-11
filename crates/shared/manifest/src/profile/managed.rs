//! The management view of a profile: inspected, edited, selected or shipped
//! from a host that does not run it.
//!
//! A profile meant for another environment may reference variables that only
//! exist there. [`ManagedProfile`] resolves the variables this host has and
//! keeps every other placeholder as a typed `!unresolved` value, recorded with
//! its key path. A command reads only the sections it needs: a section that
//! contains an unresolved value is an [`ProfileError::Interpolation`] naming
//! the variable, and the whole typed [`Profile`] is available only when
//! nothing is unresolved. A profile about to run uses [`Profile::from_yaml`],
//! where every unset variable is an error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde_yaml::Value;

use super::field_kind::profile_field_kind;
use super::layered::{raw_merged, read_layer};
use super::{Profile, ProfileError, ProfileResult};
use crate::env::{InterpolationError, interpolate_yaml_available, read_env_optional};

/// A profile loaded for management, with unset variables kept unresolved.
#[derive(Debug, Clone)]
pub struct ManagedProfile {
    path: PathBuf,
    raw: Value,
    tree: Value,
    unresolved: Vec<InterpolationError>,
}

impl ManagedProfile {
    pub fn load(overlay_path: &Path) -> ProfileResult<Self> {
        if !overlay_path.is_file() {
            return Err(ProfileError::MissingOverlay {
                path: overlay_path.to_path_buf(),
            });
        }
        Self::from_yaml(&read_layer(overlay_path)?, overlay_path)
    }

    pub fn from_yaml(content: &str, overlay_path: &Path) -> ProfileResult<Self> {
        let raw = raw_merged(content, overlay_path)?;
        let mut tree = raw.clone();
        let unresolved = interpolate_yaml_available(
            &mut tree,
            &|name| read_env_optional(name),
            &profile_field_kind,
        );
        Ok(Self {
            path: overlay_path.to_path_buf(),
            raw,
            tree,
            unresolved,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(super) const fn tree(&self) -> &Value {
        &self.tree
    }

    #[must_use]
    pub fn unresolved(&self) -> &[InterpolationError] {
        &self.unresolved
    }

    pub fn profile(&self) -> ProfileResult<Profile> {
        if let Some(first) = self.unresolved.first() {
            return Err(self.unresolved_error(first));
        }
        Profile::from_layers(None, self.tree.clone(), &self.path)
    }

    pub fn section<T: DeserializeOwned>(&self, key_path: &str) -> ProfileResult<Option<T>> {
        if let Some(first) = self.unresolved_under(key_path) {
            return Err(self.unresolved_error(first));
        }
        self.deserialize_at(&self.tree, key_path)
    }

    pub fn raw_section<T: DeserializeOwned>(&self, key_path: &str) -> ProfileResult<Option<T>> {
        self.deserialize_at(&self.raw, key_path)
    }

    fn unresolved_under(&self, key_path: &str) -> Option<&InterpolationError> {
        self.unresolved.iter().find(|entry| {
            entry.key_path == key_path
                || entry
                    .key_path
                    .strip_prefix(key_path)
                    .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
        })
    }

    fn unresolved_error(&self, entry: &InterpolationError) -> ProfileError {
        ProfileError::Interpolation {
            path: self.path.clone(),
            source: entry.clone(),
        }
    }

    fn deserialize_at<T: DeserializeOwned>(
        &self,
        root: &Value,
        key_path: &str,
    ) -> ProfileResult<Option<T>> {
        key_path
            .split('.')
            .try_fold(root, |node, key| node.get(key))
            .filter(|node| !node.is_null())
            .cloned()
            .map(serde_yaml::from_value)
            .transpose()
            .map_err(|source| ProfileError::ParseYaml {
                path: self.path.clone(),
                source,
            })
    }
}
