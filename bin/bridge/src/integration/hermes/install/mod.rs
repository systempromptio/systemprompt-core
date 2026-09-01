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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn inputs() -> ProfileGenInputs {
        ProfileGenInputs {
            gateway_base_url: "http://127.0.0.1:48217".to_owned(),
            api_key: "loopback-secret-value".to_owned(),
            models: vec!["claude-haiku-4-5".to_owned()],
            organization_uuid: None,
            headers: BTreeMap::new(),
        }
    }

    fn generated() -> Value {
        serde_yaml::from_str(&render::managed_yaml(&inputs()).expect("render")).expect("parse")
    }

    // Why: these four keys are the whole contract with Hermes. `provider` chosen
    // as the named entry, that entry's `base_url` and `api_mode`, and `key_env`
    // naming the `.env` variable — Hermes host-gates its bare OPENAI_API_KEY
    // fallback to openai.com, so a 127.0.0.1 endpoint resolves no credential
    // without it and the proxy answers 403.
    #[test]
    fn generated_profile_matches_hermes_provider_contract() {
        let mut value = generated();
        let key = take_api_key_marker(&mut value).expect("api key marker");
        assert_eq!(key, "loopback-secret-value");

        let dotted = |path: &str| -> String {
            let mut cur = &value;
            for seg in path.split('.') {
                cur = cur
                    .as_mapping()
                    .and_then(|m| m.get(Value::String(seg.to_owned())))
                    .unwrap_or_else(|| panic!("missing {path}"));
            }
            cur.as_str().expect("string").to_owned()
        };

        assert_eq!(dotted(config::MODEL_PROVIDER), config::PROVIDER_ENTRY);
        assert_eq!(
            dotted(config::PROVIDER_BASE_URL),
            "http://127.0.0.1:48217/v1"
        );
        assert_eq!(dotted(config::PROVIDER_API_MODE), "chat_completions");
        assert_eq!(dotted(config::PROVIDER_KEY_ENV), config::ENV_API_KEY);
        assert_eq!(dotted(config::MODEL_NAME), "claude-haiku-4-5");
    }

    // Why: Hermes' shipped config.yaml is a large annotated file the user edits
    // in place. The merge must land the managed keys without disturbing any
    // other key, and an uninstall must put it back exactly.
    #[test]
    fn merge_preserves_user_keys_and_uninstall_restores_them() {
        let dir = std::env::temp_dir().join(format!("hermes-merge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let target = dir.join("config.yaml");
        let original = "model:\n  default: anthropic/claude-opus-4.6\n  provider: auto\n  \
                        context_length: 131072\nproviders:\n  mine:\n    base_url: \
                        https://example.invalid/v1\nterminal:\n  backend: local\n";
        std::fs::write(&target, original).expect("seed");

        let mut source = generated();
        _ = take_api_key_marker(&mut source);
        merge::install(&source, &target).expect("install");

        let after: Value =
            serde_yaml::from_str(&std::fs::read_to_string(&target).expect("read")).expect("parse");
        let model = after
            .get(Value::String("model".to_owned()))
            .and_then(Value::as_mapping)
            .expect("model");
        // Managed keys applied...
        assert_eq!(
            model.get(Value::String("provider".to_owned())),
            Some(&Value::String(config::PROVIDER_ENTRY.to_owned()))
        );
        assert_eq!(
            model.get(Value::String("default".to_owned())),
            Some(&Value::String("claude-haiku-4-5".to_owned()))
        );
        // ...and unrelated user keys, including a foreign provider entry, survive.
        assert!(model.contains_key(Value::String("context_length".to_owned())));
        assert!(after
            .get(Value::String("providers".to_owned()))
            .and_then(Value::as_mapping)
            .expect("providers")
            .contains_key(Value::String("mine".to_owned())));
        assert!(after.get(Value::String("terminal".to_owned())).is_some());

        assert!(merge::uninstall(&target).expect("uninstall"));
        let restored: Value =
            serde_yaml::from_str(&std::fs::read_to_string(&target).expect("read")).expect("parse");
        let restored_providers = restored
            .get(Value::String("providers".to_owned()))
            .and_then(Value::as_mapping)
            .expect("providers survive");
        assert!(restored_providers.contains_key(Value::String("mine".to_owned())));
        assert!(!restored_providers.contains_key(Value::String(config::PROVIDER_ENTRY.to_owned())));

        std::fs::remove_dir_all(&dir).ok();
    }

    // Why: `.env` holds the user's other secrets; only our key may move.
    #[test]
    fn env_write_replaces_only_the_managed_key() {
        let dir = std::env::temp_dir().join(format!("hermes-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let env = dir.join(".env");
        std::fs::write(&env, "BROWSER_SESSION_TIMEOUT=300\nOPENAI_API_KEY=old\n").expect("seed");

        write_env_key(&env, config::ENV_API_KEY, "new-secret").expect("write");
        let body = std::fs::read_to_string(&env).expect("read");
        assert!(body.contains("BROWSER_SESSION_TIMEOUT=300"));
        assert!(body.contains("OPENAI_API_KEY=new-secret"));
        assert!(!body.contains("OPENAI_API_KEY=old"));

        assert!(remove_env_key(&env, config::ENV_API_KEY).expect("remove"));
        let after = std::fs::read_to_string(&env).expect("read");
        assert!(after.contains("BROWSER_SESSION_TIMEOUT=300"));
        assert!(!after.contains("OPENAI_API_KEY"));

        std::fs::remove_dir_all(&dir).ok();
    }
}

