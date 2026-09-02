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
