//! Scheduled job extension trait.

use std::sync::Arc;

use systemprompt_traits::Job;

use crate::types::ExtensionMeta;

pub trait JobExtensionTyped: ExtensionMeta {
    fn jobs(&self) -> Vec<Arc<dyn Job>>;
}
