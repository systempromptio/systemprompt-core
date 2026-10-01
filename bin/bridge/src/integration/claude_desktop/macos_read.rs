//! Reading the Claude Desktop managed-preferences domain on macOS: the
//! per-user and machine plists, then the managed policy store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::process::Command;

use super::shared::{API_KEY_KEY, DomainRead, KEYS_OF_INTEREST, redact_if_sensitive};

#[derive(Debug, thiserror::Error)]
enum DomainReadError {
    #[error("{}: {source}", path.display())]
    Exists {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{}: run plutil: {source}", path.display())]
    PlutilSpawn {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{}: plutil exited {status}: {stderr}", path.display())]
    PlutilFailed {
        path: PathBuf,
        status: std::process::ExitStatus,
        stderr: String,
    },
    #[error("{}: {source}", path.display())]
    PlistJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("read managed policy key {key}: {source}")]
    ManagedPolicy {
        key: String,
        #[source]
        source: crate::config::store::ConfigStoreError,
    },
}

pub(super) fn read_domain(domain: &str) -> DomainRead {
    let mut out = DomainRead::default();
    if let Err(e) = read_domain_into(domain, &mut out) {
        out.keys.clear();
        out.api_key_fp = None;
        out.probe_error = Some(e.to_string());
    }
    out
}

fn read_domain_into(domain: &str, out: &mut DomainRead) -> Result<(), DomainReadError> {
    let mut plist_path = None;
    for candidate in super::macos::candidates(domain) {
        let exists = candidate
            .try_exists()
            .map_err(|source| DomainReadError::Exists {
                path: candidate.clone(),
                source,
            })?;
        if exists {
            plist_path = Some(candidate);
            break;
        }
    }

    if let Some(path) = plist_path.as_ref() {
        out.source_path = Some(path.display().to_string());
    }

    let plist_json = plist_path
        .as_deref()
        .map(read_plist_as_json)
        .transpose()?
        .unwrap_or(serde_json::Value::Null);

    for key in KEYS_OF_INTEREST {
        if let Some(raw) = read_key_raw(&plist_json, domain, key)? {
            if *key == API_KEY_KEY {
                out.api_key_fp = Some(crate::proxy::secret::fingerprint(raw.trim()));
            }
            out.keys
                .insert((*key).to_owned(), redact_if_sensitive(key, raw));
        }
    }
    Ok(())
}

// JSON: Claude Desktop plist — native preferences read back as JSON.
fn read_plist_as_json(path: &Path) -> Result<serde_json::Value, DomainReadError> {
    let output = Command::new("/usr/bin/plutil")
        .arg("-convert")
        .arg("json")
        .arg("-o")
        .arg("-")
        .arg(path)
        .output()
        .map_err(|source| DomainReadError::PlutilSpawn {
            path: path.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(DomainReadError::PlutilFailed {
            path: path.to_path_buf(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|source| DomainReadError::PlistJson {
        path: path.to_path_buf(),
        source,
    })
}

// JSON: Claude Desktop plist — native preferences read back as JSON.
fn read_key_raw(
    plist_json: &serde_json::Value,
    _domain: &str,
    key: &str,
) -> Result<Option<String>, DomainReadError> {
    if let Some(val) = plist_json.get(key) {
        return Ok(Some(format_plist_value(val)));
    }

    let raw = crate::config::store::managed_policy_store()
        .read_managed_policy(key)
        .map_err(|source| DomainReadError::ManagedPolicy {
            key: key.to_owned(),
            source,
        })?;
    Ok(raw
        .map(|raw| raw.trim().to_owned())
        .filter(|trimmed| !trimmed.is_empty()))
}

// Why: an array of objects (`allowedWorkspaceFolders`, `managedMcpServers`)
// rendered through a strings-only join printed as empty, which hid a plist
// whose entries Claude Desktop was dropping as malformed.
// JSON: Claude Desktop plist — native preferences read back as JSON.
fn format_plist_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) if items.iter().all(serde_json::Value::is_string) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}
