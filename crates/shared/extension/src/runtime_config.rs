use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

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

static INJECTED_EXTENSIONS: OnceLock<InjectedExtensions> = OnceLock::new();

pub fn set_injected_extensions(config: InjectedExtensions) -> Result<(), InjectedExtensions> {
    INJECTED_EXTENSIONS.set(config)
}

pub fn get_injected_extensions() -> Vec<Arc<dyn Extension>> {
    INJECTED_EXTENSIONS
        .get()
        .map(|c| c.extensions.clone())
        .unwrap_or_default()
}

pub fn get_web_assets_strategy() -> WebAssetsStrategy {
    INJECTED_EXTENSIONS
        .get()
        .map(|c| c.web_assets.clone())
        .unwrap_or_default()
}
