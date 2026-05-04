//! Runtime context handed to extensions during router resolution.

use std::sync::Arc;
use systemprompt_traits::{ConfigProvider, DatabaseHandle};

/// Runtime context that lets an extension reach into the configuration,
/// database, and sibling-extension lookup at router-build time.
pub trait ExtensionContext: Send + Sync {
    /// Returns the active configuration provider.
    fn config(&self) -> Arc<dyn ConfigProvider>;

    /// Returns the active database handle.
    fn database(&self) -> Arc<dyn DatabaseHandle>;

    /// Returns a sibling extension by ID, if registered.
    fn get_extension(&self, id: &str) -> Option<Arc<dyn crate::Extension>>;

    /// Returns true if an extension with the given ID is registered.
    fn has_extension(&self, id: &str) -> bool {
        self.get_extension(id).is_some()
    }
}

/// Type alias for an `Arc<dyn ExtensionContext>` since the trait cannot be
/// boxed by value.
pub type DynExtensionContext = Arc<dyn ExtensionContext>;
