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
    #[error(
        "gateway changed while signing trust was being established: {0}; retry against the current gateway"
    )]
    GatewayChanged(String),
    #[error("config path unresolvable on this platform")]
    PathUnresolvable,
    #[error("config key {key}: expected a nonempty path through TOML tables")]
    InvalidPath { key: String },
    #[error("config {path} changed during the edit; retry with the current contents")]
    ConcurrentEdit { path: PathBuf },
    #[error("read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {source}")]
    Malformed {
        path: PathBuf,
        source: Box<toml_edit::TomlError>,
    },
    #[error("write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn edit(
    mutate: impl FnOnce(&mut DocumentMut) -> Result<(), ConfigWriteError>,
) -> Result<(), ConfigWriteError> {
    let path = super::config_path().ok_or(ConfigWriteError::PathUnresolvable)?;
    edit_file(&path, mutate)
}

pub fn edit_file(
    path: &Path,
    mutate: impl FnOnce(&mut DocumentMut) -> Result<(), ConfigWriteError>,
) -> Result<(), ConfigWriteError> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
        && !parent
            .try_exists()
            .map_err(|source| ConfigWriteError::Read {
                path: parent.to_owned(),
                source,
            })?
    {
        crate::fsutil::create_dir_all_mode_0700(parent).map_err(|source| {
            ConfigWriteError::Write {
                path: parent.to_owned(),
                source,
            }
        })?;
    }
    let lock_path = path.with_extension("toml.lock");
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(&lock_path)
        .map_err(|source| ConfigWriteError::Write {
            path: lock_path.clone(),
            source,
        })?;
    lock.lock().map_err(|source| ConfigWriteError::Write {
        path: lock_path,
        source,
    })?;
    let existing = crate::fsutil::read_optional(path).map_err(|source| ConfigWriteError::Read {
        path: path.to_owned(),
        source,
    })?;

    let mut doc: DocumentMut = existing
        .as_deref()
        .unwrap_or("")
        .parse()
        .map_err(|source| ConfigWriteError::Malformed {
            path: path.to_owned(),
            source: Box::new(source),
        })?;

    mutate(&mut doc)?;
    let current = crate::fsutil::read_optional(path).map_err(|source| ConfigWriteError::Read {
        path: path.to_owned(),
        source,
    })?;
    if current != existing {
        return Err(ConfigWriteError::ConcurrentEdit {
            path: path.to_owned(),
        });
    }

    crate::fsutil::atomic_write_0600(path, doc.to_string().as_bytes()).map_err(|source| {
        ConfigWriteError::Write {
            path: path.to_owned(),
            source,
        }
    })
}

pub fn set(
    doc: &mut DocumentMut,
    path: &[&str],
    value: impl Into<Value>,
) -> Result<(), ConfigWriteError> {
    let Some((leaf, parents)) = path.split_last() else {
        return Err(ConfigWriteError::InvalidPath {
            key: path.join("."),
        });
    };
    let mut table = doc.as_table_mut();
    for key in parents {
        let entry = table
            .entry(key)
            .or_insert_with(|| Item::Table(toml_edit::Table::new()));
        let Some(next) = entry.as_table_mut() else {
            return Err(ConfigWriteError::InvalidPath {
                key: path.join("."),
            });
        };
        table = next;
    }
    let mut next = value.into();
    if let Some(existing) = table.get_mut(leaf).and_then(Item::as_value_mut) {
        *next.decor_mut() = existing.decor().clone();
        *existing = next;
        return Ok(());
    }
    table.insert(leaf, Item::Value(next));
    Ok(())
}

pub fn set_if_absent(
    doc: &mut DocumentMut,
    path: &[&str],
    value: impl Into<Value>,
) -> Result<(), ConfigWriteError> {
    if path.is_empty() {
        return Err(ConfigWriteError::InvalidPath { key: String::new() });
    }
    if get(doc, path).is_none() {
        set(doc, path, value)?;
    }
    Ok(())
}

pub fn remove(doc: &mut DocumentMut, path: &[&str]) -> Result<(), ConfigWriteError> {
    let Some((leaf, parents)) = path.split_last() else {
        return Err(ConfigWriteError::InvalidPath {
            key: path.join("."),
        });
    };
    let mut table = doc.as_table_mut();
    for key in parents {
        let Some(entry) = table.get_mut(key) else {
            return Ok(());
        };
        let next = entry
            .as_table_mut()
            .ok_or_else(|| ConfigWriteError::InvalidPath {
                key: path.join("."),
            })?;
        table = next;
    }
    table.remove(leaf);
    Ok(())
}

#[must_use]
pub fn get<'a>(doc: &'a DocumentMut, path: &[&str]) -> Option<&'a Item> {
    let mut item = doc.as_item();
    for key in path {
        item = item.get(key)?;
    }
    Some(item)
}
