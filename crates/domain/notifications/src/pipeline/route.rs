//! The routing step: every registered router plans recipients and the plans
//! merge, the louder decision winning. A router that fails is dropped and
//! recorded; the others still deliver. Each router runs under the resolver
//! timeout, and a router that times out fails the whole fact with
//! [`NotificationsError::RouteTimeout`], which is transient, so the fact is
//! retried rather than delivered without that router's recipients.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::notifications::NotificationRouter;
use systemprompt_models::notifications::{NotificationEvent, RoutePlan};

use super::StepFailure;
use crate::error::{NotificationsError, NotificationsResult};
use crate::repository::FailureStage;

pub(crate) async fn route(
    routers: &[Arc<dyn NotificationRouter>],
    event: &NotificationEvent,
    timeout: Duration,
) -> NotificationsResult<(RoutePlan, Vec<StepFailure>)> {
    let mut plan = RoutePlan::default();
    let mut failures = Vec::new();
    for router in routers {
        match tokio::time::timeout(timeout, router.route(event)).await {
            Ok(Ok(contribution)) => plan.merge(contribution),
            Ok(Err(error)) => failures.push(StepFailure::new(
                FailureStage::Route,
                None,
                format!("{}: {error}", router.name()),
            )),
            Err(_) => {
                tracing::warn!(router = router.name(), "Notification router timed out");
                return Err(NotificationsError::RouteTimeout {
                    router: router.name(),
                });
            },
        }
    }
    Ok((plan, failures))
}
