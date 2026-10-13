//! `OpenCode` profile installer: renders the bridge-owned provider block and
//! merges it into the user's `opencode.json`, preserving every key the bridge
//! does not own.
//!
//! `OpenCode` 2.x loads only the user tier, so that is where the provider, its
//! `apiKey` and the `enabled_providers` allowlist go — the same file the MCP
//! connectors are written to, rewritable by unattended sync without a prompt.
//! The admin tier 1.x read, and the `auth.json` it took the key from, are now
//! only cleaned: install drops a stale `auth.json` entry, uninstall strips the
//! admin tier (elevating if it must).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod merge;
mod render;

pub(super) use render::provider_json_text;

use std::path::Path;

use serde_json::{Map, Value};

use super::config;
use crate::integration::generated_profile;
use crate::integration::host_app::{
    GeneratedProfile, HostAppError, ProfileGenInputs, ProfileInstalled, ProfileRemoval,
};

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let uuids = generated_profile::profile_uuids();
    let json_text = provider_json_text(inputs)?;
    let path = generated_profile::write("opencode-bridge-opencode", ".json", json_text.as_bytes())?;
    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: json_text.len(),
        payload_uuid: uuids.payload,
        profile_uuid: uuids.profile,
    })
}

pub(super) fn install_profile(generated_path: &str) -> Result<ProfileInstalled, HostAppError> {
    let source_text = std::fs::read_to_string(generated_path)?;
    let source = parse_object(&source_text, generated_path)?;
    generated_profile::consume(generated_path)?;
    merge::install(&source, &config::user_config_path())?;
    remove_auth_key(&config::auth_json_path())?;
    Ok(ProfileInstalled::ok())
}

#[must_use]
pub(super) fn stale_admin_tier() -> Option<std::path::PathBuf> {
    let managed = config::managed_config_path().ok()?;
    merge::carries_owned(&managed).then_some(managed)
}

pub(super) fn remove_profile() -> Result<ProfileRemoval, HostAppError> {
    let user = config::user_config_path();
    let removed_user = merge::uninstall_user(&user)?;
    let removed_managed = match config::managed_config_path() {
        Ok(managed) => merge::uninstall_managed(&managed)?,
        Err(_) => false,
    };
    let removed_auth = remove_auth_key(&config::auth_json_path())?;
    super::managed_resources::remove_hook_plugin().map_err(std::io::Error::other)?;
    Ok(if removed_user || removed_managed || removed_auth {
        ProfileRemoval::Removed {
            path: Some(user.display().to_string()),
        }
    } else {
        ProfileRemoval::NothingToRemove
    })
}

pub(super) fn elevation_prompt() -> String {
    format!(
        "{} needs administrator privileges to remove the retired OpenCode managed configuration.",
        crate::brand::brand().app_name
    )
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
pub(super) fn parse_object(text: &str, source: &str) -> std::io::Result<Map<String, Value>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{source} is not a JSON object; refusing to overwrite"),
        )),
        Err(e) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("parse {source}: {e}; refusing to overwrite"),
        )),
    }
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
pub(super) fn read_object(path: &Path) -> std::io::Result<Map<String, Value>> {
    crate::fsutil::read_optional(path)?.map_or_else(
        || Ok(Map::new()),
        |text| parse_object(&text, &path.display().to_string()),
    )
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
pub(super) fn pretty(map: &Map<String, Value>) -> std::io::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(map.clone()))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn remove_auth_key(path: &Path) -> std::io::Result<bool> {
    let Some(text) = crate::fsutil::read_optional(path)? else {
        return Ok(false);
    };
    let mut auth = parse_object(&text, &path.display().to_string())?;
    if auth.remove(config::PROVIDER_ID).is_none() {
        return Ok(false);
    }
    if auth.is_empty() {
        crate::fsutil::remove_verified(path)?;
    } else {
        crate::fsutil::atomic_write_0600(path, &pretty(&auth)?)?;
    }
    Ok(true)
}
