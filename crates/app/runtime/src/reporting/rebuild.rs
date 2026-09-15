//! Transactional source capture installation and analytics baseline rebuild.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_analytics::AnalyticsError;
use systemprompt_analytics::projection::{ReportingProjector, ReportingRow, SOURCE_DEFINITIONS};
use systemprompt_database::DbPool;

use crate::RuntimeResult;

pub async fn initialize(db: &DbPool) -> RuntimeResult<()> {
    configure(db, false).await
}

pub async fn rebuild(db: &DbPool) -> RuntimeResult<()> {
    configure(db, true).await
}

async fn configure(db: &DbPool, force_rebuild: bool) -> RuntimeResult<()> {
    let pool = db.write_pool_arc()?;
    let mut transaction = pool.begin().await.map_err(AnalyticsError::from)?;
    sqlx::query("SELECT public.lock_user_deletion_for_retention()")
        .execute(&mut *transaction)
        .await
        .map_err(AnalyticsError::from)?;
    lock_sources(&mut transaction).await?;
    super::lock(&mut transaction).await?;
    install_capture(&mut transaction).await?;
    let initialized: bool =
        sqlx::query_scalar("SELECT initialized FROM analytics_projection_state WHERE singleton")
            .fetch_one(&mut *transaction)
            .await
            .map_err(AnalyticsError::from)?;
    if force_rebuild || !initialized {
        rebuild_locked(&mut transaction).await?;
    }
    transaction.commit().await.map_err(AnalyticsError::from)?;
    Ok(())
}

async fn install_capture(connection: &mut PgConnection) -> Result<(), AnalyticsError> {
    for script in [
        systemprompt_events::REPORTING_CAPTURE_SQL,
        systemprompt_users::REPORTING_CAPTURE_SQL,
        systemprompt_agent::REPORTING_CAPTURE_SQL,
        systemprompt_ai::REPORTING_CAPTURE_SQL,
        systemprompt_mcp::REPORTING_CAPTURE_SQL,
        systemprompt_content::REPORTING_CAPTURE_SQL,
        systemprompt_logging::REPORTING_CAPTURE_SQL,
    ] {
        sqlx::raw_sql(script).execute(&mut *connection).await?;
    }
    Ok(())
}

async fn lock_sources(connection: &mut PgConnection) -> Result<(), AnalyticsError> {
    let tables = SOURCE_DEFINITIONS
        .iter()
        .map(|definition| definition.table)
        .collect::<Vec<_>>()
        .join(", ");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "LOCK TABLE {tables} IN SHARE MODE"
    )))
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn rebuild_locked(connection: &mut PgConnection) -> Result<(), AnalyticsError> {
    let cutoff: i64 = sqlx::query_scalar("SELECT nextval('event_outbox_reporting_revision')")
        .fetch_one(&mut *connection)
        .await?;
    let generation = ReportingProjector::begin_rebuild(connection).await?;
    for definition in SOURCE_DEFINITIONS {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DECLARE reporting_snapshot NO SCROLL CURSOR FOR SELECT entity_key, row FROM {}",
            definition.view,
        )))
        .execute(&mut *connection)
        .await?;
        loop {
            let rows: Vec<SnapshotRow> =
                sqlx::query_as("FETCH FORWARD 1000 FROM reporting_snapshot")
                    .fetch_all(&mut *connection)
                    .await?;
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
        sqlx::query("CLOSE reporting_snapshot")
            .execute(&mut *connection)
            .await?;
    }
    ReportingProjector::finish_rebuild(connection, generation, cutoff).await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct SnapshotRow {
    entity_key: String,
    // JSON: owner-published reporting views provide the versioned row payload.
    row: serde_json::Value,
}
