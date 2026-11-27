//! The persisted account store: one JSON file, written owner-only and
//! atomically with `sp_secure::write_private`.
//!
//! The file is versioned and tolerant: fields this build does not know are kept
//! on every rewrite, so a newer build's additions survive an older one. A file
//! that does not decode, or names a version this build does not read, is
//! reported and left exactly as it is; only [`set_aside_corrupt_store`], an
//! explicit user decision after `AccountsError::StoreCorrupt` or
//! `AccountsError::StoreVersion` was shown, moves it to `<name>.corrupt-<UTC
//! time>` so the next start begins empty. Secrets in it are sealed
//! (`SealedString`); a PAT kept for one run only is never written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sp_protocol::ManifestVersion;
use sp_protocol::ids::{AccountId, DeviceId, InstallId, InstanceKey, UserId};
use sp_protocol::view::KeyFingerprint;
use sp_secure::SealedString;

use crate::error::AccountsError;
use crate::slug::unique_slug;

pub(crate) const STORE_VERSION: u32 = 1;

const FALLBACK_NAME: &str = "accounts";

type Extra = serde_json::Map<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoreFile {
    pub(crate) version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) install_id: Option<InstallId>,
    #[serde(default)]
    pub(crate) accounts: Vec<AccountRecord>,
    #[serde(flatten)]
    pub(crate) extra: Extra,
}

impl Default for StoreFile {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            install_id: None,
            accounts: Vec::new(),
            extra: Extra::new(),
        }
    }
}

impl StoreFile {
    pub(crate) fn find(&self, id: &AccountId) -> Option<&AccountRecord> {
        self.accounts.iter().find(|record| &record.id == id)
    }

    pub(crate) fn find_mut(&mut self, id: &AccountId) -> Result<&mut AccountRecord, AccountsError> {
        self.accounts
            .iter_mut()
            .find(|record| &record.id == id)
            .ok_or_else(|| AccountsError::UnknownAccount {
                account: id.clone(),
            })
    }

    pub(crate) fn active(&self) -> Option<&AccountRecord> {
        self.accounts.iter().find(|record| record.active)
    }

    pub(crate) fn taken_keys(&self) -> Vec<&str> {
        self.accounts
            .iter()
            .filter_map(|record| record.instance_key.as_ref().map(InstanceKey::as_str))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AccountRecord {
    pub(crate) id: AccountId,
    pub(crate) label: String,
    pub(crate) url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) instance_key: Option<InstanceKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) user_id: Option<UserId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) org: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pat: Option<SealedString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pinned_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pinned_fingerprint: Option<KeyFingerprint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) offered_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) applied_version: Option<ManifestVersion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) device: Option<DeviceRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) outbox: Option<String>,
    #[serde(default)]
    pub(crate) active: bool,
    pub(crate) added_at: DateTime<Utc>,
    #[serde(flatten)]
    pub(crate) extra: Extra,
}

impl AccountRecord {
    pub(crate) fn new(
        id: AccountId,
        label: String,
        url: String,
        instance_key: InstanceKey,
        added_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            label,
            url,
            instance_key: Some(instance_key),
            user: None,
            user_id: None,
            org: None,
            pat: None,
            pinned_key: None,
            pinned_fingerprint: None,
            offered_key: None,
            applied_version: None,
            device: None,
            outbox: None,
            active: false,
            added_at,
            extra: Extra::new(),
        }
    }

    pub(crate) fn forget_user_state(&mut self) {
        self.applied_version = None;
        self.device = None;
        self.outbox = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeviceRecord {
    pub(crate) device_id: DeviceId,
    pub(crate) user: UserId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) installation: Option<String>,
    pub(crate) credential: SealedString,
    #[serde(flatten)]
    pub(crate) extra: Extra,
}

pub(crate) fn load(path: &Path) -> Result<Option<StoreFile>, AccountsError> {
    let bytes = sp_secure::read_optional(path).map_err(|source| AccountsError::StoreRead {
        path: path.to_owned(),
        source,
    })?;
    bytes.map(|bytes| decode(path, &bytes)).transpose()
}

fn decode(path: &Path, bytes: &[u8]) -> Result<StoreFile, AccountsError> {
    let store: StoreFile =
        serde_json::from_slice(bytes).map_err(|error| AccountsError::StoreCorrupt {
            path: path.to_owned(),
            category: category(&error),
            line: error.line(),
            column: error.column(),
        })?;
    if store.version == 0 || store.version > STORE_VERSION {
        return Err(AccountsError::StoreVersion {
            path: path.to_owned(),
            found: store.version,
        });
    }
    Ok(store)
}

pub(crate) fn encode(store: &StoreFile) -> Result<Vec<u8>, AccountsError> {
    let mut bytes = serde_json::to_vec_pretty(store).map_err(AccountsError::StoreEncode)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(crate) fn save(path: &Path, bytes: &[u8]) -> Result<(), AccountsError> {
    sp_secure::write_private(path, bytes).map_err(|source| AccountsError::StoreWrite {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn normalize(store: &mut StoreFile) -> bool {
    let mut changed = store.version != STORE_VERSION;
    store.version = STORE_VERSION;
    let mut seen_active = false;
    for record in &mut store.accounts {
        if record.active && (seen_active || record.pat.is_none()) {
            record.active = false;
            changed = true;
        }
        seen_active |= record.active;
    }
    assign_missing_keys(store) || changed
}

fn assign_missing_keys(store: &mut StoreFile) -> bool {
    let mut taken: Vec<String> = Vec::new();
    let mut missing: Vec<usize> = Vec::new();
    for (index, record) in store.accounts.iter().enumerate() {
        match record.instance_key.as_ref().map(InstanceKey::as_str) {
            Some(key) if !key.is_empty() && !taken.iter().any(|seen| seen == key) => {
                taken.push(key.to_owned());
            },
            _ => missing.push(index),
        }
    }
    let changed = !missing.is_empty();
    for index in missing {
        let Some(record) = store.accounts.get_mut(index) else {
            continue;
        };
        let refs: Vec<&str> = taken.iter().map(String::as_str).collect();
        let key = unique_slug(&record.label, &refs);
        record.instance_key = Some(InstanceKey::new(key.clone()));
        taken.push(key);
    }
    changed
}

pub fn set_aside_corrupt_store(path: &Path) -> Result<PathBuf, AccountsError> {
    let name = path.file_name().map_or_else(
        || FALLBACK_NAME.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let aside = path.with_file_name(format!("{name}.corrupt-{stamp}"));
    std::fs::rename(path, &aside).map_err(|source| AccountsError::StoreSetAside {
        path: path.to_owned(),
        source,
    })?;
    tracing::warn!(
        path = %path.display(),
        aside = %aside.display(),
        "set the unreadable account store aside"
    );
    Ok(aside)
}

fn category(error: &serde_json::Error) -> &'static str {
    match error.classify() {
        serde_json::error::Category::Io => "io",
        serde_json::error::Category::Syntax => "syntax",
        serde_json::error::Category::Data => "data",
        serde_json::error::Category::Eof => "eof",
    }
}
