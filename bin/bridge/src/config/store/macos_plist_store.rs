//! Managed Preferences plist files as a policy document store.
//!
//! Reads go through `plutil -convert json` so typed values come back typed;
//! writes render the whole plist and replace the file, which is why a write
//! needs root (the elevator stages and installs the same bytes otherwise).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::Command;

use super::plist::render_plist;
use super::{ConfigStoreError, PolicyDocument, PolicyDocumentValue, PolicyHive};

const MANAGED_PREFS_ROOT: &str = "/Library/Managed Preferences";
const POLICY_DOMAIN: &str = "com.anthropic.claudefordesktop";

#[must_use]
pub(super) fn plist_path(hive: PolicyHive) -> Option<PathBuf> {
    let root = PathBuf::from(MANAGED_PREFS_ROOT);
    match hive {
        PolicyHive::Machine => Some(root.join(format!("{POLICY_DOMAIN}.plist"))),
        PolicyHive::User => {
            let user = std::env::var("USER").ok().filter(|u| !u.is_empty())?;
            Some(root.join(user).join(format!("{POLICY_DOMAIN}.plist")))
        },
    }
}

pub(super) fn read_document(
    hive: PolicyHive,
    keys: &[&str],
) -> Result<PolicyDocument, ConfigStoreError> {
    let Some(path) = plist_path(hive) else {
        return Ok(PolicyDocument::new());
    };
    if !path.exists() {
        return Ok(PolicyDocument::new());
    }
    let output = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(&path)
        .output()
        .map_err(|e| ConfigStoreError::Backend(format!("plutil: {e}")))?;
    if !output.status.success() {
        return Err(ConfigStoreError::Backend(format!(
            "plutil exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ConfigStoreError::Backend(format!("plutil json: {e}")))?;
    let mut doc = PolicyDocument::new();
    if let Some(obj) = json.as_object() {
        for key in keys {
            if let Some(v) = obj.get(*key).and_then(PolicyDocumentValue::from_json) {
                doc.insert((*key).to_owned(), v);
            }
        }
    }
    Ok(doc)
}

pub(super) fn write_values(
    hive: PolicyHive,
    entries: &[(String, PolicyDocumentValue)],
) -> Result<(), ConfigStoreError> {
    let Some(path) = plist_path(hive) else {
        return Err(ConfigStoreError::Backend(
            "no $USER for the per-user plist".into(),
        ));
    };
    let mut doc = read_all(hive)?;
    for (name, value) in entries {
        doc.insert(name.clone(), value.clone());
    }
    write_document(&path, &doc)
}

pub(super) fn delete_values(hive: PolicyHive, names: &[&str]) -> Result<usize, ConfigStoreError> {
    let Some(path) = plist_path(hive) else {
        return Ok(0);
    };
    if !path.exists() {
        return Ok(0);
    }
    let mut doc = read_all(hive)?;
    let before = doc.len();
    for name in names {
        doc.remove(*name);
    }
    let removed = before - doc.len();
    if removed > 0 {
        write_document(&path, &doc)?;
    }
    Ok(removed)
}

pub(super) fn delete_key(hive: PolicyHive) -> Result<bool, ConfigStoreError> {
    let Some(path) = plist_path(hive) else {
        return Ok(false);
    };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(ConfigStoreError::AccessDenied {
                hive: hive.label().to_owned(),
                subkey: path.display().to_string(),
            })
        },
        Err(e) => Err(ConfigStoreError::Backend(format!(
            "remove {}: {e}",
            path.display()
        ))),
    }
}

fn read_all(hive: PolicyHive) -> Result<PolicyDocument, ConfigStoreError> {
    let Some(path) = plist_path(hive) else {
        return Ok(PolicyDocument::new());
    };
    if !path.exists() {
        return Ok(PolicyDocument::new());
    }
    let output = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(&path)
        .output()
        .map_err(|e| ConfigStoreError::Backend(format!("plutil: {e}")))?;
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ConfigStoreError::Backend(format!("plutil json: {e}")))?;
    let mut doc = PolicyDocument::new();
    if let Some(obj) = json.as_object() {
        for (k, v) in obj {
            if let Some(value) = PolicyDocumentValue::from_json(v) {
                doc.insert(k.clone(), value);
            }
        }
    }
    Ok(doc)
}

fn write_document(path: &std::path::Path, doc: &PolicyDocument) -> Result<(), ConfigStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| map_io(path, e))?;
    }
    std::fs::write(path, render_plist(doc)).map_err(|e| map_io(path, e))?;
    _ = Command::new("/usr/bin/killall").arg("cfprefsd").status();
    Ok(())
}

fn map_io(path: &std::path::Path, e: std::io::Error) -> ConfigStoreError {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        ConfigStoreError::AccessDenied {
            hive: "Managed Preferences".to_owned(),
            subkey: path.display().to_string(),
        }
    } else {
        ConfigStoreError::Backend(format!("{}: {e}", path.display()))
    }
}
