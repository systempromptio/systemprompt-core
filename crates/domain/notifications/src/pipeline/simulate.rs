//! [`NotificationPipeline::simulate`]: the delivery decisions for one event,
//! computed on a read connection without writing anything.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::{NotificationEvent, SimulateResponse, SimulatedRecipient};

use super::context::BatchContext;
use super::plan::{self, Decision};
use super::{NotificationPipeline, visibility};
use crate::error::NotificationsResult;

impl NotificationPipeline {
    pub async fn simulate(
        &self,
        event: &NotificationEvent,
    ) -> NotificationsResult<SimulateResponse> {
        let mut conn = self.parts.repos.read_pool().acquire().await?;
        let mut ctx = BatchContext::load(&mut conn).await?;
        let members = self.members();
        let plan = plan::plan(&self.parts.routers, &members, &mut conn, &mut ctx, event).await?;
        let now = Utc::now();
        let decided: Vec<(UserId, Decision)> = plan
            .routed
            .recipients
            .iter()
            .map(|d| (d.user.clone(), plan::decide(event, &plan, d, now)))
            .collect();
        let readers: Vec<(UserId, bool)> = decided
            .iter()
            .map(|(user, _)| (user.clone(), plan.in_audience.contains(user)))
            .collect();
        let verdicts =
            visibility::check_many(&self.parts.visibilities, &event.subjects, &readers).await;
        let decisions = plan
            .routed
            .recipients
            .iter()
            .zip(decided)
            .zip(verdicts)
            .map(
                |((routed, (user, decision)), verdict)| match (decision, verdict) {
                    (
                        Decision::Deliver {
                            attention,
                            reason,
                            matched,
                            ..
                        },
                        Ok(_),
                    ) => SimulatedRecipient {
                        user,
                        attention: Some(attention),
                        reason,
                        matched_rules: matched,
                        withheld_reason: None,
                    },
                    (Decision::Deliver { matched, .. }, Err(withheld)) => SimulatedRecipient {
                        user,
                        attention: None,
                        reason: routed.reason.clone(),
                        matched_rules: matched,
                        withheld_reason: Some(withheld.to_string()),
                    },
                    (Decision::Muted { matched }, _) => SimulatedRecipient {
                        user,
                        attention: None,
                        reason: routed.reason.clone(),
                        matched_rules: matched,
                        withheld_reason: Some("muted by rule".to_owned()),
                    },
                },
            )
            .collect();
        Ok(SimulateResponse {
            plan: plan.routed,
            decisions,
        })
    }
}
