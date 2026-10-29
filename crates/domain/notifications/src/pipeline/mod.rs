//! [`NotificationPipeline`]: one outbox fact in, one row per recipient out.
//!
//! [`NotificationPipeline::process`] runs on the consumer's per-fact
//! transaction with the round's [`BatchContext`]: route, gate and expand the
//! audience, apply the policy steps, decide visibility for every candidate
//! at once, and persist every row with one statement. Every withheld
//! recipient and failed step is recorded in `notification_delivery_failures`
//! on the same transaction. A re-processed fact is idempotent per recipient.
//! It never runs channels: rows commit as `pending` (or `held` under a
//! digest), and [`NotificationPipeline::deliver`] announces the committed
//! `pending` rows and moves them to `delivered`.
//! [`NotificationPipeline::sweep`] releases due `held` rows and delivers
//! every row `pending` for longer than the sweep grace, so a crash between
//! commit and delivery, or a channel's transient failure, only delays an
//! announcement.
//!
//! [`NotificationPipeline::simulate`] runs the same planning and decisions on
//! a read connection and writes nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod action;
mod audience;
pub mod coalesce;
mod context;
mod deliver;
mod ledger;
mod membership;
pub(crate) mod metrics;
mod plan;
mod process;
mod route;
pub mod rules;
mod simulate;
mod sweep;
pub mod visibility;
mod withheld;

use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::notifications::{
    NotificationAudienceResolver, NotificationChannel, NotificationRouter, ResourceVisibility,
};
use systemprompt_identifiers::{NotificationId, UserId};
use systemprompt_manifest::profile::NotificationsProfile;

pub use action::ActionPromptBuilder;
pub use context::BatchContext;
pub use deliver::DeliverOutcome;
pub use ledger::{DELIVERY_ABANDONED, MAX_DELIVERY_ATTEMPTS};
pub use rules::{RuleContext, RuleEvaluator, RuleOutcome};
pub use withheld::Withheld;

use self::membership::Members;
use crate::repository::NotificationRepositories;

/// The seams and repositories a pipeline runs over, assembled at the
/// composition root from the `discover_notification_*` registrations.
#[derive(Clone)]
pub struct PipelineParts {
    pub routers: Vec<Arc<dyn NotificationRouter>>,
    pub audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    pub visibilities: Vec<Arc<dyn ResourceVisibility>>,
    pub channels: Vec<Arc<dyn NotificationChannel>>,
    pub repos: Arc<NotificationRepositories>,
    pub profile: NotificationsProfile,
}

impl std::fmt::Debug for PipelineParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineParts")
            .field("routers", &self.routers.len())
            .field("audiences", &self.audiences.len())
            .field("visibilities", &self.visibilities.len())
            .field("channels", &self.channels.len())
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

/// What processing one event did. `pending` holds the committed-to-be rows
/// to announce once the fact's transaction commits; digest-held rows are
/// persisted but left for the sweep.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessOutcome {
    pub persisted: Vec<NotificationId>,
    pub pending: Vec<NotificationId>,
    pub held: u32,
    pub withheld: u32,
    pub coalesced: u32,
}

#[derive(Debug)]
pub struct NotificationPipeline {
    parts: PipelineParts,
}

#[derive(Debug, Clone)]
pub(crate) struct StepFailure {
    pub(crate) stage: String,
    pub(crate) recipient: Option<UserId>,
    pub(crate) reason: String,
}

impl StepFailure {
    pub(crate) fn new(stage: &str, recipient: Option<UserId>, reason: impl Into<String>) -> Self {
        Self {
            stage: stage.to_owned(),
            recipient,
            reason: reason.into(),
        }
    }

    pub(crate) fn withheld(stage: &str, recipient: Option<UserId>, why: &Withheld) -> Self {
        match why {
            Withheld::Resolver(source) => {
                tracing::warn!(stage, error = %source, "Notification audience resolver failed");
            },
            Withheld::Visibility(source) => {
                tracing::warn!(stage, error = %source, "Notification visibility check failed");
            },
            _ => {},
        }
        Self::new(stage, recipient, why.to_string())
    }

    pub(crate) fn row(&self) -> (Option<UserId>, String, String) {
        (
            self.recipient.clone(),
            self.stage.clone(),
            self.reason.clone(),
        )
    }
}

impl NotificationPipeline {
    #[must_use]
    pub const fn new(parts: PipelineParts) -> Self {
        Self { parts }
    }

    #[must_use]
    pub const fn profile(&self) -> &NotificationsProfile {
        &self.parts.profile
    }

    #[must_use]
    pub fn repositories(&self) -> &NotificationRepositories {
        &self.parts.repos
    }

    #[must_use]
    pub fn handles_resource_kind(&self, kind: &str) -> bool {
        visibility::has_provider(&self.parts.visibilities, kind)
    }

    pub(crate) fn members(&self) -> Members<'_> {
        Members {
            resolvers: &self.parts.audiences,
            limit: usize::try_from(self.parts.profile.max_audience).unwrap_or(usize::MAX),
            timeout: Duration::from_millis(self.parts.profile.consumer.resolver_timeout_ms),
        }
    }
}
