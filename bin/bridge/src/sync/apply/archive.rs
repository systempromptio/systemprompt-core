//! Unpacking a plugin archive into staging, trusting nothing in it but bytes.
//!
//! The archive is only a transport: every entry must be a regular file the
//! signed manifest names, at most once, no larger than the manifest says, and
//! hashing to the manifest's digest. Links, devices, absolute paths and `..`
//! are refused outright, and a file the manifest names but the archive lacks
//! fails the plugin — so a staged tree is exactly the manifest's tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path};

use super::ApplyError;
use super::safe_path::join_under;
use crate::gateway::manifest::{PluginEntry, PluginFile};
use crate::hash::sha256_hex;

pub fn extract_verified(
    bytes: &[u8],
    plugin: &PluginEntry,
    stage: &Path,
) -> Result<(), ApplyError> {
    let mut pending: BTreeMap<&str, &PluginFile> =
        plugin.files.iter().map(|f| (f.path.as_str(), f)).collect();
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    let entries = archive
        .entries()
        .map_err(|e| archive_io(plugin, "read archive", e))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| archive_io(plugin, "read archive entry", e))?;
        let raw_path = entry
            .path()
            .map_err(|e| archive_io(plugin, "read archive entry path", e))?
            .into_owned();
        let relative = normalise(&raw_path).ok_or_else(|| unsafe_entry(plugin, &raw_path))?;
        match entry.header().entry_type() {
            tar::EntryType::Directory | tar::EntryType::XGlobalHeader => continue,
            tar::EntryType::Regular | tar::EntryType::Continuous => {},
            _ => return Err(unsafe_entry(plugin, &raw_path)),
        }
        let file = pending
            .remove(relative.as_str())
            .ok_or_else(|| unsafe_entry(plugin, &raw_path))?;
        let mut contents = Vec::new();
        entry
            .by_ref()
            .take(file.size.saturating_add(1))
            .read_to_end(&mut contents)
            .map_err(|e| archive_io(plugin, "read archive file", e))?;
        let actual = sha256_hex(&contents);
        if actual != file.sha256.as_str() {
            return Err(ApplyError::HashMismatch {
                what: format!("file {}/{}", plugin.id, file.path),
                expected: file.sha256.clone(),
                actual,
            });
        }
        let out = join_under(stage, &file.path)?;
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| ApplyError::Io {
                context: format!("create parent {}", parent.display()),
                source: e,
            })?;
        }
        fs::write(&out, &contents).map_err(|e| ApplyError::Io {
            context: format!("write {}", out.display()),
            source: e,
        })?;
    }
    if let Some(missing) = pending.keys().next() {
        return Err(archive_io(
            plugin,
            "complete archive",
            std::io::Error::other(format!("archive lacks manifest file {missing}")),
        ));
    }
    Ok(())
}

// Why: tar entries may carry a leading `./`; anything but plain name
// components (root, prefix, `..`) could escape the staging directory.
fn normalise(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {},
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn unsafe_entry(plugin: &PluginEntry, path: &Path) -> ApplyError {
    ApplyError::UnsafePath(format!("{}: archive entry {}", plugin.id, path.display()))
}

fn archive_io(plugin: &PluginEntry, what: &str, source: std::io::Error) -> ApplyError {
    ApplyError::Io {
        context: format!("{what} for plugin {}", plugin.id),
        source,
    }
}
