//! Files the bridge writes under `~/.config/opencode` (agents, bundle plugins,
//! themes), recorded in a ledger beside them so the next sync removes exactly
//! the ones it no longer projects. A ledger that cannot be read is an error:
//! a corrupt record must not read as "nothing is ours".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::host_sync::ApplyError;

const LEDGER_FILE: &str = ".systemprompt-owned.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct Ledger {
    #[serde(default)]
    files: Vec<String>,
}

const fn io(context: String, source: std::io::Error) -> ApplyError {
    ApplyError::Io { context, source }
}

fn ledger_path(root: &Path) -> PathBuf {
    root.join(LEDGER_FILE)
}

fn read_ledger(root: &Path) -> Result<Ledger, ApplyError> {
    let path = ledger_path(root);
    let Some(text) = crate::fsutil::read_optional(&path)
        .map_err(|e| io(format!("read {}", path.display()), e))?
    else {
        return Ok(Ledger::default());
    };
    serde_json::from_str(&text).map_err(|source| ApplyError::Serialize {
        what: path.display().to_string(),
        source,
    })
}

pub(super) fn is_plain_relative(relative: &str) -> bool {
    let path = Path::new(relative);
    !relative.is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

pub(super) fn sync(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<(), ApplyError> {
    let previous = read_ledger(root)?;
    for relative in &previous.files {
        if !files.contains_key(relative) && is_plain_relative(relative) {
            remove(&root.join(relative))?;
        }
    }
    for (relative, bytes) in files {
        if !is_plain_relative(relative) {
            return Err(ApplyError::UnsafePath(relative.clone()));
        }
        let path = root.join(relative);
        if std::fs::read(&path).is_ok_and(|current| &current == bytes) {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| io(format!("create {}", parent.display()), e))?;
        }
        crate::fsutil::atomic_write_0644(&path, bytes)
            .map_err(|e| io(format!("write {}", path.display()), e))?;
        crate::fsutil::verify_contents(&path, bytes)
            .map_err(|e| io(format!("verify {}", path.display()), e))?;
    }
    let ledger = Ledger {
        files: files.keys().cloned().collect(),
    };
    let path = ledger_path(root);
    if ledger.files.is_empty() {
        return remove(&path);
    }
    let bytes = serde_json::to_vec_pretty(&ledger).map_err(|source| ApplyError::Serialize {
        what: path.display().to_string(),
        source,
    })?;
    crate::fsutil::atomic_write_0644(&path, &bytes)
        .map_err(|e| io(format!("write {}", path.display()), e))
}

pub(super) fn remove(path: &Path) -> Result<(), ApplyError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io(format!("remove {}", path.display()), source)),
    }
}
