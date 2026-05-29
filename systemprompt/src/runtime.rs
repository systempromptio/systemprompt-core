//! Embedding helper for the `systemprompt` CLI.
//!
//! [`RuntimeBuilder`] lets a custom binary inject [`Extension`] instances at
//! compile time, choose a [`WebAssets`] strategy, and then hand control to the
//! standard CLI entry point. It is the recommended way to embed the platform
//! when the default `cargo install systemprompt` binary is not enough.
//!
//! ```no_run
//! # #[cfg(all(feature = "runtime", feature = "core"))]
//! # async fn demo() -> Result<(), systemprompt::RuntimeError> {
//! use systemprompt::RuntimeBuilder;
//!
//! RuntimeBuilder::new().run().await
//! # }
//! ```

use std::sync::Arc;

use systemprompt_extension::Extension;
use systemprompt_extension::runtime_config::{
    InjectedExtensions, WebAssetsStrategy, set_injected_extensions,
};
use thiserror::Error;

/// Web-asset serving strategy re-exported from `systemprompt-extension`.
pub use systemprompt_extension::runtime_config::WebAssetsStrategy as WebAssets;

/// Typed error returned by [`RuntimeBuilder::run`].
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// Extensions have already been injected for this process; calling
    /// [`RuntimeBuilder::run`] more than once per process is not supported.
    #[error(
        "InjectedExtensions already set: a RuntimeBuilder has already been run in this process"
    )]
    ExtensionsAlreadyInjected,

    /// The CLI exited with an error. Wraps the original CLI failure.
    #[error("CLI exited with error: {0}")]
    Cli(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Fluent builder that wires extensions and a web-asset strategy into the CLI
/// before delegating to `systemprompt_cli::run`.
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

    /// Register an extension by type, instantiating it via
    /// [`Default::default`].
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

    /// Inject the configured extensions and run the CLI.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::ExtensionsAlreadyInjected`] if a previous call
    /// to `run` already initialised the global injection slot, or
    /// [`RuntimeError::Cli`] if the CLI itself exits with an error.
    pub async fn run(self) -> Result<(), RuntimeError> {
        let config = InjectedExtensions {
            extensions: self.extensions,
            web_assets: self.web_assets,
        };
        set_injected_extensions(config).map_err(|_e| RuntimeError::ExtensionsAlreadyInjected)?;
        Box::pin(systemprompt_cli::run())
            .await
            .map_err(|err| RuntimeError::Cli(err.into()))
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
