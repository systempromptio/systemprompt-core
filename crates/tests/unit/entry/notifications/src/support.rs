use std::sync::Arc;
use systemprompt_test_fixtures::AppContextSeeds as _;

use axum::Router;
use systemprompt_extension::router::RouteSet;
use systemprompt_notifications::NotificationsServiceSlot;
use systemprompt_notifications_entry::http::{admin_routes, user_routes};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};
use systemprompt_users::DevicePushService;

fn mounted(ctx: &AppContext, set: &RouteSet) -> Router {
    let slot = ctx
        .kernel()
        .state()
        .get::<std::sync::Arc<NotificationsServiceSlot>>()
        .expect("the fixture context provides the notification service handle")
        .clone();
    let devices = ctx
        .kernel()
        .state()
        .get::<Arc<DevicePushService>>()
        .expect("the fixture context provides the device push service")
        .clone();
    let database: Arc<dyn DatabaseHandle> = ctx.db_pool().clone();
    let kernel = Kernel::new(
        database,
        BackgroundTasks::new(),
        Dependencies::default().with(slot).with(devices),
    );
    set.materialize(&kernel.resolved())
        .expect("the fixture context binds the notification service")
        .into_iter()
        .fold(Router::new(), Router::merge)
}

pub(crate) fn user_router(ctx: &AppContext) -> Router {
    mounted(ctx, &user_routes())
}

pub(crate) fn admin_router(ctx: &AppContext) -> Router {
    mounted(ctx, &admin_routes())
}
