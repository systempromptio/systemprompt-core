//! Run directories past the retention window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::AgentRunId;

use super::AgentRunRepository;
use crate::error::AgentRunsResult;

/// A finished run whose directory is still on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiredRunDir {
    pub id: AgentRunId,
    pub run_dir: String,
}

impl AgentRunRepository {
    pub async fn list_expired_run_dirs(
        &self,
        finished_before: DateTime<Utc>,
        limit: i64,
    ) -> AgentRunsResult<Vec<ExpiredRunDir>> {
        let rows = sqlx::query!(
            r#"
            SELECT id AS "id: AgentRunId", run_dir AS "run_dir!"
            FROM agent_runs
            WHERE finished_at < $1 AND run_dir IS NOT NULL
            ORDER BY finished_at
            LIMIT $2
            "#,
            finished_before,
            limit,
        )
        .fetch_all(self.write_pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ExpiredRunDir {
                id: row.id,
                run_dir: row.run_dir,
            })
            .collect())
    }

    pub async fn clear_run_dir(&self, id: &AgentRunId) -> AgentRunsResult<()> {
        sqlx::query!(
            "UPDATE agent_runs SET run_dir = NULL WHERE id = $1",
            id.as_str()
        )
        .execute(self.write_pool())
        .await?;
        Ok(())
    }
}
