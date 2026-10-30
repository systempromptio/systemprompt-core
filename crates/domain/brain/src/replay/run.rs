//! [`ReplayService::dry_run`]: replays the brain consumer's events of a
//! window under a chosen pipeline version on one transaction that is
//! always rolled back, then diffs the facts it wrote against the stored
//! ones. Extraction model calls do happen, capped by `max_events` and a
//! share of the daily token budget; nothing they record survives.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::Utc;
use sqlx::{Acquire, PgConnection};
use systemprompt_events::services::durable::{OutboxConsumer, OutboxReplayWindow, ReplayItem};
use systemprompt_extension::brain::{AnalysisWindow, BrainModelClient};
use systemprompt_identifiers::PipelineVersion;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{BRAIN_CONSUMER, BRAIN_EVENT_VERSION, BrainEvent};

use super::budget::CountingModel;
use super::diff::{self, ReplayReport};
use super::store::ReplayRepository;
use crate::error::BrainResult;
use crate::pipeline::{BatchContext, BrainPipeline, PipelineSeams};
use crate::plane::load_declared_state;
use crate::repository::BrainRepositories;

const PAGE: i64 = 100;
const DEFAULT_MAX_EVENTS: u64 = 500;
const BUDGET_SHARE: u64 = 4;

/// Dry-runs the pipeline over past events; owns nothing it writes.
#[derive(Debug, Clone)]
pub struct ReplayService {
    repos: Arc<BrainRepositories>,
    outbox: OutboxConsumer,
    seams: PipelineSeams,
    profile: BrainProfile,
    max_events: u64,
}

impl ReplayService {
    #[must_use]
    pub fn new(
        repos: Arc<BrainRepositories>,
        outbox: OutboxConsumer,
        seams: PipelineSeams,
        profile: BrainProfile,
    ) -> Self {
        Self {
            repos,
            outbox,
            seams,
            profile,
            max_events: DEFAULT_MAX_EVENTS,
        }
    }

    #[must_use]
    pub const fn with_max_events(mut self, max_events: u64) -> Self {
        self.max_events = max_events;
        self
    }

    pub async fn dry_run(
        &self,
        window: AnalysisWindow,
        version: PipelineVersion,
        model: Arc<dyn BrainModelClient>,
    ) -> BrainResult<ReplayReport> {
        let budget = self.profile.extraction.daily_token_budget / BUDGET_SHARE;
        let counting = Arc::new(CountingModel::new(model, budget));
        let client: Arc<dyn BrainModelClient> = counting.clone();
        let pipeline =
            BrainPipeline::builder(Arc::clone(&self.repos), client, self.profile.clone())
                .seams(self.seams.clone())
                .build();
        let mut report = ReplayReport {
            pipeline_version: version.to_string(),
            ..ReplayReport::default()
        };
        let mut tx = self.repos.write_pool().begin().await?;
        let docs = load_declared_state(&mut tx).await?.docs();
        let ctx = BatchContext::from_docs(docs, version, Utc::now());
        let run = Run {
            pipeline: &pipeline,
            ctx: &ctx,
            counting: &counting,
        };
        let replayed = self.replay_pages(&run, &mut tx, window, &mut report).await;
        let written = match replayed {
            Ok(()) => ReplayRepository::written_in_transaction(&mut tx).await,
            Err(error) => {
                tx.rollback().await?;
                return Err(error);
            },
        };
        tx.rollback().await?;
        let written = written?;
        let stored = ReplayRepository::stored(self.repos.read_pool(), &written).await?;
        diff::tally(&mut report, &written, &stored);
        report.tokens = counting.spent();
        Ok(report)
    }

    async fn replay_pages(
        &self,
        run: &Run<'_>,
        conn: &mut PgConnection,
        window: AnalysisWindow,
        report: &mut ReplayReport,
    ) -> BrainResult<()> {
        let mut page = OutboxReplayWindow {
            since: window.since,
            until: window.until,
            limit: PAGE,
            after: None,
        };
        loop {
            let items = self.outbox.replay_window(BRAIN_CONSUMER, &page).await?;
            let Some(last) = items.last() else {
                return Ok(());
            };
            page.after = Some(last.id.clone());
            for item in &items {
                if report.events >= self.max_events || run.counting.exhausted() {
                    report.truncated = true;
                    return Ok(());
                }
                run.one(conn, item, report).await?;
            }
        }
    }
}

struct Run<'a> {
    pipeline: &'a BrainPipeline,
    ctx: &'a BatchContext,
    counting: &'a CountingModel,
}

impl Run<'_> {
    async fn one(
        &self,
        conn: &mut PgConnection,
        item: &ReplayItem,
        report: &mut ReplayReport,
    ) -> BrainResult<()> {
        let Some(event) = decode(item) else {
            report.undecodable += 1;
            return Ok(());
        };
        report.events += 1;
        ReplayRepository::forget_result(&mut *conn, &item.id, self.ctx.version()).await?;
        let mut savepoint = conn.begin().await?;
        match self
            .pipeline
            .process(&item.id, event, &mut savepoint, self.ctx)
            .await
        {
            Ok(_) => savepoint.commit().await?,
            Err(error) => {
                report.failed += 1;
                tracing::debug!(outbox_id = %item.id, error = %error, "Brain replay event failed");
                savepoint.rollback().await?;
            },
        }
        Ok(())
    }
}

fn decode(item: &ReplayItem) -> Option<BrainEvent> {
    if item.fact.version != BRAIN_EVENT_VERSION {
        return None;
    }
    // JSON: the fact data is the versioned brain event.
    serde_json::from_value(item.fact.data.clone()).ok()
}

/// A replay service with the model client its dry runs call, as a
/// `BrainService` holds it.
#[derive(Clone)]
pub struct ProposalReplay {
    pub service: Arc<ReplayService>,
    pub model: Arc<dyn BrainModelClient>,
}

impl std::fmt::Debug for ProposalReplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProposalReplay")
            .field("service", &self.service)
            .finish_non_exhaustive()
    }
}
