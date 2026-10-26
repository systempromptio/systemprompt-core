//! `OpenCode` profile installer: renders the bridge's V2 provider block with
//! the shared builder, writes the host token to the 0600 file the block names
//! with `{file:}`, and merges the block into the user's `opencode.json`,
//! preserving every key the bridge does not own.
//!
//! When an admin-managed tier already carries the `systemprompt` provider
//! (MDM), only the token file is written: provider and policy config belong to
//! the admin. Every install strips the keys the V1 writer owned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod merge;
mod profile;

pub(super) use profile::{generated_text, render};

use std::path::Path;

use serde_json::{Map, Value};

use super::config;
use super::tier::{ProviderTier, provider_tier};
use crate::integration::generated_profile;
use crate::integration::host_app::{
    GeneratedProfile, HostAppError, ProfileGenInputs, ProfileInstalled, ProfileRemoval,
};

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let uuids = generated_profile::profile_uuids();
    let text = generated_text(inputs)?;
    let path = generated_profile::write("opencode-bridge-opencode", ".json", text.as_bytes())?;
    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: text.len(),
        payload_uuid: uuids.payload,
        profile_uuid: uuids.profile,
    })
}

pub(super) fn install_profile(generated_path: &str) -> Result<ProfileInstalled, HostAppError> {
    let text = std::fs::read_to_string(generated_path)?;
    let generated: profile::GeneratedOpenCode = serde_json::from_str(&text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    generated_profile::consume(generated_path)?;
    write_token(&generated.token)?;
    remove_auth_key(&config::auth_json_path())?;
    if provider_tier() == ProviderTier::User {
        let Value::Object(source) = generated.config else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "generated OpenCode profile is not a JSON object",
            )
            .into());
        };
        merge::install(&source, &config::user_config_path())?;
    } else {
        merge::uninstall(&config::user_config_path())?;
    }
    Ok(ProfileInstalled::ok())
}

fn write_token(token: &str) -> std::io::Result<()> {
    let path = config::token_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::fsutil::atomic_write_0600(&path, token.as_bytes())?;
    crate::fsutil::verify_contents(&path, token.as_bytes())
}

pub(super) fn remove_profile() -> Result<ProfileRemoval, HostAppError> {
    let user = config::user_config_path();
    let removed_user = merge::uninstall(&user)?;
    let token = config::token_path();
    let removed_token = token.exists();
    if removed_token {
        crate::fsutil::remove_verified(&token)?;
    }
    let removed_auth = remove_auth_key(&config::auth_json_path())?;
    super::managed_resources::remove_owned_content().map_err(std::io::Error::other)?;
    Ok(if removed_user || removed_token || removed_auth {
        ProfileRemoval::Removed {
            path: Some(user.display().to_string()),
        }
    } else {
        ProfileRemoval::NothingToRemove
    })
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
