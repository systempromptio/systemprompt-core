//! Merge bridge-owned keys into the user's Codex config, stripping prior
//! bridge-owned values first so removed keys don't linger, preserving every
//! other key together with its comments and layout. Bridge-owned surface: the
//! `model_provider` selector, the `approval_policy` and `sandbox_mode`
//! selectors, the `otel`/`analytics`/`sandbox_workspace_write` tables, and the
//! `model_providers.systemprompt` entry; all other tables survive unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use toml_edit::{DocumentMut, Item};

use crate::host_sync::ForeignShape;

const OWNED_SCALAR_KEYS: &[&str] = &["model_provider", "approval_policy", "sandbox_mode"];
const OWNED_TABLES: &[&str] = &["otel", "analytics", "sandbox_workspace_write"];
const OWNED_PROVIDER: &str = "systemprompt";

fn invalid(e: impl std::error::Error + Send + Sync + 'static) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
}

fn read_document(target: &Path) -> std::io::Result<Option<DocumentMut>> {
    let existing_text = match std::fs::read_to_string(target) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    existing_text
        .parse::<DocumentMut>()
        .map(Some)
        .map_err(invalid)
}

fn write_document(target: &Path, doc: &DocumentMut) -> std::io::Result<()> {
    crate::fsutil::atomic_write_0644(target, doc.to_string().as_bytes())
}

pub(super) fn install(source: &Path, target: &Path) -> std::io::Result<()> {
    let source_text = std::fs::read_to_string(source)?;
    let source_value: toml::Value = toml::from_str(&source_text).map_err(invalid)?;

    let mut doc = read_document(target)?.unwrap_or_default();
    strip_owned(&mut doc);
    let toml::Value::Table(source_table) = &source_value else {
        return Err(ForeignShape {
            path: source.display().to_string(),
            key: "<root>".to_owned(),
            found: source_value.type_str(),
            expected: "a table",
        }
        .into());
    };
    deep_merge(doc.as_table_mut(), source_table, target)?;
    write_document(target, &doc)
}

pub(super) fn uninstall(target: &Path) -> std::io::Result<bool> {
    let Some(mut doc) = read_document(target)? else {
        return Ok(false);
    };
    let before = doc.to_string();
    strip_owned(&mut doc);
    if doc.to_string() == before {
        return Ok(false);
    }
    if doc.as_table().is_empty() {
        crate::fsutil::remove_verified(target)?;
        return Ok(true);
    }
    write_document(target, &doc)?;
    Ok(true)
}

fn strip_owned(doc: &mut DocumentMut) {
    let top = doc.as_table_mut();
    for k in OWNED_SCALAR_KEYS {
        top.remove(k);
    }
    for k in OWNED_TABLES {
        top.remove(k);
    }
    if let Some(providers) = top.get_mut("model_providers").and_then(Item::as_table_mut) {
        providers.remove(OWNED_PROVIDER);
        if providers.is_empty() {
            top.remove("model_providers");
        }
    }
}

fn deep_merge(
    target: &mut toml_edit::Table,
    source: &toml::map::Map<String, toml::Value>,
    path: &Path,
) -> Result<(), ForeignShape> {
    for (key, value) in source {
        match value {
            toml::Value::Table(inner) => {
                let entry = target
                    .entry(key)
                    .or_insert_with(|| Item::Table(toml_edit::Table::new()));
                let Some(table) = entry.as_table_mut() else {
                    return Err(ForeignShape {
                        path: path.display().to_string(),
                        key: key.clone(),
                        found: entry.type_name(),
                        expected: "a table",
                    });
                };
                deep_merge(table, inner, path)?;
            },
            other => {
                target.insert(key, Item::Value(edit_value(other)));
            },
        }
    }
    Ok(())
}

fn edit_value(value: &toml::Value) -> toml_edit::Value {
    match value {
        toml::Value::String(s) => toml_edit::Value::from(s.as_str()),
        toml::Value::Integer(i) => toml_edit::Value::from(*i),
        toml::Value::Float(f) => toml_edit::Value::from(*f),
        toml::Value::Boolean(b) => toml_edit::Value::from(*b),
        toml::Value::Datetime(d) => toml_edit::Value::from(d.to_string()),
        toml::Value::Array(items) => {
            toml_edit::Value::Array(items.iter().map(edit_value).collect())
        },
        toml::Value::Table(table) => {
            let mut inline = toml_edit::InlineTable::new();
            for (k, v) in table {
                inline.insert(k, edit_value(v));
            }
            toml_edit::Value::InlineTable(inline)
        },
    }
}
