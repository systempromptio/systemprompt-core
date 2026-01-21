use std::sync::Arc;

use anyhow::Result;
use systemprompt_extension::runtime_config::{
    set_injected_extensions, InjectedExtensions, WebAssetsStrategy,
};
use systemprompt_extension::Extension;

pub use systemprompt_extension::runtime_config::WebAssetsStrategy as WebAssets;

pub struct RuntimeBuilder {
    extensions: Vec<Arc<dyn Extension>>,
    web_assets: WebAssetsStrategy,
}

impl std::fmt::Debug for RuntimeBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeBuilder")
            .field("extension_count", &self.extensions.len())
            .field("web_assets", &self.web_assets)
            .finish()
    }
}

impl RuntimeBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            web_assets: WebAssetsStrategy::default(),
        }
    }

    #[must_use]
    pub fn with_extension<E: Extension + Default + 'static>(mut self) -> Self {
        self.extensions.push(Arc::new(E::default()));
        self
    }

    #[must_use]
    pub fn with_extension_instance(mut self, ext: Arc<dyn Extension>) -> Self {
        self.extensions.push(ext);
        self
    }

    #[must_use]
    pub fn with_web_assets(mut self, strategy: WebAssetsStrategy) -> Self {
        self.web_assets = strategy;
        self
    }

    pub async fn run(self) -> Result<()> {
        let config = InjectedExtensions {
            extensions: self.extensions,
            web_assets: self.web_assets,
        };
        set_injected_extensions(config)
            .map_err(|_| anyhow::anyhow!("InjectedExtensions already set"))?;
        systemprompt_cli::run().await
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
