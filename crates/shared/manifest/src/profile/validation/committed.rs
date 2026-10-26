//! Source checks for committed profile files.
//!
//! A profile committed to an install's repository is read on other machines,
//! so an absolute path into a temp directory (`/tmp/...`) only ever resolves
//! on the host that wrote it. These checks read the files as written, before
//! interpolation and path resolution, for every layer of a profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_yaml::Value;

use super::super::layered::{base_path_for, read_layer};
use super::super::{Profile, ProfileError, ProfileResult};

const TEMP_PREFIXES: &[&str] = &[
    "/tmp/",
    "/var/tmp/",
    "/private/tmp/",
    "/private/var/folders/",
];

fn is_temp_path(value: &str) -> bool {
    let trimmed = value.trim();
    TEMP_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
        || TEMP_PREFIXES
            .iter()
            .any(|prefix| trimmed == prefix.trim_end_matches('/'))
}

fn collect_temp_paths(value: &Value, at: &str, out: &mut Vec<String>) {
    match value {
        Value::String(s) if is_temp_path(s) => out.push(format!(
            "{at} is an absolute temp path ({s}); a committed profile must use a path relative \
             to paths.root"
        )),
        Value::Mapping(map) => {
            for (key, child) in map {
                let key = key.as_str().unwrap_or("?");
                let child_at = if at.is_empty() {
                    key.to_owned()
                } else {
                    format!("{at}.{key}")
                };
                collect_temp_paths(child, &child_at, out);
            }
        },
        Value::Sequence(items) => {
            for (idx, child) in items.iter().enumerate() {
                collect_temp_paths(child, &format!("{at}[{idx}]"), out);
            }
        },
        _ => {},
    }
}

#[must_use]
pub fn committed_source_errors(content: &str) -> Vec<String> {
    let Ok(value) = serde_yaml::from_str::<Value>(content) else {
        return Vec::new();
    };
    let mut errors = Vec::new();
    collect_temp_paths(&value, "", &mut errors);
    errors
}

impl Profile {
    pub fn validate_committed(&self, overlay_path: &Path) -> ProfileResult<()> {
        let mut errors = Vec::new();
        let layers = base_path_for(overlay_path)
            .filter(|base| base.is_file())
            .into_iter()
            .chain(std::iter::once(overlay_path.to_path_buf()));
        for layer in layers {
            let content = read_layer(&layer)?;
            errors.extend(
                committed_source_errors(&content)
                    .into_iter()
                    .map(|e| format!("{}: {e}", layer.display())),
            );
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ProfileError::Validation {
                name: self.name.clone(),
                errors,
            })
        }
    }
}
