//! `OpenCode` managed-profile installer: renders the bridge-owned provider
//! block, merges it into the managed `opencode.json` preserving every
//! admin-authored key, and writes the host token into the user's
//! `auth.json`.
//!
//! The managed file is admin-owned, so the merge goes through
//! [`crate::install::managed_file`], which writes directly when it can and
//! escalates only when refused. The host token is derived deterministically
//! from the loopback secret, so it is written once as a static key rather
//! than through a helper subprocess, and rotates with the secret via the
//! stale-profile re-apply.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod merge;
mod render;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::config;
use crate::integration::generated_profile;
use crate::integration::host_app::{
    GeneratedProfile, ProfileGenInputs, ProfileInstalled, ProfileRemoval,
};
use crate::integration::reapply::Attendance;

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let uuids = generated_profile::profile_uuids();
    let json_text = render::managed_json_text(inputs)?;
    let path = generated_profile::write("opencode-bridge-opencode", ".json", json_text.as_bytes())?;
    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: json_text.len(),
        payload_uuid: uuids.payload,
        profile_uuid: uuids.profile,
    })
}

pub(super) fn install_profile(
    generated_path: &str,
    attendance: Attendance,
) -> std::io::Result<ProfileInstalled> {
    let source_text = std::fs::read_to_string(generated_path)?;
    let mut source = parse_object(&source_text, generated_path)?;

    if let Some(Value::String(key)) = source.remove(render::API_KEY_MARKER) {
        upsert_auth_key(&config::auth_json_path(), &key)?;
    }

    let managed = config::managed_config_path().map_err(std::io::Error::other)?;
    generated_profile::consume(generated_path)?;
    // Why: an admin-tier file written by an elevated install is read-only to
    // the tray, and an unattended sync may not raise the prompt that would
    // refresh it — without this the file keeps the catalogue it was written
    // with forever.
    if attendance == Attendance::Unattended && is_read_only(&managed) {
        return install_user_tier(&source, &managed);
    }
    match merge::install(&source, &managed) {
        Ok(_) => Ok(ProfileInstalled::ok()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            if managed.is_file() {
                return install_user_tier(&source, &managed);
            }
            let Some(fallback) = config::fallback_config_path(&managed) else {
                return Err(e);
            };
            tracing::warn!(
                managed = %managed.display(),
                fallback = %fallback.display(),
                error = %e,
                "opencode install: managed tier not writable; writing the provider block to the \
                 user tier instead (weaker: the user can edit it)"
            );
            merge::install(&source, &fallback).map(|_| ProfileInstalled::ok())
        },
        Err(e) => Err(e),
    }
}

fn is_read_only(path: &Path) -> bool {
    path.is_file() && std::fs::OpenOptions::new().write(true).open(path).is_err()
}

fn install_user_tier(
    source: &Map<String, Value>,
    managed: &Path,
) -> std::io::Result<ProfileInstalled> {
    let user = config::user_tier_path(managed).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("{} is not writable by this process", managed.display()),
        )
    })?;
    merge::install(source, &user)?;
    let warning = format!(
        "{} is read-only to this process, so the live model list was written to {} instead; \
         OpenCode ranks the admin file higher, so models it lists that the gateway no longer \
         serves, and its default model, remain until `install --host opencode` is re-run as \
         administrator",
        managed.display(),
        user.display()
    );
    tracing::warn!(
        managed = %managed.display(),
        user = %user.display(),
        "opencode install: admin config read-only, live model list written to the user config"
    );
    Ok(ProfileInstalled::with_warning(warning))
}

pub(super) fn admin_tier_models() -> Option<(PathBuf, Vec<String>)> {
    let managed = config::managed_config_path().ok()?;
    let root = read_object(&managed).ok()?;
    let models = root
        .get("provider")?
        .get(config::PROVIDER_ID)?
        .get("models")?
        .as_object()?
        .keys()
        .cloned()
        .collect();
    Some((managed, models))
}

pub(super) fn remove_profile() -> std::io::Result<ProfileRemoval> {
    let target = config::managed_config_path().map_err(std::io::Error::other)?;
    let removed_config = merge::uninstall(&target)?;
    let removed_fallback = match config::user_tier_path(&target) {
        Some(path) => merge::uninstall(&path)?,
        None => false,
    };
    let removed_auth = remove_auth_key(&config::auth_json_path())?;
    super::managed_resources::remove_hook_plugin().map_err(std::io::Error::other)?;
    Ok(if removed_config || removed_fallback || removed_auth {
        ProfileRemoval::Removed {
            path: Some(target.display().to_string()),
        }
    } else {
        ProfileRemoval::NothingToRemove
    })
}

pub(super) fn elevation_prompt() -> String {
    format!(
        "{} needs administrator privileges to install the OpenCode managed configuration.",
        crate::brand::brand().app_name
    )
}

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

pub(super) fn read_object(path: &Path) -> std::io::Result<Map<String, Value>> {
    crate::fsutil::read_optional(path)?.map_or_else(
        || Ok(Map::new()),
        |text| parse_object(&text, &path.display().to_string()),
    )
}

pub(super) fn pretty(map: &Map<String, Value>) -> std::io::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(map.clone()))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn installed_key_fingerprint(path: &Path) -> Option<String> {
    let auth = read_object(path).ok()?;
    let key = auth.get(config::PROVIDER_ID)?.get("key")?.as_str()?;
    Some(crate::proxy::secret::fingerprint(key))
}

fn upsert_auth_key(path: &Path, key: &str) -> std::io::Result<()> {
    let mut auth = read_object(path)?;
    let entry = serde_json::json!({ "type": "api", "key": key });
    if auth.get(config::PROVIDER_ID) == Some(&entry) {
        return Ok(());
    }
    auth.insert(config::PROVIDER_ID.to_owned(), entry);
    crate::fsutil::atomic_write_0600(path, &pretty(&auth)?)
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
        std::fs::remove_file(path)?;
    } else {
        crate::fsutil::atomic_write_0600(path, &pretty(&auth)?)?;
    }
    Ok(true)
}
