//! Transactional source capture installation and analytics baseline rebuild.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use sqlx::{PgConnection, PgPool};
use systemprompt_analytics::AnalyticsError;
use systemprompt_analytics::projection::{
    self, ReportingProjector, ReportingRow, SOURCE_DEFINITIONS, SnapshotCursor,
};
use systemprompt_database::DbPool;
use systemprompt_database::resilience::{Outcome, RetryConfig, retry_async};

use crate::RuntimeResult;

pub async fn initialize(db: &DbPool) -> RuntimeResult<()> {
    configure(db, false).await
}

pub async fn rebuild(db: &DbPool) -> RuntimeResult<()> {
    configure(db, true).await
}

async fn configure(db: &DbPool, force_rebuild: bool) -> RuntimeResult<()> {
    let pool = db.write_pool_arc()?;
    let retry = RetryConfig {
        max_attempts: 4,
        base_delay: Duration::from_millis(50),
        max_delay: Duration::from_millis(800),
        jitter: true,
    };
    let classify = |error: &AnalyticsError| match error {
        AnalyticsError::Repository(repository) if repository.is_serialization_failure() => {
            Outcome::Transient { retry_after: None }
        },
        _ => Outcome::Permanent,
    };
    retry_async(&retry, "reporting-rebuild", classify, || {
        configure_once(&pool, force_rebuild)
    })
    .await?;
    Ok(())
}

async fn configure_once(pool: &Arc<PgPool>, force_rebuild: bool) -> Result<(), AnalyticsError> {
    let mut transaction = pool.begin().await.map_err(AnalyticsError::from)?;
    projection::lock_user_deletion(&mut transaction).await?;
    SnapshotCursor::lock_sources(&mut transaction).await?;
    projection::lock_projector(&mut transaction).await?;
    if force_rebuild || !projection::is_initialized(&mut transaction).await? {
        rebuild_locked(&mut transaction).await?;
    }
    transaction.commit().await.map_err(AnalyticsError::from)?;
    Ok(())
}

async fn rebuild_locked(connection: &mut PgConnection) -> Result<(), AnalyticsError> {
    let cutoff = projection::next_cutoff_revision(&mut *connection).await?;
    let generation = ReportingProjector::begin_rebuild(connection).await?;
    for definition in SOURCE_DEFINITIONS {
        let cursor = SnapshotCursor::open(&mut *connection, definition).await?;
        loop {
            let rows = cursor.fetch(&mut *connection).await?;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                ReportingProjector::apply_snapshot(
                    connection,
                    &ReportingRow {
                        source: definition.source,
                        key: row.entity_key,
                        revision: cutoff,
                        deleted: false,
                        row: row.row,
                    },
                )
                .await?;
            }
        }
        cursor.close(&mut *connection).await?;
    }
    ReportingProjector::finish_rebuild(connection, generation, cutoff).await?;
    Ok(())
}
