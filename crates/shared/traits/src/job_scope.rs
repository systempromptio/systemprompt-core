//! Scopes a job run executes inside.
//!
//! A member whose services read task-local context set by the caller (the
//! files member's document actor) contributes a [`JobRunScope`] to the
//! admitted schedule. The scheduler wraps every job's future in each admitted
//! scope, so a job's writes carry the identity the scope sets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures::future::BoxFuture;
use systemprompt_provider_contracts::{JobResult, ProviderResult};

/// Wraps one job run. `job` is the running job's registered name.
pub trait JobRunScope: Send + Sync {
    fn name(&self) -> &'static str;

    fn scope<'a>(
        &self,
        job: &'static str,
        run: BoxFuture<'a, ProviderResult<JobResult>>,
    ) -> BoxFuture<'a, ProviderResult<JobResult>>;
}
