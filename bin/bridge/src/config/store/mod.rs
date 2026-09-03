//! `ConfigStore` abstraction over per-OS managed-policy sources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

pub mod document;
#[cfg(target_os = "macos")]
mod macos_managed_prefs;
#[cfg(target_os = "macos")]
mod macos_plist_store;
pub mod plist;
#[cfg(target_os = "windows")]
mod windows_registry;
mod windows_registry_write;

pub use document::{PolicyDocument, PolicyDocumentValue, PolicyHive};

#[derive(Debug, thiserror::Error)]
pub enum ConfigStoreError {
    #[error("config store: {0}")]
    Backend(String),

    #[error("administrator rights required to write {subkey} under {hive}")]
    AccessDenied { hive: String, subkey: String },
}

#[derive(Debug, Default)]
pub struct ManagedPolicyRead {
    pub source: Option<String>,
    pub values: BTreeMap<String, String>,
}

/// One managed-policy backend per OS.
///
/// Reads answer "what is in force"; the hive-addressed document methods are
/// the only way the bridge writes policy, so a fake store can stand in for the
/// registry or the plist in tests.
pub trait ConfigStore: Send + Sync {
    fn read_managed_policy(&self, key: &str) -> Result<Option<String>, ConfigStoreError>;

    fn read_managed_policy_keys(
        &self,
        keys: &[&str],
    ) -> Result<ManagedPolicyRead, ConfigStoreError>;

    fn read_policy_document(
        &self,
        hive: PolicyHive,
        keys: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError>;

    fn write_policy_values(
        &self,
        hive: PolicyHive,
        entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError>;

    fn delete_policy_values(
        &self,
        hive: PolicyHive,
        names: &[&str],
    ) -> Result<usize, ConfigStoreError>;

    fn delete_policy_key(&self, hive: PolicyHive) -> Result<bool, ConfigStoreError>;
}

pub const MANIFEST_PUBKEY_KEY: &str = "manifestPubkey";

// Why: where the same key used to live, inside Claude's own policy hive. Sync
// clears it and `validate` warns while one remains, so it is named from both
// `install` and `validate` — it belongs beside its replacement, below both.
pub const LEGACY_MANIFEST_PUBKEY_KEY: &str = "inferenceManifestPubkey";

// Why: the manifest pubkey used to ride in Claude's own policy hive as
// `inferenceManifestPubkey`. Claude Desktop 1.44121 logs it as an unrecognized
// key and ignores it; a later build may reject the hive. It is the bridge's
// policy, so it lives under a bridge-owned location keyed by the brand.
#[must_use]
pub fn bridge_policy_subkey() -> String {
    format!(r"SOFTWARE\Policies\{}", crate::brand::brand().config_dir)
}

#[must_use]
pub fn bridge_policy_domain() -> String {
    format!("io.systemprompt.{}", crate::brand::brand().config_dir)
}

#[cfg(target_os = "windows")]
#[must_use]
pub fn read_bridge_policy(key: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let subkey = bridge_policy_subkey();
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        if let Ok(Some(v)) = windows_registry::read_string(hive, &subkey, key) {
            return Some(v);
        }
    }
    None
}

#[cfg(target_os = "macos")]
#[must_use]
pub fn read_bridge_policy(key: &str) -> Option<String> {
    macos_plist_store::read_string_at(&macos_plist_store::bridge_plist_path(), key)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[must_use]
pub const fn read_bridge_policy(_key: &str) -> Option<String> {
    None
}

#[must_use]
pub fn managed_policy_store() -> Box<dyn ConfigStore> {
    #[cfg(target_os = "windows")]
    {
        Box::new(windows_registry::WindowsRegistryStore)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos_managed_prefs::MacOsManagedPrefsStore)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Box::new(NoopStore)
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn write_managed_claude_policy(
    elevated: bool,
    entries: &[(String, String)],
) -> Result<(), ConfigStoreError> {
    let hive = if elevated {
        PolicyHive::Machine
    } else {
        PolicyHive::User
    };
    let typed: Vec<(String, PolicyDocumentValue)> = entries
        .iter()
        .map(|(n, v)| (n.clone(), PolicyDocumentValue::Str(v.clone())))
        .collect();
    windows_registry_write::write_policy_values(hive, &typed)
}

#[cfg(target_os = "windows")]
pub(crate) fn write_bridge_policy(
    elevated: bool,
    entries: &[(String, String)],
) -> Result<(), ConfigStoreError> {
    let hive = if elevated {
        PolicyHive::Machine
    } else {
        PolicyHive::User
    };
    let typed: Vec<(String, PolicyDocumentValue)> = entries
        .iter()
        .map(|(n, v)| (n.clone(), PolicyDocumentValue::Str(v.clone())))
        .collect();
    windows_registry_write::write_values_at(hive, &bridge_policy_subkey(), &typed)
}

#[cfg(target_os = "windows")]
pub(crate) fn clear_managed_claude_policy(
    elevated: bool,
    names: &[&str],
) -> Result<usize, ConfigStoreError> {
    let hive = if elevated {
        PolicyHive::Machine
    } else {
        PolicyHive::User
    };
    windows_registry_write::delete_policy_values(hive, names)
}

#[cfg(target_os = "windows")]
pub(crate) fn read_registry_string(
    hive: windows_sys::Win32::System::Registry::HKEY,
    subkey: &str,
    name: &str,
) -> Result<Option<String>, ConfigStoreError> {
    windows_registry::read_string(hive, subkey, name)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
struct NoopStore;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl ConfigStore for NoopStore {
    fn read_managed_policy(&self, _key: &str) -> Result<Option<String>, ConfigStoreError> {
        Ok(None)
    }

    fn read_managed_policy_keys(
        &self,
        _keys: &[&str],
    ) -> Result<ManagedPolicyRead, ConfigStoreError> {
        Ok(ManagedPolicyRead::default())
    }

    fn read_policy_document(
        &self,
        _hive: PolicyHive,
        _keys: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError> {
        Ok(PolicyDocument::new())
    }

    fn write_policy_values(
        &self,
        _hive: PolicyHive,
        _entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError> {
        Ok(())
    }

    fn delete_policy_values(
        &self,
        _hive: PolicyHive,
        _names: &[&str],
    ) -> Result<usize, ConfigStoreError> {
        Ok(0)
    }

    fn delete_policy_key(&self, _hive: PolicyHive) -> Result<bool, ConfigStoreError> {
        Ok(false)
    }
}
