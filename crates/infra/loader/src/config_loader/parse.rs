//! The one parse step every services file goes through: YAML text to a
//! tree, `${VAR}` interpolation under the loader's
//! [`ServicesInterpolation`] mode, then typed deserialization. A file with no
//! placeholder is deserialized from its text, which keeps the line and key
//! path in a parse error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde_yaml::Value;
use systemprompt_manifest::env::contains_placeholder;
use systemprompt_manifest::services::interpolation::{
    ServicesInterpolation, interpolate_services_yaml,
};

use crate::error::{ConfigLoadError, ConfigLoadResult};

pub(super) type EnvLookup = fn(&str) -> Option<String>;

#[derive(Debug, Clone, Copy)]
pub(super) struct Interpolator {
    pub mode: ServicesInterpolation,
    pub lookup: EnvLookup,
}

impl Interpolator {
    pub(super) fn value(&self, path: &Path, content: &str) -> ConfigLoadResult<Value> {
        let mut value: Value =
            serde_yaml::from_str(content).map_err(|source| ConfigLoadError::Yaml {
                path: path.to_path_buf(),
                source,
            })?;
        let unresolved =
            interpolate_services_yaml(&mut value, &self.lookup, self.mode).map_err(|source| {
                ConfigLoadError::Interpolation {
                    path: path.to_path_buf(),
                    source,
                }
            })?;
        for missing in unresolved {
            tracing::debug!(
                path = %path.display(),
                variable = %missing.variable,
                key_path = %missing.key_path,
                "services placeholder left unresolved for inspection"
            );
        }
        Ok(value)
    }

    pub(super) fn parse<T: DeserializeOwned>(
        &self,
        path: &Path,
        content: &str,
    ) -> ConfigLoadResult<T> {
        let parsed = if contains_placeholder(content) {
            serde_yaml::from_value(self.value(path, content)?)
        } else {
            serde_yaml::from_str(content)
        };
        parsed.map_err(|source| ConfigLoadError::Yaml {
            path: path.to_path_buf(),
            source,
        })
    }
}
