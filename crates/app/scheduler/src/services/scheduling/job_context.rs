//! The [`JobContext`] every dispatched job receives: the dependencies the
//! jobs read, assembled once per run from the application context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::{BudgetedDb, DbPool};
use systemprompt_extension::ExtensionContext;
use systemprompt_identifiers::Actor;
use systemprompt_loader::BootContext;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{DbBudget, Dependencies, JobContext};

use crate::error::SchedulerResult;
use crate::jobs::InventoryRefreshMemo;

pub(crate) fn make_job_context(
    actor: Actor,
    db_pool: DbPool,
    app_context: Arc<AppContext>,
    budget: Option<DbBudget>,
    inventory_memo: &Arc<InventoryRefreshMemo>,
) -> JobContext {
    let dependencies = brain_dependencies(&app_context, Dependencies::new())
        .with(Arc::clone(inventory_memo))
        .with(Arc::clone(app_context.app_paths_arc()))
        .with(Arc::clone(app_context.files_config()))
        .with(Arc::new(app_context.profile().notifications))
        .with(Arc::new(app_context.profile().brain.clone()))
        .with(Arc::clone(app_context.documents().service()))
        .with(app_context.signing_keys().clone())
        .with(app_context.services_state().view())
        .with(BudgetedDb::new(Arc::clone(&db_pool), budget))
        .with(db_pool)
        .with(Arc::clone(app_context.config_arc()))
        .with(Arc::clone(app_context.oauth_repositories()))
        .with(extension_context(&app_context))
        .with(app_context);
    JobContext::new(actor, dependencies)
}

/// What the brain jobs read from the context, present only with the
/// `brain` feature: the notification sink `brain_review` asks for, the
/// consolidation model client, and the producer sink once the brain
/// subsystem is up.
#[cfg(feature = "brain")]
fn brain_dependencies(app_context: &AppContext, dependencies: Dependencies) -> Dependencies {
    let mut dependencies = dependencies
        .with(Arc::clone(&app_context.notifications().sink))
        .with(app_context.brain_consolidation_model());
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
