//! Analytics baseline rebuild in fenced, committed phases. Phase A mints the
//! millisecond cutoff and opens the generation under every source lock, so no
//! writer is mid-flight; phase A2 empties the targets in its own transaction;
//! phase B snapshots each source as keyset pages, one transaction and one
//! heartbeat per page, until a page comes back short; phase C flips
//! `initialized` if this generation is still the live one. Nothing here holds
//! a source lock or an open transaction for longer than one page.
//!
//! `initialize` is synchronous: it returns once the projection is initialized
//! or another node is known to be building it. `rebuild` is unconditional and
//! fences any rebuild in flight. A rebuild whose heartbeat is older than
//! `STALE_HEARTBEAT` is presumed dead and taken over; a page is a bounded
//! statement, so a live rebuild heartbeats well inside that window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use sqlx::PgPool;
use systemprompt_analytics::AnalyticsError;
use systemprompt_analytics::projection::{
    self, ReportingProjector, SOURCE_DEFINITIONS, SourceDefinition,
};
use systemprompt_database::DbPool;
use systemprompt_database::resilience::{Outcome, RetryConfig, retry_async};

use crate::RuntimeResult;

const PAGE_ROWS: i64 = 10_000;
const STALE_HEARTBEAT: chrono::Duration = chrono::Duration::seconds(120);

// Why: each fence opens a generation that supersedes any in flight, so two
// forced rebuilds racing will supersede each other for as long as both keep
// retrying. Bounding the retries turns an unbounded livelock into a typed
// outcome the caller can act on; the backoff gives the winning run room to
// finish rather than being superseded again immediately.
const MAX_SUPERSEDED_RETRIES: u32 = 3;
const SUPERSEDED_BACKOFF: Duration = Duration::from_millis(250);

/// What a rebuild run actually did, which is not always what was asked for.
///
/// `InProgressElsewhere` is the one to handle: a forced rebuild is exclusive,
/// because every fence mints a generation superseding whatever is in flight,
/// so concurrent forced rebuilds cannot all win and the losers rebuilt
/// nothing. A caller that needs a baseline containing rows it has just
/// written must either act on this outcome or serialise its rebuilds; it
/// cannot get that guarantee by retrying, which is what made an earlier
/// unbounded retry a livelock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildOutcome {
    Rebuilt,
    AlreadyInitialized,
    InProgressElsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    IfNeeded,
    Force,
}

#[derive(Debug, Clone, Copy)]
struct Plan {
    generation: i64,
}

pub async fn initialize(db: &DbPool) -> RuntimeResult<RebuildOutcome> {
    run(db, Mode::IfNeeded).await
}

pub async fn rebuild(db: &DbPool) -> RuntimeResult<RebuildOutcome> {
    run(db, Mode::Force).await
}

async fn run(db: &DbPool, mode: Mode) -> RuntimeResult<RebuildOutcome> {
    let pool = db.write_pool_arc()?;
    let mut superseded = 0u32;
    loop {
        let Some(plan) = fence(&pool, mode).await? else {
            return Ok(if in_progress_elsewhere(&pool).await? {
                RebuildOutcome::InProgressElsewhere
            } else {
                RebuildOutcome::AlreadyInitialized
            });
        };
        let started = Instant::now();
        match build(&pool, plan).await {
            Ok(()) => {
                tracing::info!(
                    generation = plan.generation,
                    elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    "Analytics baseline rebuilt"
                );
                return Ok(RebuildOutcome::Rebuilt);
            },
            Err(error) if error.is_rebuild_superseded() => {
                superseded += 1;
                if superseded > MAX_SUPERSEDED_RETRIES {
                    tracing::warn!(
                        generation = plan.generation,
                        attempts = superseded,
                        "Analytics baseline rebuild superseded on every attempt; \
                         another rebuild owns the generation"
                    );
                    return Ok(RebuildOutcome::InProgressElsewhere);
                }
                tracing::warn!(
                    generation = plan.generation,
                    attempt = superseded,
                    "Analytics baseline rebuild superseded; starting over"
                );
                tokio::time::sleep(SUPERSEDED_BACKOFF * superseded).await;
            },
            Err(error) => return Err(error.into()),
        }
    }
}

