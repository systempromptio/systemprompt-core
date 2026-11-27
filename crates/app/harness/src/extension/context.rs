//! What the composition root hands every instance-side extension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_engine::config::{Layout, Paths};
use systemprompt_identifiers::engine::ExtensionId;
use systemprompt_device::accounts::Accounts;
use systemprompt_device::client::HttpClient;

use crate::extension::enablement::Enablement;
use crate::extension::handles::Handles;
use crate::extension::settings::{ExtensionSettings, SettingsAccess};

/// The product this build presents, as the brand names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostInfo {
    pub product: &'static str,
    pub display_name: &'static str,
    pub version: &'static str,
}

/// The directories an extension may use, all injected from the brand's data
/// root. `documents` is the user's Documents directory when the platform has
/// one.
#[derive(Debug, Clone)]
pub struct SpPaths {
    pub engine: Paths,
    pub layout: Layout,
    pub state: PathBuf,
    pub cache: PathBuf,
    pub documents: Option<PathBuf>,
}

impl SpPaths {
    pub fn extension_state(&self, extension: &ExtensionId) -> PathBuf {
        self.state.join("extensions").join(extension.as_str())
    }

    pub fn extension_cache(&self, extension: &ExtensionId) -> PathBuf {
        self.cache.join("extensions").join(extension.as_str())
    }
}

/// Everything an instance-side extension works with, built once by the
/// composition root and cheap to clone.
///
/// `accounts` follows sign-in and hands out each account's instance client and
/// credentials; `http` is the one instance HTTP client; `handles` carries what
/// extensions share; `enablement` is the applied instance's enablement.
#[derive(Clone)]
pub struct SpContext {
    pub host: HostInfo,
    pub paths: SpPaths,
    pub accounts: Accounts,
    pub http: HttpClient,
    pub settings: Arc<dyn SettingsAccess>,
    pub handles: Handles,
    pub enablement: Enablement,
}

impl SpContext {
    pub fn settings_for(&self, extension: &ExtensionId) -> ExtensionSettings {
        ExtensionSettings::new(extension.clone(), Arc::clone(&self.settings))
    }
}

impl fmt::Debug for SpContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpContext")
            .field("host", &self.host)
            .field("paths", &self.paths)
            .field("handles", &self.handles)
            .field("enablement", &self.enablement)
            .finish_non_exhaustive()
    }
}
