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
    let path = plist_path(hive)
        .ok_or_else(|| ConfigStoreError::Backend("per-user policy path unresolvable".to_owned()))?;
    read_document_at(&path, keys)
}

#[must_use]
pub(super) fn bridge_plist_path() -> PathBuf {
    PathBuf::from(MANAGED_PREFS_ROOT).join(format!("{}.plist", super::bridge_policy_domain()))
}

pub(super) fn read_string_at(
    path: &std::path::Path,
    key: &str,
) -> Result<Option<String>, ConfigStoreError> {
    match read_document_at(path, &[key])?.get(key) {
        None => Ok(None),
        Some(PolicyDocumentValue::Str(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ConfigStoreError::Backend(format!(
            "{}: {key} must be a string",
            path.display()
        ))),
    }
}

fn read_document_at(
    path: &std::path::Path,
    keys: &[&str],
) -> Result<PolicyDocument, ConfigStoreError> {
    if !path.try_exists().map_err(|e| map_io(path, &e))? {
        return Ok(PolicyDocument::new());
    }
    let output = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(path)
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
    let obj = json.as_object().ok_or_else(|| {
        ConfigStoreError::Backend(format!("{}: expected plist dictionary", path.display()))
    })?;
    {
        for key in keys {
            if let Some(v) = obj.get(*key) {
                let value = PolicyDocumentValue::from_json(v).ok_or_else(|| {
                    ConfigStoreError::Backend(format!(
                        "{}: unsupported value at {key}",
                        path.display()
                    ))
                })?;
                doc.insert((*key).to_owned(), value);
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
    let path = plist_path(hive)
        .ok_or_else(|| ConfigStoreError::Backend("per-user policy path unresolvable".to_owned()))?;
    if !path.try_exists().map_err(|e| map_io(path, &e))? {
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
    let path = plist_path(hive)
        .ok_or_else(|| ConfigStoreError::Backend("per-user policy path unresolvable".to_owned()))?;
    match std::fs::remove_file(&path) {
        Ok(()) => {
            if path.try_exists().map_err(|e| map_io(&path, &e))? {
                return Err(ConfigStoreError::VerifyMismatch {
                    hive: hive.label().to_owned(),
                    subkey: path.display().to_string(),
                    name: "<deleted key>".to_owned(),
                });
            }
            Ok(true)
        },
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
    let path = plist_path(hive)
        .ok_or_else(|| ConfigStoreError::Backend("per-user policy path unresolvable".to_owned()))?;
    if !path.try_exists().map_err(|e| map_io(path, &e))? {
        return Ok(PolicyDocument::new());
    }
    let output = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(&path)
        .output()
        .map_err(|e| ConfigStoreError::Backend(format!("plutil: {e}")))?;
    if !output.status.success() {
        return Err(ConfigStoreError::Backend(format!(
            "plutil {}: {}: {}",
            path.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ConfigStoreError::Backend(format!("plutil json: {e}")))?;
    let mut doc = PolicyDocument::new();
    let obj = json.as_object().ok_or_else(|| {
        ConfigStoreError::Backend(format!("{}: expected plist dictionary", path.display()))
    })?;
    {
        for (k, v) in obj {
            let value = PolicyDocumentValue::from_json(v).ok_or_else(|| {
                ConfigStoreError::Backend(format!("{}: unsupported value at {k}", path.display()))
            })?;
            doc.insert(k.clone(), value);
        }
    }
    Ok(doc)
}

fn write_document(path: &std::path::Path, doc: &PolicyDocument) -> Result<(), ConfigStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| map_io(path, &e))?;
    }
    crate::fsutil::atomic_write_0644(path, render_plist(doc).as_bytes())
        .map_err(|e| map_io(path, &e))?;
    let output = Command::new("/usr/bin/killall")
        .arg("cfprefsd")
        .output()
        .map_err(|e| map_io(path, &e))?;
    if !output.status.success() && !cfprefsd_was_not_running(&output) {
        return Err(ConfigStoreError::Backend(format!(
            "refresh managed preferences {}: {}: {}",
            path.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

// Why: `killall` exits 1 when there is nothing to kill. A cfprefsd that is
// not running holds no stale cache, so the write is complete.
fn cfprefsd_was_not_running(output: &std::process::Output) -> bool {
    output.status.code() == Some(1)
        && String::from_utf8_lossy(&output.stderr).contains("No matching processes")
}

fn map_io(path: &std::path::Path, e: &std::io::Error) -> ConfigStoreError {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        ConfigStoreError::AccessDenied {
            hive: "Managed Preferences".to_owned(),
            subkey: path.display().to_string(),
        }
    } else {
        ConfigStoreError::Backend(format!("{}: {e}", path.display()))
    }
}
