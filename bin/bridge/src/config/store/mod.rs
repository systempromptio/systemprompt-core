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

    #[error(
        "HKLM\\{subkey} already holds different values for {} — Claude ignores HKCU while that \
         machine policy exists; remove it or re-run `install --apply` as Administrator",
        differing.join(", ")
    )]
    HiveConflict {
        subkey: String,
        differing: Vec<String>,
    },

    #[error("{hive}\\{subkey}\\{name} did not read back with the value just written")]
    VerifyMismatch {
        hive: String,
        subkey: String,
        name: String,
    },
}

/// What a managed-policy write actually did, so a caller never reports a
/// write that was skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyWrite {
    Written(PolicyHive),
    /// HKLM already holds identical values; the per-user copy would be ignored.
    SatisfiedByMachine,
}

#[must_use]
pub const fn hive_for(elevated: bool) -> PolicyHive {
    if elevated {
        PolicyHive::Machine
    } else {
        PolicyHive::User
    }
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

pub const LEGACY_MANIFEST_PUBKEY_KEY: &str = "inferenceManifestPubkey";

// Why: Claude Desktop 1.44121 logs inferenceManifestPubkey as an unrecognized
// policy key.
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

#[cfg(target_os = "windows")]
pub(crate) fn read_bridge_policy_in(
    hive: PolicyHive,
    key: &str,
) -> Result<Option<String>, ConfigStoreError> {
    windows_registry::read_string(windows_registry::hkey(hive), &bridge_policy_subkey(), key)
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
) -> Result<PolicyWrite, ConfigStoreError> {
    let hive = hive_for(elevated);
    if hive == PolicyHive::User && machine_policy_satisfies(entries)? {
        tracing::info!(
            subkey = crate::cowork_compat::POLICY_SUBKEY,
            "HKLM already holds these Claude policy values; leaving the machine policy in force"
        );
        return Ok(PolicyWrite::SatisfiedByMachine);
    }
    let typed = typed_strings(entries);
    windows_registry_write::write_policy_values(hive, &typed)?;
    Ok(PolicyWrite::Written(hive))
}

/// Cowork ignores HKCU once `HKLM\SOFTWARE\Policies\Claude` exists, so a
/// per-user write is only honest when the machine key is absent or already
/// says the same thing.
#[cfg(target_os = "windows")]
fn machine_policy_satisfies(entries: &[(String, String)]) -> Result<bool, ConfigStoreError> {
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    let machine =
        windows_registry::WindowsRegistryStore.read_policy_document(PolicyHive::Machine, &names)?;
    if machine.is_empty() {
        return Ok(false);
    }
    let differing: Vec<String> = entries
        .iter()
        .filter(|(name, value)| {
            machine.get(name).and_then(PolicyDocumentValue::as_str) != Some(value.as_str())
        })
        .map(|(name, _)| name.clone())
        .collect();
    if differing.is_empty() {
        Ok(true)
    } else {
        Err(ConfigStoreError::HiveConflict {
            subkey: crate::cowork_compat::POLICY_SUBKEY.to_owned(),
            differing,
        })
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn write_bridge_policy(
    elevated: bool,
    entries: &[(String, String)],
) -> Result<(), ConfigStoreError> {
    let typed = typed_strings(entries);
    windows_registry_write::write_values_at(hive_for(elevated), &bridge_policy_subkey(), &typed)
}

#[cfg(target_os = "windows")]
fn typed_strings(entries: &[(String, String)]) -> Vec<(String, PolicyDocumentValue)> {
    entries
        .iter()
        .map(|(n, v)| (n.clone(), PolicyDocumentValue::Str(v.clone())))
        .collect()
}

#[cfg(target_os = "windows")]
pub(crate) fn clear_managed_claude_policy(
    elevated: bool,
    names: &[&str],
) -> Result<usize, ConfigStoreError> {
    windows_registry_write::delete_policy_values(hive_for(elevated), names)
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
