//! Merge bridge-owned keys into the user's Codex config, stripping prior
//! bridge-owned values first so removed keys don't linger, preserving every
//! other key. Bridge-owned surface: the `model_provider` selector, the
//! `otel`/`analytics` tables, and the `model_providers.systemprompt` entry; all
//! other tables survive unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

const OWNED_SCALAR_KEYS: &[&str] = &["model_provider"];
const OWNED_TABLES: &[&str] = &["otel", "analytics"];
const OWNED_PROVIDER: &str = "systemprompt";

pub(super) fn install(source: &Path, target: &Path) -> std::io::Result<()> {
    let source_text = std::fs::read_to_string(source)?;
    let source_value: toml::Value = toml::from_str(&source_text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let existing_text = match std::fs::read_to_string(target) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let mut merged: toml::Value = if existing_text.is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        toml::from_str(&existing_text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
    };

    strip_owned(&mut merged);
    deep_merge(&mut merged, &source_value);

    let rendered = toml::to_string_pretty(&merged)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = target.with_extension(format!(
        "{}.tmp.{}",
        target
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("toml"),
        std::process::id()
    ));
    std::fs::write(&tmp, rendered)?;
    std::fs::rename(&tmp, target)?;
    Ok(())
}

fn strip_owned(target: &mut toml::Value) {
    let toml::Value::Table(top) = target else {
        return;
    };
    for k in OWNED_SCALAR_KEYS {
        top.remove(*k);
    }
    for k in OWNED_TABLES {
        top.remove(*k);
    }
    if let Some(toml::Value::Table(providers)) = top.get_mut("model_providers") {
        providers.remove(OWNED_PROVIDER);
        if providers.is_empty() {
            top.remove("model_providers");
        }
    }
}

fn deep_merge(target: &mut toml::Value, source: &toml::Value) {
    let (toml::Value::Table(t), toml::Value::Table(s)) = (target, source) else {
        return;
    };
    for (k, v) in s {
        match (t.get_mut(k), v) {
            (Some(existing @ toml::Value::Table(_)), toml::Value::Table(_)) => {
                deep_merge(existing, v);
            },
            _ => {
                t.insert(k.clone(), v.clone());
            },
        }
    }
}
