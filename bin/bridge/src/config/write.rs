//! Comment-preserving mutation of the bridge config TOML.
//!
//! The config file is operator-editable and may be provisioned by MDM, so every
//! write goes through `toml_edit` rather than a serialise-and-replace:
//! comments, key order and keys this build does not know about all survive.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Value};

#[derive(Debug, thiserror::Error)]
pub enum ConfigWriteError {
    #[error("config path unresolvable on this platform")]
    PathUnresolvable,
    #[error("read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {source}")]
    Malformed {
        path: PathBuf,
        source: toml_edit::TomlError,
    },
    #[error("write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn edit(mutate: impl FnOnce(&mut DocumentMut)) -> Result<(), ConfigWriteError> {
    let path = super::config_path().ok_or(ConfigWriteError::PathUnresolvable)?;
    edit_file(&path, mutate)
}

pub fn edit_file(
    path: &Path,
    mutate: impl FnOnce(&mut DocumentMut),
) -> Result<(), ConfigWriteError> {
    let existing = crate::fsutil::read_optional(path)
        .map_err(|source| ConfigWriteError::Read {
            path: path.to_owned(),
            source,
        })?
        .unwrap_or_default();

    let mut doc: DocumentMut = existing
        .parse()
        .map_err(|source| ConfigWriteError::Malformed {
            path: path.to_owned(),
            source,
        })?;

    mutate(&mut doc);

    crate::fsutil::atomic_write_0600(path, doc.to_string().as_bytes()).map_err(|source| {
        ConfigWriteError::Write {
            path: path.to_owned(),
            source,
        }
    })
}

pub fn set(doc: &mut DocumentMut, path: &[&str], value: impl Into<Value>) {
    let Some((leaf, parents)) = path.split_last() else {
        return;
    };
    let mut table = doc.as_table_mut();
    for key in parents {
        let entry = table
            .entry(key)
            .or_insert_with(|| Item::Table(toml_edit::Table::new()));
        let Some(next) = entry.as_table_mut() else {
            return;
        };
        table = next;
    }
    let mut next = value.into();
    // Why: replacing the item outright would discard the whitespace and any
    // comment the operator wrote against this key.
    if let Some(existing) = table.get_mut(leaf).and_then(Item::as_value_mut) {
        *next.decor_mut() = existing.decor().clone();
        *existing = next;
        return;
    }
    table.insert(leaf, Item::Value(next));
}

pub fn set_if_absent(doc: &mut DocumentMut, path: &[&str], value: impl Into<Value>) {
    if get(doc, path).is_none() {
        set(doc, path, value);
    }
}

pub fn remove(doc: &mut DocumentMut, path: &[&str]) {
    let Some((leaf, parents)) = path.split_last() else {
        return;
    };
    let mut table = doc.as_table_mut();
    for key in parents {
        let Some(next) = table.get_mut(key).and_then(Item::as_table_mut) else {
            return;
        };
        table = next;
    }
    table.remove(leaf);
}

#[must_use]
pub fn get<'a>(doc: &'a DocumentMut, path: &[&str]) -> Option<&'a Item> {
    let mut item = doc.as_item();
    for key in path {
        item = item.get(key)?;
    }
    Some(item)
}
