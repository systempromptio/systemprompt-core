use std::collections::BTreeMap;

#[cfg(target_os = "macos")]
mod macos_managed_prefs;
#[cfg(target_os = "windows")]
mod windows_registry;

#[derive(Debug, thiserror::Error)]
pub enum ConfigStoreError {
    #[error("config store: {0}")]
    Backend(String),
}

#[derive(Debug, Default)]
pub struct ManagedPolicyRead {
    pub source: Option<String>,
    pub values: BTreeMap<String, String>,
}

pub trait ConfigStore: Send + Sync {
    fn read_managed_policy(&self, key: &str) -> Result<Option<String>, ConfigStoreError>;

    fn read_managed_policy_keys(
        &self,
        keys: &[&str],
    ) -> Result<ManagedPolicyRead, ConfigStoreError>;
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
    fn read_managed_policy(&self, _key: &str) -> Result<Option<String>, ConfigStoreError> {
        Ok(None)
    }

    fn read_managed_policy_keys(
        &self,
        _keys: &[&str],
    ) -> Result<ManagedPolicyRead, ConfigStoreError> {
        Ok(ManagedPolicyRead::default())
    }
}
