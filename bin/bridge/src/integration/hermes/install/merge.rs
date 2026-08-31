//! Merge the bridge-owned `model` keys into the user's Hermes `config.yaml`,
//! stripping prior bridge-owned values first so removed keys don't linger and
//! preserving every other key. Bridge-owned surface: `model.base_url`,
//! `model.api_mode`, and `model.model`; the `model` table itself is removed
//! only if it becomes empty. All other keys survive unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_yaml::Value;

const OWNED_MODEL_KEYS: &[&str] = &["base_url", "api_mode", "model"];
const MODEL_TABLE: &str = "model";

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
    let Some(Value::Mapping(model)) = top.get_mut(yaml_key(MODEL_TABLE)) else {
        return;
    };
    for k in OWNED_MODEL_KEYS {
        model.remove(yaml_key(k));
    }
    if model.is_empty() {
        top.remove(yaml_key(MODEL_TABLE));
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
