//! Extension context trait - provides access to core services.

use std::sync::Arc;
use systemprompt_traits::{ConfigProvider, DatabaseHandle};

/// Context provided to extensions during initialization and runtime.
///
/// This trait provides extensions with access to core services like
/// configuration, database, and other registered extensions.
pub trait ExtensionContext: Send + Sync {
    /// Get the configuration provider.
    fn config(&self) -> Arc<dyn ConfigProvider>;

    /// Get the database handle.
    fn database(&self) -> Arc<dyn DatabaseHandle>;

    /// Get a registered extension by its ID.
    ///
    /// Returns `None` if no extension with the given ID is registered.
    fn get_extension(&self, id: &str) -> Option<Arc<dyn crate::Extension>>;

    /// Check if an extension is registered.
    fn has_extension(&self, id: &str) -> bool {
        self.get_extension(id).is_some()
    }
}

/// A dynamic reference to an extension context.
pub type DynExtensionContext = Arc<dyn ExtensionContext>;
