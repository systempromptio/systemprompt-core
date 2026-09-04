//! `OpenCode` managed-profile installer: renders the bridge-owned provider
//! block, merges it into the managed `opencode.json` preserving every
//! admin-authored key, and writes the static API key into the user's
//! `auth.json`.
//!
//! The managed file is admin-owned, so the merge goes through
//! [`crate::install::managed_file`], which writes directly when it can and
//! escalates only when refused. The loopback secret is stable, so it is
//! written once as a static key rather than through a helper subprocess.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod merge;
mod render;

use std::io::Write;
use std::path::Path;

use serde_json::{Map, Value};

use super::config;
use crate::integration::host_app::{GeneratedProfile, ProfileGenInputs, ProfileRemoval};

fn unique_stem() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        config::now_unix(),
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&dir)?;
    let (payload_uuid, profile_uuid) = config::make_uuids();

    let json_text = render::managed_json_text(inputs)?;
    let path = dir.join(format!("opencode-bridge-{}-opencode.json", unique_stem()));
    std::fs::File::create(&path)?.write_all(json_text.as_bytes())?;
    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: json_text.len(),
        payload_uuid,
        profile_uuid,
    })
}

pub(super) fn install_profile(generated_path: &str) -> std::io::Result<()> {
    let source_text = std::fs::read_to_string(generated_path)?;
    let mut source = parse_object(&source_text, generated_path)?;

    // Why: lift the API key out of the generated artifact into auth.json, then
    // drop the marker so only the provider block reaches the managed file.
    if let Some(Value::String(key)) = source.remove(render::API_KEY_MARKER) {
        upsert_auth_key(&config::auth_json_path(), &key)?;
    }

    let managed = config::managed_config_path();
    match merge::install(&source, &managed) {
        Ok(_) => Ok(()),
        // Why: on Linux `write_managed_file` has no elevation to offer, so a
        // read-only /etc/opencode used to fail the whole enrolment and leave
        // the client with MCP servers and no credential — the 403 this fallback
        // exists to prevent. See `config::fallback_config_path`.
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let Some(fallback) = config::fallback_config_path() else {
                return Err(e);
            };
            tracing::warn!(
                managed = %managed.display(),
                fallback = %fallback.display(),
                error = %e,
                "opencode install: managed tier not writable; writing the provider block to the \
                 user tier instead (weaker: the user can edit it)"
            );
            merge::install(&source, &fallback).map(|_| ())
        },
        Err(e) => Err(e),
    }
}

pub(super) fn remove_profile() -> std::io::Result<ProfileRemoval> {
    let target = config::managed_config_path();
    let removed_config = merge::uninstall(&target)?;
    // Why: an install that fell back to the user tier left its block there, so
    // uninstall has to sweep both or the client keeps routing at a dead port.
    let removed_fallback = match config::fallback_config_path() {
        Some(path) => merge::uninstall(&path)?,
        None => false,
    };
    let removed_auth = remove_auth_key(&config::auth_json_path())?;
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

// Why: auth.json holds other providers' OAuth blobs; an unparseable or
// non-object file must abort rather than be overwritten.
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
