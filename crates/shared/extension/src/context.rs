use std::sync::Arc;
use systemprompt_traits::{ConfigProvider, DatabaseHandle};

pub trait ExtensionContext: Send + Sync {
    fn config(&self) -> Arc<dyn ConfigProvider>;

    fn database(&self) -> Arc<dyn DatabaseHandle>;

    fn get_extension(&self, id: &str) -> Option<Arc<dyn crate::Extension>>;

    fn has_extension(&self, id: &str) -> bool {
        self.get_extension(id).is_some()
    }
}

pub type DynExtensionContext = Arc<dyn ExtensionContext>;
