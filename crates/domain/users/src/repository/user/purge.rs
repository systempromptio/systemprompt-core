//! Deleting a user's rows from every table that keys on them.
//!
//! The schema cannot cascade a user delete: activity tables carry `user_id`
//! without a foreign key, because the column also holds sentinel principals
//! and the identifiers a session column means differ table to table. Each
//! owning crate therefore declares its tables in the
//! [`systemprompt_extension::purge`] registry and the delete runs them here,
//! inside the privacy transaction that already guards `users` itself. The
//! same list, counted instead of deleted, is the dry-run report: what a
//! deletion would remove, table by table — and, run against a user who no
//! longer exists, an orphan report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{AssertSqlSafe, Postgres, Transaction};
use systemprompt_database::admin::SafeIdentifier;
use systemprompt_extension::purge::{UserPurgeTable, registered_user_purge_tables as registered};
use systemprompt_extension::user_purge_tables;
use systemprompt_identifiers::UserId;

use crate::error::{Result, UserError};
use crate::repository::UserRepository;

// Why: core's own user-keyed tables without a foreign key to `users`, one
// entry per owning crate's table. `ai_requests` cascades to its children;
// `user_sessions` is deleted by the caller first because `ai_requests` and
// `user_contexts` point at it with SET NULL and must go before the session.
user_purge_tables!(
    "systemprompt-core",
    [
        ("ai_requests", "user_id"),
        ("mcp_artifacts", "user_id"),
        ("mcp_tool_executions", "user_id"),
        ("mcp_sessions", "user_id"),
        ("mcp_external_sessions", "user_id"),
        ("mcp_proxy_identities", "user_id"),
        ("governance_decisions", "user_id"),
        ("agent_tasks", "user_id"),
        ("task_messages", "user_id"),
        ("files", "user_id"),
        ("event_outbox", "user_id"),
        ("logs", "user_id"),
        ("user_rate_limit_buckets", "user_id"),
        ("oauth_jti_revocations", "user_id"),
    ]
);

/// How many rows one purge table holds for a user.
#[derive(Debug, Clone, Copy)]
pub struct PurgeCount {
    pub owner: &'static str,
    pub table: &'static str,
    pub rows: i64,
}

fn validated(entry: &UserPurgeTable) -> Result<(SafeIdentifier, SafeIdentifier)> {
    let table = SafeIdentifier::parse(entry.table)
        .map_err(|e| UserError::Validation(format!("purge table {}: {e}", entry.table)))?;
    let column = SafeIdentifier::parse(entry.column)
        .map_err(|e| UserError::Validation(format!("purge column {}: {e}", entry.column)))?;
    Ok((table, column))
}

impl UserRepository {
    /// Deletes the user's rows from every registered purge table, in
    /// registration order, on the given transaction.
    pub(super) async fn purge_user_rows(
        tx: &mut Transaction<'_, Postgres>,
        id: &UserId,
    ) -> Result<Vec<PurgeCount>> {
        let mut removed = Vec::new();
        for entry in registered() {
            let (table, column) = validated(entry)?;
            // Why: `$1::text` comparison — one purge table keys on a uuid
            // column and a text bind would not coerce.
            let sql = format!(
                "DELETE FROM {} WHERE {}::text = $1",
                table.quoted(),
                column.quoted()
            );
            let result = sqlx::query(AssertSqlSafe(sql))
                .bind(id.as_str())
                .execute(&mut **tx)
                .await?;
            removed.push(PurgeCount {
                owner: entry.owner,
                table: entry.table,
                rows: i64::try_from(result.rows_affected()).unwrap_or(i64::MAX),
            });
        }
        Ok(removed)
    }

    /// What deleting the user would remove, per registered table, without
    /// removing anything. `user_sessions` is included because the delete
    /// clears it too.
    pub async fn purge_preview(&self, id: &UserId) -> Result<Vec<PurgeCount>> {
        let mut counts = Vec::new();
        let sessions: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM user_sessions WHERE user_id = $1"#,
            id.as_str()
        )
        .fetch_one(&*self.pool)
        .await?;
        counts.push(PurgeCount {
            owner: "systemprompt-core",
            table: "user_sessions",
            rows: sessions,
        });
        for entry in registered() {
            let (table, column) = validated(entry)?;
            let sql = format!(
                "SELECT COUNT(*) FROM {} WHERE {}::text = $1",
                table.quoted(),
                column.quoted()
            );
            let rows: i64 = sqlx::query_scalar(AssertSqlSafe(sql))
                .bind(id.as_str())
                .fetch_one(&*self.pool)
                .await?;
            counts.push(PurgeCount {
                owner: entry.owner,
                table: entry.table,
                rows,
            });
        }
        Ok(counts)
    }
}
