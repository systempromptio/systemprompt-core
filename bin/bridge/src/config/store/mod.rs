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
pub mod verified;
#[cfg(target_os = "windows")]
mod windows_policy;
#[cfg(target_os = "windows")]
mod windows_registry;
mod windows_registry_write;

pub use document::{PolicyDocument, PolicyDocumentValue, PolicyHive, PolicyTarget};
#[cfg(target_os = "windows")]
pub(crate) use windows_policy::{
    clear_managed_claude_policy, read_registry_string, write_managed_claude_policy,
};

#[derive(Debug, thiserror::Error)]
pub enum ConfigStoreError {
    #[error("policy partially applied ({completed:?}); {source}")]
    Partial {
        completed: Vec<verified::PolicyReceipt>,
        #[source]
        source: Box<Self>,
    },
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

/// Verified policy disposition, including the scope that supplies the effective
/// value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PolicyWrite {
    Written(PolicyHive),
    AlreadyVerified(PolicyHive),
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
    fn policy_key_exists(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
    ) -> Result<bool, ConfigStoreError>;
    fn read_managed_policy(&self, key: &str) -> Result<Option<String>, ConfigStoreError>;

    fn read_managed_policy_keys(
        &self,
        keys: &[&str],
    ) -> Result<ManagedPolicyRead, ConfigStoreError>;

    fn read_policy_document(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
        keys: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError>;

    fn write_policy_values(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
        entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError>;

    fn delete_policy_values(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
        names: &[&str],
    ) -> Result<usize, ConfigStoreError>;

    fn delete_policy_key(&self, hive: PolicyHive) -> Result<bool, ConfigStoreError>;
}

#[derive(Clone)]
pub struct PolicyStore(std::sync::Arc<dyn ConfigStore>);
impl PolicyStore {
    pub fn new(store: Box<dyn ConfigStore>) -> Self {
        Self(store.into())
    }
    pub fn backend(&self) -> &dyn ConfigStore {
        self.0.as_ref()
    }
}
impl std::fmt::Debug for PolicyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyStore").finish_non_exhaustive()
    }
}

pub const MANIFEST_TRUST_KEY: &str = "manifestTrust";

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
pub fn read_bridge_policy(key: &str) -> Result<Option<String>, ConfigStoreError> {
    let subkey = bridge_policy_subkey();
    for hive in [PolicyHive::Machine, PolicyHive::User] {
        if windows_registry::key_exists(hive, &subkey)? {
            return windows_registry::read_string(windows_registry::hkey(hive), &subkey, key);
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
pub fn read_bridge_policy(key: &str) -> Result<Option<String>, ConfigStoreError> {
    macos_plist_store::read_string_at(&macos_plist_store::bridge_plist_path(), key)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature matches the Windows and macOS stores"
)]
pub const fn read_bridge_policy(_key: &str) -> Result<Option<String>, ConfigStoreError> {
    Ok(None)
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

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
struct NoopStore;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl ConfigStore for NoopStore {
    fn policy_key_exists(
        &self,
        _hive: PolicyHive,
        _target: PolicyTarget,
    ) -> Result<bool, ConfigStoreError> {
        Ok(false)
    }
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
        _target: PolicyTarget,
        _keys: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError> {
        Ok(PolicyDocument::new())
    }

    fn write_policy_values(
        &self,
        _hive: PolicyHive,
        _target: PolicyTarget,
        _entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError> {
        Err(ConfigStoreError::Backend(
            "managed policy writes are unsupported on this platform".to_owned(),
        ))
    }

    fn delete_policy_values(
        &self,
        _hive: PolicyHive,
        _target: PolicyTarget,
        _names: &[&str],
    ) -> Result<usize, ConfigStoreError> {
        Ok(0)
    }

    fn delete_policy_key(&self, _hive: PolicyHive) -> Result<bool, ConfigStoreError> {
        Ok(false)
    }
}
