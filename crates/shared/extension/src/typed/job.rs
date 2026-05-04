//! [`JobExtensionTyped`] — typed contract for extensions that contribute
//! scheduled jobs.

use std::sync::Arc;

use systemprompt_provider_contracts::Job;

use crate::types::ExtensionMeta;

/// Typed contract for an extension that contributes scheduled jobs.
pub trait JobExtensionTyped: ExtensionMeta {
    /// Returns the jobs this extension registers.
    fn jobs(&self) -> Vec<Arc<dyn Job>>;
}
