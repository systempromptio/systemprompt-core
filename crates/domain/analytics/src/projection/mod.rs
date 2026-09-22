//! Durable analytics projections over versioned source reporting contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgConnection;

use crate::{AnalyticsError, Result};

mod snapshot;
mod sources;
mod state;
pub use snapshot::{SnapshotPage, lock_sources, write_snapshot_page};
pub use sources::SOURCE_DEFINITIONS;
pub use state::{
    ProjectionStatus, RebuildState, heartbeat_rebuild, is_initialized, lock_projector,
    lock_user_deletion, next_cutoff_revision, rebuild_state, status,
};

pub const REPORTING_CONSUMER: &str = "analytics_reporting";
pub const REPORTING_KIND: &str = "reporting.row";
pub const REPORTING_VERSION: u32 = 1;
pub const REPORTING_STATE_SEED: &str =
    "INSERT INTO analytics_projection_state(singleton) VALUES (TRUE) ON CONFLICT DO NOTHING";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportingSource {
    Users,
    UserSessions,
    AgentTasks,
    TaskMessages,
    UserContexts,
    AiRequests,
    McpToolExecutions,
    MarkdownContent,
    AnalyticsEvents,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReportingRow {
    pub source: ReportingSource,
    pub key: String,
    pub revision: i64,
    pub deleted: bool,
    pub row: Value,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceDefinition {
    pub source: ReportingSource,
    pub table: &'static str,
    pub view: &'static str,
    pub target: &'static str,
    pub key: &'static str,
    pub key_type: &'static str,
    pub columns: &'static [&'static str],
}

impl ReportingSource {
    pub fn definition(self) -> &'static SourceDefinition {
        &SOURCE_DEFINITIONS[match self {
            Self::Users => 0,
            Self::UserSessions => 1,
            Self::AgentTasks => 2,
            Self::TaskMessages => 3,
            Self::UserContexts => 4,
            Self::AiRequests => 5,
            Self::McpToolExecutions => 6,
            Self::MarkdownContent => 7,
            Self::AnalyticsEvents => 8,
        }]
    }
}

impl ReportingRow {
    fn validate(&self) -> Result<()> {
        if self.key.is_empty() || self.revision < 0 {
            return Err(AnalyticsError::invalid_argument(
                "invalid reporting key or revision",
            ));
        }
        if self.deleted {
            if !self.row.is_null() {
                return Err(AnalyticsError::invalid_argument(
                    "deleted reporting fact must have a null row",
                ));
            }
            return Ok(());
        }
        let definition = self.source.definition();
        let object = self
            .row
            .as_object()
            .ok_or_else(|| AnalyticsError::invalid_argument("reporting row must be an object"))?;
        if object.len() != definition.columns.len()
            || definition
                .columns
                .iter()
                .any(|column| !object.contains_key(*column))
        {
            return Err(AnalyticsError::invalid_argument(
                "reporting row does not match its versioned column contract",
            ));
        }
        let key = &object[definition.key];
        let matches = key.as_str().is_some_and(|key| key == self.key)
            || key.as_i64().is_some_and(|key| key.to_string() == self.key);
        if !matches {
            return Err(AnalyticsError::invalid_argument(
                "reporting row key does not match envelope",
            ));
        }
        Ok(())
    }
}

/// Applies reporting facts inside the caller's transaction and projector lock.
#[derive(Debug, Clone, Copy)]
pub struct ReportingProjector;

impl ReportingProjector {
    /// Opens a new generation with its cutoff and the in-progress marker.
    /// Callers hold the source locks: every fact minted before `cutoff`
    /// has committed, every later one is above it and will be applied.
    pub async fn begin_rebuild(connection: &mut PgConnection, cutoff: i64) -> Result<i64> {
        Ok(sqlx::query_scalar!(
            r#"UPDATE analytics_projection_state
               SET generation = generation + 1, initialized = FALSE, cutoff_revision = $1,
                   rebuild_started_at = NOW(), rebuild_heartbeat_at = NOW(),
                   rebuild_source = NULL, rebuild_rows = 0
               WHERE singleton RETURNING generation AS "generation!""#,
            cutoff
        )
        .fetch_one(&mut *connection)
        .await?)
    }

