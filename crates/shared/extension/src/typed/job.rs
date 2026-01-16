use std::sync::Arc;

use systemprompt_provider_contracts::Job;

use crate::types::ExtensionMeta;

pub trait JobExtensionTyped: ExtensionMeta {
    fn jobs(&self) -> Vec<Arc<dyn Job>>;
}
