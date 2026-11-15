use std::sync::Arc;

use axum::Router;
use systemprompt_extension::ExtensionContext;
use systemprompt_extension::router::RouteSet;
use systemprompt_notifications::NotificationsServiceSlot;
use systemprompt_notifications_entry::http::user_routes;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

fn mounted(ctx: &AppContext, set: &RouteSet) -> Router {
    let slot = ctx
        .extension_state()
        .get::<NotificationsServiceSlot>()
        .expect("the fixture context provides the notification service handle")
        .clone();
    let database: Arc<dyn DatabaseHandle> = Arc::clone(ctx.db_pool());
    let kernel = Kernel::new(
        database,
        BackgroundTasks::new(),
        Dependencies::default().with(slot),
    );
    set.materialize(&kernel.resolved())
        .expect("the fixture context binds the notification service")
        .into_iter()
        .fold(Router::new(), Router::merge)
}

pub(crate) fn user_router(ctx: &AppContext) -> Router {
    mounted(ctx, &user_routes())
}
