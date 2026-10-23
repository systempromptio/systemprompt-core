//! Event inserts into and reads from `agent_run_events`.
//!
//! A batch is one `UNNEST` insert; a replayed `seq` is ignored, and an empty
//! tool name or id is stored as `NULL`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::AgentRunId;

use super::AgentRunRepository;
use crate::error::AgentRunsResult;
use crate::event::{RunEvent, RunEventKind, RunEventPayload};

impl AgentRunRepository {
    pub async fn insert_events_batch(
        &self,
        run_id: &AgentRunId,
        events: &[RunEvent],
    ) -> AgentRunsResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        let seqs: Vec<i64> = events.iter().map(|e| e.seq).collect();
        let kinds: Vec<String> = events.iter().map(|e| e.kind.as_str().to_owned()).collect();
        let tool_names: Vec<String> = events
            .iter()
            .map(|e| e.tool_name.clone().unwrap_or_default())
            .collect();
        let tool_use_ids: Vec<String> = events
            .iter()
            .map(|e| e.tool_use_id.clone().unwrap_or_default())
            .collect();
        let payloads = events
            .iter()
            .map(|e| serde_json::to_value(&e.payload))
            .collect::<Result<Vec<_>, _>>()?;
        let created: Vec<DateTime<Utc>> = events.iter().map(|e| e.created_at).collect();
        let mut tx = self.write_pool().begin().await?;
        sqlx::query!(
            r#"
            INSERT INTO agent_run_events
                (run_id, seq, kind, tool_name, tool_use_id, payload, created_at)
            SELECT $1, u.seq, u.kind, NULLIF(u.tool_name, ''), NULLIF(u.tool_use_id, ''),
                   u.payload, u.created_at
            FROM UNNEST(
                $2::bigint[], $3::text[], $4::text[], $5::text[], $6::jsonb[], $7::timestamptz[]
            ) AS u(seq, kind, tool_name, tool_use_id, payload, created_at)
            ON CONFLICT (run_id, seq) DO NOTHING
            "#,
            run_id.as_str(),
            &seqs,
            &kinds,
            &tool_names,
            &tool_use_ids,
            &payloads,
            &created,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_scalar!(
            r#"SELECT 1 AS "notified!" FROM pg_notify('agent_run_events', $1)"#,
            run_id.as_str(),
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_events_after(
        &self,
        run_id: &AgentRunId,
        after_seq: i64,
        limit: i64,
    ) -> AgentRunsResult<Vec<RunEvent>> {
        let rows = sqlx::query!(
            r#"
            SELECT seq, kind, tool_name, tool_use_id, payload, created_at
            FROM agent_run_events
            WHERE run_id = $1 AND seq > $2
            ORDER BY seq
            LIMIT $3
            "#,
            run_id.as_str(),
            after_seq,
            limit,
        )
        .fetch_all(self.write_pool())
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(RunEvent {
                    seq: row.seq,
                    kind: row.kind.parse::<RunEventKind>()?,
                    tool_name: row.tool_name,
                    tool_use_id: row.tool_use_id,
                    payload: serde_json::from_value::<RunEventPayload>(row.payload)?,
                    created_at: row.created_at,
                })
            })
            .collect()
    }

    pub async fn count_events(&self, run_id: &AgentRunId) -> AgentRunsResult<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM agent_run_events WHERE run_id = $1"#,
            run_id.as_str(),
        )
        .fetch_one(self.write_pool())
        .await?;
        Ok(count)
    }
}
