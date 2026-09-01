//! Merge the bridge-owned keys into the user's Hermes `config.yaml`, stripping
//! prior bridge-owned values first so removed keys don't linger and preserving
//! every other key. Bridge-owned surface: `model.provider`, `model.default`,
//! and the whole `providers.<PROVIDER_ENTRY>` entry; the `model` and
//! `providers` tables are removed only if they become empty. All other keys
//! survive unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_yaml::Value;

const OWNED_MODEL_KEYS: &[&str] = &["provider", "default"];
const MODEL_TABLE: &str = "model";
const PROVIDERS_TABLE: &str = "providers";

fn yaml_key(k: &str) -> Value {
    Value::String(k.to_owned())
}

fn io_invalid(e: serde_yaml::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
}

pub(super) fn install(source: &Value, target: &Path) -> std::io::Result<()> {
    let existing_text = match std::fs::read_to_string(target) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let mut merged: Value = if existing_text.trim().is_empty() {
        Value::Mapping(serde_yaml::Mapping::new())
    } else {
        serde_yaml::from_str(&existing_text).map_err(io_invalid)?
    };
    if !matches!(merged, Value::Mapping(_)) {
        merged = Value::Mapping(serde_yaml::Mapping::new());
    }

    strip_owned(&mut merged);
    deep_merge(&mut merged, source);

    write_atomic(target, &merged)
}

// Why: the inverse of `install` — take the bridge-owned model keys back out and
// leave every other key exactly where it was.
pub(super) fn uninstall(target: &Path) -> std::io::Result<bool> {
    let existing_text = match std::fs::read_to_string(target) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if existing_text.trim().is_empty() {
        return Ok(false);
    }
    let mut value: Value = serde_yaml::from_str(&existing_text).map_err(io_invalid)?;
    let before = value.clone();
    strip_owned(&mut value);
    if value == before {
        return Ok(false);
    }

    let empty = matches!(&value, Value::Mapping(m) if m.is_empty());
    if empty {
        std::fs::remove_file(target)?;
        return Ok(true);
    }
    write_atomic(target, &value)?;
    Ok(true)
}

fn write_atomic(target: &Path, value: &Value) -> std::io::Result<()> {
    let rendered = serde_yaml::to_string(value).map_err(io_invalid)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = target.with_extension(format!("yaml.tmp.{}", std::process::id()));
    std::fs::write(&tmp, rendered)?;
    std::fs::rename(&tmp, target)?;
    Ok(())
}

fn strip_owned(target: &mut Value) {
    let Value::Mapping(top) = target else {
        return;
    };
    // Why: only our own `providers:` entry is removed. A user's other named
    // providers live in the same table and must survive an uninstall.
    if let Some(Value::Mapping(providers)) = top.get_mut(yaml_key(PROVIDERS_TABLE)) {
        providers.remove(yaml_key(super::super::config::PROVIDER_ENTRY));
        if providers.is_empty() {
            top.remove(yaml_key(PROVIDERS_TABLE));
        }
    }
    if let Some(Value::Mapping(model)) = top.get_mut(yaml_key(MODEL_TABLE)) {
        for k in OWNED_MODEL_KEYS {
            model.remove(yaml_key(k));
        }
        if model.is_empty() {
            top.remove(yaml_key(MODEL_TABLE));
        }
    }
}

fn deep_merge(target: &mut Value, source: &Value) {
    let (Value::Mapping(t), Value::Mapping(s)) = (target, source) else {
        return;
    };
    for (k, v) in s {
        match (t.get_mut(k), v) {
            (Some(existing @ Value::Mapping(_)), Value::Mapping(_)) => {
                deep_merge(existing, v);
            },
            _ => {
                t.insert(k.clone(), v.clone());
            },
        }
    }
}
