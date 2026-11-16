//! The [`JobContext`] every dispatched job receives: the resolved kernel
//! state (the kernel's seeds, every admitted member's provided values and
//! built slots, the [`AdmittedSchedule`] the retention job reads its purges,
//! sweeps and holds from, and the [`AdmittedProjections`] the drainers read)
//! extended with only the per-run handles: the budgeted database, the
//! inventory refresh memo, the signing keys and the application context the
//! jobs that still read it reach through [`job_app_context`].
//!
//! [`AdmittedSchedule`]: systemprompt_extension::AdmittedSchedule
//! [`AdmittedProjections`]: systemprompt_extension::AdmittedProjections
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::BudgetedDb;
use systemprompt_extension::ExtensionContext;
use systemprompt_identifiers::Actor;
use systemprompt_loader::BootContext;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{DbBudget, Dependencies, JobContext};

use crate::error::SchedulerResult;
use crate::jobs::InventoryRefreshMemo;

pub(crate) fn make_job_context(
    actor: Actor,
    app_context: Arc<AppContext>,
    budget: Option<DbBudget>,
    inventory_memo: &Arc<InventoryRefreshMemo>,
) -> JobContext {
    let dependencies = brain_dependencies(&app_context, app_context.kernel().state().clone())
        .with(Arc::clone(inventory_memo))
        .with(app_context.signing_keys().clone())
        .with(BudgetedDb::new(Arc::clone(app_context.db_pool()), budget))
        .with(extension_context(&app_context))
        .with(app_context);
    JobContext::new(actor, dependencies)
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

fn extension_context(app_context: &Arc<AppContext>) -> Arc<dyn ExtensionContext> {
    let concrete = Arc::clone(app_context);
    let ext: Arc<dyn ExtensionContext> = concrete;
    ext
}

pub(crate) fn deployment_dependencies(boot: &BootContext) -> Dependencies {
    Dependencies::new().with(Arc::clone(boot.loaded_profile()))
}

pub(crate) fn job_app_context(ctx: &JobContext) -> SchedulerResult<&Arc<AppContext>> {
    Ok(ctx.get::<Arc<AppContext>>()?)
}