    /// Empties every report table and the revision guard for the generation
    /// opened by `begin_rebuild`; a separate transaction so no source lock is
    /// held while the targets are truncated.
    pub async fn clear_targets(connection: &mut PgConnection, generation: i64) -> Result<()> {
        Self::verify_generation(connection, generation).await?;
        let targets = SOURCE_DEFINITIONS
            .iter()
            .map(|definition| definition.target)
            .collect::<Vec<_>>()
            .join(", ");
        sqlx::query(sqlx::AssertSqlSafe(format!("TRUNCATE TABLE {targets}")))
            .execute(&mut *connection)
            .await?;
        sqlx::query!("DELETE FROM analytics_projection_revisions")
            .execute(&mut *connection)
            .await?;
        Ok(())
    }

    /// Marks the baseline complete. The cutoff is left as recorded by
    /// `begin_rebuild` or raised since by a privacy compaction; lowering it
    /// would replay facts that compaction already delivered.
    pub async fn finish_rebuild(connection: &mut PgConnection, generation: i64) -> Result<()> {
        let result = sqlx::query!(
            "UPDATE analytics_projection_state
             SET initialized = TRUE, rebuilt_at = NOW(), rebuild_started_at = NULL,
                 rebuild_heartbeat_at = NULL, rebuild_source = NULL
             WHERE singleton AND generation = $1 AND NOT initialized",
            generation
        )
        .execute(&mut *connection)
        .await?;
        if result.rows_affected() != 1 {
            return Err(AnalyticsError::rebuild_superseded());
        }
        Ok(())
    }

    async fn verify_generation(connection: &mut PgConnection, generation: i64) -> Result<()> {
        let state = rebuild_state(connection).await?;
        if state.generation != generation || state.initialized || state.rebuild_started_at.is_none()
        {
            return Err(AnalyticsError::rebuild_superseded());
        }
        Ok(())
    }

    pub async fn apply_fact(connection: &mut PgConnection, fact: &ReportingRow) -> Result<bool> {
        fact.validate()?;
        let state = sqlx::query!(
            "SELECT initialized, cutoff_revision FROM analytics_projection_state WHERE singleton FOR UPDATE"
        )
        .fetch_one(&mut *connection)
        .await?;
        let cutoff = state.cutoff_revision;
        if !state.initialized {
            return Err(AnalyticsError::invalid_argument(
                "analytics projection requires a baseline snapshot",
            ));
        }
        if fact.revision <= cutoff {
            return Ok(false);
        }
        let accepted = sqlx::query_scalar!(
            "INSERT INTO analytics_projection_revisions(source, entity_key, revision)
             VALUES ($1, $2, $3)
             ON CONFLICT(source, entity_key) DO UPDATE SET revision = EXCLUDED.revision
             WHERE analytics_projection_revisions.revision < EXCLUDED.revision
             RETURNING revision",
            fact.source.definition().table,
            &fact.key,
            fact.revision
        )
        .fetch_optional(&mut *connection)
        .await?;
        if accepted.is_none() {
            return Ok(false);
        }
        Self::write_row(connection, fact).await?;
        Ok(true)
    }

    async fn retained(connection: &mut PgConnection, fact: &ReportingRow) -> Result<bool> {
        Ok(sqlx::query_scalar!(
            r#"SELECT reporting_row_retained($1, $2) AS "retained!""#,
            fact.source.definition().table,
            &fact.row
        )
        .fetch_one(connection)
        .await?)
    }

    async fn write_row(connection: &mut PgConnection, fact: &ReportingRow) -> Result<()> {
        let definition = fact.source.definition();
        if fact.deleted || !Self::retained(connection, fact).await? {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM {} WHERE {} = CAST($1 AS {})",
                definition.target, definition.key, definition.key_type,
            )))
            .bind(&fact.key)
            .execute(connection)
            .await?;
        } else {
            let assignments = definition
                .columns
                .iter()
                .filter(|column| **column != definition.key)
                .map(|column| format!("{column} = EXCLUDED.{column}"))
                .collect::<Vec<_>>()
                .join(", ");
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {} SELECT * FROM jsonb_populate_record(NULL::{}, $1)
                 ON CONFLICT ({}) DO UPDATE SET {}",
                definition.target, definition.target, definition.key, assignments,
            )))
            .bind(&fact.row)
            .execute(connection)
            .await?;
        }
        Ok(())
    }
}
