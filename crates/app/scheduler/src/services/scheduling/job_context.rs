//! The [`JobContext`] every dispatched job receives: the application's
//! extension state (the kernel's handles and every enabled extension's
//! provided values) extended with the per-run handles the jobs read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::{BudgetedDb, DbPool};
use systemprompt_extension::ExtensionContext;
use systemprompt_identifiers::Actor;
use systemprompt_loader::BootContext;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{DbBudget, Dependencies, DynIpBanStore, JobContext};
use systemprompt_users::BannedIpRepository;

use crate::error::SchedulerResult;
use crate::jobs::InventoryRefreshMemo;

pub(crate) fn make_job_context(
    actor: Actor,
    db_pool: DbPool,
    app_context: Arc<AppContext>,
    budget: Option<DbBudget>,
    inventory_memo: &Arc<InventoryRefreshMemo>,
) -> JobContext {
    let ip_bans: DynIpBanStore = Arc::new(BannedIpRepository::new(&db_pool));
    let mut dependencies = brain_dependencies(&app_context, app_context.extension_state())
        .with(Arc::clone(inventory_memo))
        .with(Arc::new(app_context.profile().brain.clone()))
        .with(Arc::clone(app_context.documents().service()))
        .with(app_context.signing_keys().clone())
        .with(app_context.services_state().view())
        .with(BudgetedDb::new(Arc::clone(&db_pool), budget))
        .with(db_pool)
        .with(Arc::clone(app_context.config_arc()))
        .with(extension_context(&app_context))
        .with(ip_bans)
        .with(Arc::clone(app_context.analytics_repositories()));
    if let Some(reader) = app_context.geoip_reader() {
        dependencies.insert(reader.clone());
    }
    dependencies.insert(app_context);
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
