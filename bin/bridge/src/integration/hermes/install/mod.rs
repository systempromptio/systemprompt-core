//! Hermes managed-profile installer: renders the bridge-owned `model` block,
//! merges it into `HERMES_HOME/config.yaml` preserving every user-authored key,
//! and writes the static `OPENAI_API_KEY` into `HERMES_HOME/.env`.
//!
//! Unlike Codex, Hermes reads a plain file on every OS, so there is no
//! `.mobileconfig` path and no credential-helper subprocess — the loopback
//! secret is stable, so it is written to `.env` once as a static key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod merge;
mod render;

use std::io::Write;

use serde_yaml::Value;

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

    let yaml_text = render::managed_yaml(inputs)?;
    let path = dir.join(format!("hermes-bridge-{}-config.yaml", unique_stem()));
    std::fs::File::create(&path)?.write_all(yaml_text.as_bytes())?;
    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: yaml_text.len(),
        payload_uuid,
        profile_uuid,
    })
}

pub(super) fn install_profile(generated_path: &str) -> std::io::Result<()> {
    let source_text = std::fs::read_to_string(generated_path)?;
    let mut source: Value = serde_yaml::from_str(&source_text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Why: lift the API key out of the generated artifact and into .env, then
    // remove the marker so only the `model` block reaches config.yaml.
    let api_key = take_api_key_marker(&mut source);
    if let Some(key) = api_key {
        write_env_key(&config::env_path(), config::ENV_API_KEY, &key)?;
    }

    let target = config::config_yaml_path();
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    merge::install(&source, &target)
}

pub(super) fn remove_profile() -> std::io::Result<ProfileRemoval> {
    let target = config::config_yaml_path();
    let removed_config = merge::uninstall(&target)?;
    let removed_env = remove_env_key(&config::env_path(), config::ENV_API_KEY)?;
    Ok(if removed_config || removed_env {
        ProfileRemoval::Removed {
            path: Some(target.display().to_string()),
        }
    } else {
        ProfileRemoval::NothingToRemove
    })
}

fn take_api_key_marker(source: &mut Value) -> Option<String> {
    let Value::Mapping(top) = source else {
        return None;
    };
    let key = Value::String(render::API_KEY_MARKER.to_owned());
    match top.remove(key) {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}

// Why: `.env` is a flat KEY=VALUE file, not YAML — a targeted line replace
// preserves every other secret the user keeps there.
fn write_env_key(path: &std::path::Path, key: &str, value: &str) -> std::io::Result<()> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let prefix = format!("{key}=");
    let mut lines: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in existing.lines() {
        if line.trim_start().starts_with(&prefix) {
            lines.push(format!("{key}={value}"));
            replaced = true;
        } else {
            lines.push(line.to_owned());
        }
    }
    if !replaced {
        lines.push(format!("{key}={value}"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut body = lines.join("\n");
    body.push('\n');
    std::fs::write(path, body)
}

fn remove_env_key(path: &std::path::Path, key: &str) -> std::io::Result<bool> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let prefix = format!("{key}=");
    let kept: Vec<&str> = existing
        .lines()
        .filter(|l| !l.trim_start().starts_with(&prefix))
        .collect();
    if kept.len() == existing.lines().count() {
        return Ok(false);
    }
    if kept.iter().all(|l| l.trim().is_empty()) {
        std::fs::remove_file(path)?;
        return Ok(true);
    }
    let mut body = kept.join("\n");
    body.push('\n');
    std::fs::write(path, body)?;
    Ok(true)
}
