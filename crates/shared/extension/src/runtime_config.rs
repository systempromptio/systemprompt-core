//! Process-level injection of extensions registered outside of the
//! `inventory` linker collector.
//!
//! When the host binary cannot rely on the `inventory`-based registration
//! path (typically because LTO has stripped the static `submit!` slots),
//! `set_injected_extensions` lets the application install a fallback
//! list of [`crate::Extension`] values plus a `WebAssetsStrategy` that the
//! runtime should honour.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use crate::Extension;

/// Strategy for resolving the web-distribution assets at runtime.
#[derive(Debug, Clone, Default)]
pub enum WebAssetsStrategy {
    /// Web assets are not exposed by this process.
    #[default]
    Disabled,
    /// Web assets are read from the given on-disk directory.
    FilePath(PathBuf),
    /// Web assets are downloaded from a remote URL and cached on disk.
    Remote {
        /// Remote URL to fetch assets from.
        url: String,
        /// Local cache directory.
        cache_dir: PathBuf,
    },
}

/// Bundle of extensions and web-asset strategy injected at process start.
#[derive(Default)]
pub struct InjectedExtensions {
    /// Extensions to register in addition to those collected by
    /// `inventory`.
    pub extensions: Vec<Arc<dyn Extension>>,
    /// How the runtime should serve web assets.
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

static INJECTED_EXTENSIONS: OnceLock<InjectedExtensions> = OnceLock::new();

/// Installs the process-wide injected-extensions value. Returns the
/// supplied value back as `Err` if a value has already been installed
/// (the `OnceLock` is single-shot).
pub fn set_injected_extensions(config: InjectedExtensions) -> Result<(), InjectedExtensions> {
    INJECTED_EXTENSIONS.set(config)
}

/// Returns the injected extension list, or an empty `Vec` if none were
/// installed.
#[must_use]
pub fn get_injected_extensions() -> Vec<Arc<dyn Extension>> {
    INJECTED_EXTENSIONS
        .get()
        .map_or_else(Vec::new, |config| config.extensions.clone())
}

/// Returns the injected web-assets strategy, or
/// [`WebAssetsStrategy::Disabled`].
#[must_use]
pub fn get_web_assets_strategy() -> WebAssetsStrategy {
    INJECTED_EXTENSIONS
        .get()
        .map_or(WebAssetsStrategy::Disabled, |config| {
            config.web_assets.clone()
        })
}
