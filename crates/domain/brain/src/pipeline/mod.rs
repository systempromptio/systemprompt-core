//! [`BrainPipeline`]: one claimed outbox event in, revisioned facts out.
//!
//! [`BrainPipeline::process`] runs on the consumer's per-event savepoint
//! with the round's [`BatchContext`], in order: normalise the event, claim
//! it once per pipeline version, stale the facts its source change
//! replaces, run the composite filter, extract candidates, then for each
//! candidate apply the rules, validate, dedupe, route, set its status,
//! persist it and notify, and finally record the event's result. Every
//! stage lives in its own module; this one only dispatches.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod candidate;
pub mod context;
pub mod dedupe;
pub mod extract;
pub mod filter;
pub(crate) mod metrics;
pub mod normalise;
pub mod notify;
mod outcome;
pub mod persist;
pub mod route;
pub mod rules;
mod seams;
pub mod similarity;
pub mod stale;
pub mod status;
mod sweep;
pub mod validate;

use std::sync::Arc;

use chrono::Utc;
use sqlx::PgConnection;
use systemprompt_extension::brain::BrainModelClient;
use systemprompt_extension::notifications::DynNotificationSink;
use systemprompt_identifiers::EventOutboxId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{BrainEvent, KindDecl};

pub use candidate::CandidateResult;
pub use context::BatchContext;
pub use outcome::{EventSummary, ProcessOutcome, Tally};
pub use seams::PipelineSeams;
pub use sweep::SweepReport;

use self::filter::FilterDecision;
use crate::defaults::ModelExtractor;
use crate::error::BrainResult;
use crate::models::EventOutcome;
use crate::repository::{BrainRepositories, EventResultRepository};

/// The capture pipeline over the injected seams, model client, optional
/// notification sink and the brain profile.
pub struct BrainPipeline {
    repos: Arc<BrainRepositories>,
    seams: PipelineSeams,
    model: ModelExtractor,
    notifications: Option<DynNotificationSink>,
    profile: BrainProfile,
}

impl std::fmt::Debug for BrainPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainPipeline")
            .field("seams", &self.seams)
            .field("notifications", &self.notifications.is_some())
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

/// Builds a [`BrainPipeline`]; seams default to none and notifications to
/// unavailable.
#[derive(Debug)]
pub struct BrainPipelineBuilder {
    pipeline: BrainPipeline,
}

impl BrainPipeline {
    #[must_use]
    pub fn builder(
        repos: Arc<BrainRepositories>,
        model: Arc<dyn BrainModelClient>,
        profile: BrainProfile,
    ) -> BrainPipelineBuilder {
        BrainPipelineBuilder {
            pipeline: Self {
                repos,
                seams: PipelineSeams::default(),
                model: ModelExtractor::new(model, &profile.extraction),
                notifications: None,
                profile,
            },
        }
    }

    #[must_use]
    pub const fn profile(&self) -> &BrainProfile {
        &self.profile
    }

    #[must_use]
    pub fn repositories(&self) -> &BrainRepositories {
        &self.repos
    }

    pub async fn process(
        &self,
        delivery_id: &EventOutboxId,
        event: BrainEvent,
        conn: &mut PgConnection,
        ctx: &BatchContext,
    ) -> BrainResult<ProcessOutcome> {
        let event = normalise::normalise(delivery_id.clone(), event, Utc::now())?;
        let kind = event.envelope.event.kind.as_str();
        if !EventResultRepository::claim(&mut *conn, delivery_id, ctx.version(), kind).await? {
            return Ok(ProcessOutcome::Skipped);
        }
        let mut summary = EventSummary {
            staled: stale::mark_stale(conn, &event).await?.len(),
            ..EventSummary::default()
        };
        let decision = filter::decide(&self.seams.filters, &self.profile.capture, &event.envelope);
        summary.filter = decision.filter_name().map(str::to_owned);
        let kinds: Vec<&KindDecl> = match &decision {
            FilterDecision::NoFilter => {
                return outcome::finish(conn, ctx, &event, summary.with(EventOutcome::NoFilter))
                    .await;
            },
            FilterDecision::Ignored { .. } => {
                return outcome::finish(conn, ctx, &event, summary.with(EventOutcome::Ignored))
                    .await;
            },
            FilterDecision::Gather { kinds, .. } => gathered(ctx, kinds),
        };
        let extracted = self.extract(conn, ctx, &event, &kinds).await?;
        if let Some(reason) = extracted.unreadable {
            return outcome::unreadable(conn, ctx, &event, summary, reason).await;
        }
        for candidate in extracted.candidates {
            let result = self
                .candidate(conn, ctx, &event, candidate, extracted.text.as_ref())
                .await?;
            summary.tally.record(&result);
        }
        let result = summary.tally.outcome();
        outcome::finish(conn, ctx, &event, summary.with(result)).await
    }
}

impl BrainPipelineBuilder {
    #[must_use]
    pub fn seams(mut self, seams: PipelineSeams) -> Self {
        self.pipeline.seams = seams;
        self
    }

    #[must_use]
    pub fn notifications(mut self, sink: DynNotificationSink) -> Self {
        self.pipeline.notifications = Some(sink);
        self
    }

    #[must_use]
    pub fn build(self) -> BrainPipeline {
        self.pipeline
    }
}

fn gathered<'a>(
    ctx: &'a BatchContext,
    kinds: &[systemprompt_identifiers::FactKindId],
) -> Vec<&'a KindDecl> {
    if kinds.is_empty() {
        return ctx.kinds().collect();
    }
    kinds.iter().filter_map(|kind| ctx.kind(kind)).collect()
}
