//! The routing step: every registered router plans recipients and the plans
//! merge, the louder decision winning. A failed router's contribution is
//! dropped and recorded; the others still deliver.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::notifications::NotificationRouter;
use systemprompt_models::notifications::{NotificationEvent, RoutePlan};

use super::StepFailure;

pub(crate) async fn route(
    routers: &[Arc<dyn NotificationRouter>],
    event: &NotificationEvent,
) -> (RoutePlan, Vec<StepFailure>) {
    let mut plan = RoutePlan::default();
    let mut failures = Vec::new();
    for router in routers {
        match router.route(event).await {
            Ok(contribution) => plan.merge(contribution),
            Err(error) => failures.push(StepFailure::new(
                "route",
                None,
                format!("{}: {error}", router.name()),
            )),
        }
    }
    (plan, failures)
}
