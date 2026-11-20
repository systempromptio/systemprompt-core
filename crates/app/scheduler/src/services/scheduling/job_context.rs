//! The [`JobContext`] every dispatched job receives: the kernel's runtime
//! state ([`systemprompt_runtime::Kernel::runtime_state`]: the open seeds,
//! every admitted member's provided values and built slots, the
//! [`AdmittedSchedule`] the retention job reads its purges, sweeps and holds
//! from, and the [`AdmittedProjections`] the drainers read, every sealed seed
//! withheld) extended with only the per-run handle: the budgeted database. No
//! signing key, secret, profile or runtime
//! `Config` is in it.
//!
//! A job the scheduler member itself contributes is kernel work and also
//! receives [`KernelJobHandles`]. The type is private to this crate, so no
//! member's job can name it, and it is inserted only when the dispatched
//! job's admitted owner is the scheduler member with the core standing the
//! facade stamped: an install cannot take the scheduler's id or core standing,
//! and a job name has one owner, so a member's job context never carries it.
//!
//! [`deployment_dependencies`] is the map
//! [`systemprompt_traits::Job::configured`] reads: the [`JobDeployment`] facts
//! the scheduler's own jobs need, derived from the profile here, and nothing
//! else.
//!
//! [`AdmittedSchedule`]: systemprompt_extension::AdmittedSchedule
//! [`AdmittedProjections`]: systemprompt_extension::AdmittedProjections
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::BudgetedDb;
use systemprompt_extension::{AdmittedSchedule, StandingTag};
use systemprompt_identifiers::{Actor, JobName};
use systemprompt_loader::BootContext;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{DbBudget, Dependencies, JobContext};

use crate::error::SchedulerResult;
use crate::extension::SCHEDULER_MEMBER_ID;

pub(crate) struct KernelJobHandles {
    app: Arc<AppContext>,
}

impl KernelJobHandles {
    pub(crate) const fn app(&self) -> &Arc<AppContext> {
        &self.app
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct JobDeployment {
    pub(crate) licence_checkin: bool,
    pub(crate) otlp_export: bool,
}

pub(crate) struct JobRun<'a> {
    pub(crate) actor: Actor,
    pub(crate) job_name: &'a JobName,
    pub(crate) budget: Option<DbBudget>,
}

pub(crate) fn make_job_context(
    run: JobRun<'_>,
    app_context: &Arc<AppContext>,
    schedule: &AdmittedSchedule,
) -> JobContext {
    let mut dependencies = brain_dependencies(app_context, app_context.kernel().runtime_state())
        .with(BudgetedDb::new(
            Arc::clone(app_context.db_pool()),
            run.budget,
        ));
    if is_kernel_job(schedule, run.job_name) {
        dependencies.insert(KernelJobHandles {
            app: Arc::clone(app_context),
        });
    }
    JobContext::new(run.actor, dependencies)
}

fn is_kernel_job(schedule: &AdmittedSchedule, job_name: &JobName) -> bool {
    schedule
        .owner_of(job_name.as_str())
        .is_some_and(|(owner, standing)| {
            standing == StandingTag::Core && owner.as_str() == SCHEDULER_MEMBER_ID
        })
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
    let profile = boot.loaded_profile();
    Dependencies::new().with(JobDeployment {
        licence_checkin: profile.licence.enabled(),
        otlp_export: profile.observability.otlp().is_some(),
    })
}

pub(crate) fn kernel_handles(ctx: &JobContext) -> SchedulerResult<&KernelJobHandles> {
    Ok(ctx.get::<KernelJobHandles>()?)
}
