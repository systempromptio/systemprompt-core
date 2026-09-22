//! Analytics baseline rebuild in fenced, committed phases: a millisecond
//! cutoff fence under the source locks, a truncate, one set-based page per
//! transaction per source, then the flip to `initialized`. Nothing here
//! holds a source lock or an open transaction for longer than one page.
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
/// A rebuild whose heartbeat is older than this is presumed dead and taken
/// over; a page is a bounded statement, so a live one heartbeats well inside.
const STALE_HEARTBEAT: chrono::Duration = chrono::Duration::seconds(120);

/// Why a non-forced run returned without a fresh baseline being its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildOutcome {
    /// This run built the baseline.
    Rebuilt,
    /// The baseline already existed.
    AlreadyInitialized,
    /// Another node is mid-rebuild and heartbeating; try again later.
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

/// Builds the baseline when there is none. Synchronous: returns once the
/// projection is initialized or another node is known to be building it.
pub async fn initialize(db: &DbPool) -> RuntimeResult<RebuildOutcome> {
    run(db, Mode::IfNeeded).await
}

/// Rebuilds the baseline unconditionally, fencing any rebuild in flight.
pub async fn rebuild(db: &DbPool) -> RuntimeResult<()> {
    run(db, Mode::Force).await?;
    Ok(())
}

async fn run(db: &DbPool, mode: Mode) -> RuntimeResult<RebuildOutcome> {
    let pool = db.write_pool_arc()?;
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
                tracing::warn!(
                    generation = plan.generation,
                    "Analytics baseline rebuild superseded; starting over"
                );
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

fn retry_config() -> RetryConfig {
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

/// Phase A. Under every source lock, so no writer is mid-flight: mint the
/// cutoff and open the generation. Returns `None` when there is nothing to do.
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

/// Phase A2: empty the targets in their own transaction.
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

/// Phase B for one source: keyset pages, each its own transaction and
/// heartbeat, until a page comes back short.
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

/// Phase C: flip `initialized` if this generation is still the live one.
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
