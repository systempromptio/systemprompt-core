//! The [`JobContext`] every dispatched job receives: the resolved kernel
//! state (the kernel's open seeds, every admitted member's provided values
//! and built slots, the [`AdmittedSchedule`] the retention job reads its
//! purges, sweeps and holds from, and the [`AdmittedProjections`] the
//! drainers read) extended with only the per-run handles: the budgeted
//! database, the inventory refresh memo and the signing keys.
//! [`JobContext::new`] withholds every sealed seed.
//!
//! A job the scheduler member itself contributes is kernel work and also
//! receives [`KernelJobHandles`]. The type is private to this crate, so no
//! member's job can name it, and it is inserted only when the dispatched
//! job's admitted owner is the scheduler member: a member's job context never
//! carries it, and nothing in a member's job context yields the application
//! context, the runtime `Config`, the secret store or the profile.
//!
//! [`AdmittedSchedule`]: systemprompt_extension::AdmittedSchedule
//! [`AdmittedProjections`]: systemprompt_extension::AdmittedProjections
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::BudgetedDb;
use systemprompt_extension::AdmittedSchedule;
use systemprompt_identifiers::Actor;
use systemprompt_loader::BootContext;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{DbBudget, Dependencies, JobContext};

use crate::error::SchedulerResult;
use crate::extension::SCHEDULER_MEMBER_ID;
use crate::jobs::InventoryRefreshMemo;

pub(crate) struct KernelJobHandles {
    app: Arc<AppContext>,
}

impl KernelJobHandles {
    pub(crate) const fn app(&self) -> &Arc<AppContext> {
        &self.app
    }
}

pub(crate) struct JobRun<'a> {
    pub(crate) actor: Actor,
    pub(crate) job_name: &'a str,
    pub(crate) budget: Option<DbBudget>,
}

pub(crate) fn make_job_context(
    run: JobRun<'_>,
    app_context: &Arc<AppContext>,
    schedule: &AdmittedSchedule,
    inventory_memo: &Arc<InventoryRefreshMemo>,
) -> JobContext {
    let mut dependencies =
        brain_dependencies(app_context, app_context.kernel().state().clone())
            .with(Arc::clone(inventory_memo))
            .with(app_context.signing_keys().clone())
            .with(BudgetedDb::new(Arc::clone(app_context.db_pool()), run.budget));
    if is_kernel_job(schedule, run.job_name) {
        dependencies.insert(KernelJobHandles {
            app: Arc::clone(app_context),
        });
    }
    JobContext::new(run.actor, dependencies)
}

fn is_kernel_job(schedule: &AdmittedSchedule, job_name: &str) -> bool {
    schedule
        .job(job_name)
        .is_some_and(|(owner, _)| owner.as_str() == SCHEDULER_MEMBER_ID)
}

#[cfg(feature = "brain")]
fn brain_dependencies(app_context: &AppContext, dependencies: Dependencies) -> Dependencies {
    let mut dependencies = dependencies.with(app_context.brain_consolidation_model());
    if let Some(sink) = app_context.brain_sink() {
        dependencies.insert(sink);
    }
    dependencies
}

#[cfg(not(feature = "brain"))]
const fn brain_dependencies(_: &AppContext, dependencies: Dependencies) -> Dependencies {
    dependencies
}

pub(crate) fn deployment_dependencies(boot: &BootContext) -> Dependencies {
    Dependencies::new().with(Arc::clone(boot.loaded_profile()))
}

pub(crate) fn kernel_handles(ctx: &JobContext) -> SchedulerResult<&KernelJobHandles> {
    Ok(ctx.get::<KernelJobHandles>()?)
}
