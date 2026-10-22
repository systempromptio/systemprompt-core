//! Refusal of pre-0.73 profile shapes.
//!
//! The loader never translates an old profile. A top-level `providers:`,
//! `gateway:` or `evaluation:` key, or a `*.secrets.profile.yaml` file name, is
//! refused with the command that rewrites the tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_yaml::Value;

use super::{ProfileError, ProfileResult};

pub const MIGRATE_COMMAND: &str = "systemprompt admin config migrate";

const LEGACY_TOP_LEVEL_KEYS: &[(&str, &str)] = &[
    (
        "providers",
        "the provider catalog lives in services/ai/providers.yaml",
    ),
    ("gateway", "gateway routes live in services/ai/gateway.yaml"),
    ("evaluation", "the section is named `judge`"),
];

const LEGACY_FILE_SUFFIX: &str = ".secrets.profile.yaml";

pub(super) fn reject_legacy_file_name(path: &Path) -> ProfileResult<()> {
    let is_legacy = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(LEGACY_FILE_SUFFIX));
    if is_legacy {
        return Err(ProfileError::LegacyShape {
            path: path.to_path_buf(),
            key: LEGACY_FILE_SUFFIX.to_owned(),
            reason: "profiles live at profiles/<env>/profile.yaml over profiles/base.yaml"
                .to_owned(),
        });
    }
    Ok(())
}

pub(super) fn reject_legacy_shape(merged: &Value, path: &Path) -> ProfileResult<()> {
    let Value::Mapping(map) = merged else {
        return Ok(());
    };
    for (key, reason) in LEGACY_TOP_LEVEL_KEYS {
        if map.contains_key(*key) {
            return Err(ProfileError::LegacyShape {
                path: path.to_path_buf(),
                key: (*key).to_owned(),
                reason: (*reason).to_owned(),
            });
        }
    }
    Ok(())
}
