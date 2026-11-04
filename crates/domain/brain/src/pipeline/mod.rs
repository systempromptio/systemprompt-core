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
//! [`BrainPipeline::process_with`] takes [`ProcessParams`]: the [`Reindex`]
//! mode (`Now` refreshes each landed fact's derived indexes inside the
//! write, for console edits, replay and the one-event drain; `Deferred`
//! leaves them to the caller, which collects [`EventSummary::facts`] and
//! reindexes the set once) and, for a consumer round, the [`RoundCache`]:
//! the round then owns the result ledger (set claim before, set finish
//! after, the event's result in [`EventSummary::result`]) and the alias
//! and dedupe reads come from the cache, with every committed write
//! reported in [`EventSummary::writes`] for the round to apply.
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
pub mod round;
pub mod round_extractions;
mod round_load;
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
pub use persist::{Effect, Persisted};
pub use round::{CachedFact, DedupeWrite, RoundCache, RoundRows};
pub use seams::PipelineSeams;
pub use sweep::SweepReport;

use self::candidate::EventScope;
use self::filter::FilterDecision;
use crate::defaults::ModelExtractor;
use crate::error::BrainResult;
use crate::index::Reindex;
use crate::models::EventOutcome;
use crate::repository::{BrainRepositories, EventResultRepository};

/// How one event is processed: the round's declarations, when its facts'
/// indexes are written, and the round cache when a consumer round owns the
/// result ledger and the dedupe reads.
#[derive(Debug, Clone, Copy)]
pub struct ProcessParams<'a> {
    pub ctx: &'a BatchContext,
    pub reindex: Reindex,
    pub round: Option<&'a RoundCache>,
}

impl<'a> ProcessParams<'a> {
    #[must_use]
    pub const fn new(ctx: &'a BatchContext) -> Self {
        Self {
            ctx,
            reindex: Reindex::Now,
            round: None,
        }
    }
}

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
        self.process_with(delivery_id, event, conn, &ProcessParams::new(ctx))
            .await
    }

    pub async fn process_with(
        &self,
        delivery_id: &EventOutboxId,
        event: BrainEvent,
        conn: &mut PgConnection,
        params: &ProcessParams<'_>,
    ) -> BrainResult<ProcessOutcome> {
        let ctx = params.ctx;
        let event = normalise::normalise(delivery_id.clone(), event, Utc::now())?;
        let kind = event.envelope.event.kind.as_str();
        let claimed = match params.round {
            Some(round) => round.claimed(delivery_id),
            None => {
                EventResultRepository::claim(&mut *conn, delivery_id, ctx.version(), kind).await?
            },
        };
        if !claimed {
            return Ok(ProcessOutcome::Skipped);
        }
        let staled = stale::mark_stale(conn, &event).await?;
        let mut summary = EventSummary {
            staled: staled.len(),
            ..EventSummary::default()
        };
        if !staled.is_empty() {
            summary.writes.push(DedupeWrite::Staled(staled));
        }
        let decision = filter::decide(&self.seams.filters, &self.profile.capture, &event.envelope);
        summary.filter = decision.filter_name().map(str::to_owned);
        let kinds: Vec<&KindDecl> = match &decision {
            FilterDecision::NoFilter => {
                return outcome::finish(conn, params, &event, summary.with(EventOutcome::NoFilter))
                    .await;
            },
            FilterDecision::Ignored { .. } => {
                return outcome::finish(conn, params, &event, summary.with(EventOutcome::Ignored))
                    .await;
            },
            FilterDecision::Gather { kinds, .. } => gathered(ctx, kinds),
        };
        let extracted = self.extract(conn, params, &event, &kinds).await?;
        if let Some(reason) = extracted.unreadable {
            return outcome::unreadable(conn, params, &event, summary, reason).await;
        }
        let scope = EventScope {
            event: &event,
            text: extracted.text.as_ref(),
        };
        for candidate in extracted.candidates {
            let (result, write) = self.candidate(conn, params, &scope, candidate).await?;
            summary.record(&result);
            summary.writes.extend(write);
        }
        let result = summary.tally.outcome();
        outcome::finish(conn, params, &event, summary.with(result)).await
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
