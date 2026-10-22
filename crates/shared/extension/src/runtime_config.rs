//! Extensions registered outside of the `inventory` linker collector.
//!
//! When the host binary cannot rely on the `inventory`-based registration
//! path (typically because LTO has stripped the static `submit!` slots), it
//! hands an [`InjectedExtensions`] value — the fallback list of
//! [`crate::Extension`] values plus the `WebAssetsStrategy` the runtime
//! should honour — to [`crate::ExtensionRegistry::discover_with`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use crate::Extension;

#[derive(Debug, Clone, Default)]
pub enum WebAssetsStrategy {
    #[default]
    Disabled,
    FilePath(PathBuf),
    Remote {
        url: String,
        cache_dir: PathBuf,
    },
}

#[derive(Default)]
pub struct InjectedExtensions {
    pub extensions: Vec<Arc<dyn Extension>>,
    pub web_assets: WebAssetsStrategy,
}

impl std::fmt::Debug for InjectedExtensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InjectedExtensions")
            .field("extension_count", &self.extensions.len())
            .field("web_assets", &self.web_assets)
            .finish()
    }
}