async fn build(pool: &Arc<PgPool>, plan: Plan) -> Result<(), AnalyticsError> {
    clear(pool, plan).await?;
    for definition in SOURCE_DEFINITIONS {
        snapshot_source(pool, plan, definition).await?;
    }
    finish(pool, plan).await
}

const fn retry_config() -> RetryConfig {
    RetryConfig {
        max_attempts: 4,
        base_delay: Duration::from_millis(50),
        max_delay: Duration::from_millis(800),
        jitter: true,
    }
}

fn classify(error: &AnalyticsError) -> Outcome {
    match error {
        AnalyticsError::Repository(repository) if repository.is_serialization_failure() => {
            Outcome::Transient { retry_after: None }
        },
        _ => Outcome::Permanent,
    }
}

async fn fence(pool: &Arc<PgPool>, mode: Mode) -> Result<Option<Plan>, AnalyticsError> {
    retry_async(
        &retry_config(),
        "reporting-rebuild-fence",
        classify,
        || async {
            let mut tx = pool.begin().await?;
            projection::lock_user_deletion(&mut tx).await?;
            projection::lock_sources(&mut tx).await?;
            projection::lock_projector(&mut tx).await?;
            let state = projection::rebuild_state(&mut tx).await?;
            if mode == Mode::IfNeeded && (state.initialized || heartbeat_is_fresh(&state)) {
                tx.commit().await?;
                return Ok(None);
            }
            let cutoff = projection::next_cutoff_revision(&mut tx).await?;
            let generation = ReportingProjector::begin_rebuild(&mut tx, cutoff).await?;
            tx.commit().await?;
            tracing::info!(generation, cutoff, "Analytics baseline rebuild started");
            Ok(Some(Plan { generation }))
        },
    )
    .await
}

fn heartbeat_is_fresh(state: &projection::RebuildState) -> bool {
    state
        .rebuild_heartbeat_at
        .is_some_and(|beat| Utc::now() - beat < STALE_HEARTBEAT)
}

async fn in_progress_elsewhere(pool: &Arc<PgPool>) -> Result<bool, AnalyticsError> {
    let mut connection = pool.acquire().await?;
    let state = projection::rebuild_state(&mut connection).await?;
    Ok(!state.initialized && heartbeat_is_fresh(&state))
}

async fn clear(pool: &Arc<PgPool>, plan: Plan) -> Result<(), AnalyticsError> {
    retry_async(
        &retry_config(),
        "reporting-rebuild-clear",
        classify,
        || async {
            let mut tx = pool.begin().await?;
            projection::lock_projector(&mut tx).await?;
            ReportingProjector::clear_targets(&mut tx, plan.generation).await?;
            tx.commit().await?;
            Ok(())
        },
    )
    .await
}

async fn snapshot_source(
    pool: &Arc<PgPool>,
    plan: Plan,
    definition: &SourceDefinition,
) -> Result<(), AnalyticsError> {
    let started = Instant::now();
    let mut after: Option<String> = None;
    let mut written = 0;
    loop {
        let page = retry_async(
            &retry_config(),
            "reporting-rebuild-page",
            classify,
            || async {
                let mut tx = pool.begin().await?;
                projection::lock_projector(&mut tx).await?;
                let page = projection::write_snapshot_page(
                    &mut tx,
                    definition,
                    after.as_deref(),
                    PAGE_ROWS,
                )
                .await?;
                projection::heartbeat_rebuild(
                    &mut tx,
                    plan.generation,
                    definition.table,
                    page.written,
                )
                .await?;
                tx.commit().await?;
                Ok(page)
            },
        )
        .await?;
        written += page.written;
        if page.fetched < PAGE_ROWS {
            break;
        }
        after = page.last_key;
    }
    tracing::info!(
        source = definition.table,
        rows = written,
        elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "Analytics baseline source snapshotted"
    );
    Ok(())
}

async fn finish(pool: &Arc<PgPool>, plan: Plan) -> Result<(), AnalyticsError> {
    retry_async(
        &retry_config(),
        "reporting-rebuild-finish",
        classify,
        || async {
            let mut tx = pool.begin().await?;
            projection::lock_projector(&mut tx).await?;
            ReportingProjector::finish_rebuild(&mut tx, plan.generation).await?;
            tx.commit().await?;
            Ok(())
        },
    )
    .await
}
