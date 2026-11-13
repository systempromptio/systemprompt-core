//! Scopes a job run executes inside.
//!
//! A crate whose services read task-local context set by the caller (the files
//! crate's document actor) registers a [`JobRunScope`] with
//! `register_job_run_scope!`. The scheduler runs every job through
//! [`run_in_job_scopes`], which wraps the job's future in every registered
//! scope, so a job's writes carry the identity the scope sets for as long as
//! the crate that registers it is linked. Registration is by linking alone: a
//! scope is never skipped because its owning extension is disabled, since
//! leaving it out would change who a job acts as rather than what runs.
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

/// A [`JobRunScope`] with the id of the extension that registers it.
#[derive(Clone, Copy)]
pub struct JobRunScopeRegistration {
    pub owner: &'static str,
    pub scope: &'static dyn JobRunScope,
}

impl std::fmt::Debug for JobRunScopeRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobRunScopeRegistration")
            .field("owner", &self.owner)
            .field("scope", &self.scope.name())
            .finish()
    }
}

inventory::collect!(JobRunScopeRegistration);

pub fn registered_job_run_scopes() -> impl Iterator<Item = &'static JobRunScopeRegistration>
{
    inventory::iter::<JobRunScopeRegistration>.into_iter()
}

pub fn run_in_job_scopes<'a, F>(
    job: &'static str,
    run: F,
) -> BoxFuture<'a, ProviderResult<JobResult>>
where
    F: std::future::Future<Output = ProviderResult<JobResult>> + Send + 'a,
{
    registered_job_run_scopes().fold(Box::pin(run), |inner, registration| {
        registration.scope.scope(job, inner)
    })
}

#[macro_export]
macro_rules! register_job_run_scope {
    ($owner:literal, $scope:expr) => {
        $crate::__inventory::submit! {
            $crate::job_scope::JobRunScopeRegistration {
                owner: $owner,
                scope: $scope as &'static dyn $crate::job_scope::JobRunScope,
            }
        }
    };
}
