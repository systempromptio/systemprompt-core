//! [`JobExtensionTyped`] — typed capability manifest for extensions that
//! contribute jobs.
//!
//! This is introspection only. The scheduler discovers and runs jobs from the
//! `inventory` catalog (`submit_job!`); it never reads this trait. The manifest
//! exists so CLI/plugin tooling can attribute a job to its owning extension.

use std::sync::Arc;

use systemprompt_provider_contracts::Job;

use crate::types::ExtensionMeta;

pub trait JobExtensionTyped: ExtensionMeta {
    fn jobs(&self) -> Vec<Arc<dyn Job>>;
}
