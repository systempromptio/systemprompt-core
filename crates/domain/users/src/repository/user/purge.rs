//! Deleting a user's rows from every table that keys on them.
//!
//! The schema cannot cascade a user delete: activity tables carry `user_id`
//! without a foreign key, because the column also holds sentinel principals
//! and the identifiers a session column means differ table to table. Each
//! owning crate therefore declares its tables in the
//! [`systemprompt_extension::purge`] registry and the delete runs them here,
//! inside the same transaction that deletes the `users` row. Content
//! the user's rows referenced but did not own — an artifact body shared by
//! digest — is declared as an orphan sweep and cleared once the user-keyed
//! tables are gone, in the same transaction. The same lists, counted instead
//! of deleted, are the dry-run report: what a deletion would remove, table by
//! table — and, run against a user who no longer exists, an orphan report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{AssertSqlSafe, Postgres, Transaction};
use systemprompt_database::admin::SafeIdentifier;
use systemprompt_extension::purge::{
    OrphanSweep, UserPurgeTable, registered_orphan_sweeps, registered_user_purge_tables,
};
use systemprompt_extension::{orphan_sweeps, user_purge_tables};
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

orphan_sweeps!(
    "systemprompt-core",
    [{
        table: "artifact_payloads",
        key: "sha256",
        referenced_by: "mcp_artifacts",
        via: "payload_sha256",
        user_column: "user_id",
    }]
);

/// How many rows one purge table holds for a user.
#[derive(Debug, Clone, Copy)]
pub struct PurgeCount {
    pub owner: &'static str,
    pub table: &'static str,
    pub rows: i64,
}

fn identifier(kind: &str, raw: &str) -> Result<SafeIdentifier> {
    SafeIdentifier::parse(raw)
        .map_err(|e| UserError::Validation(format!("purge {kind} {raw}: {e}")))
}

// Why: the predicate is a fixed fragment from a crate's registration, never
// user input, but it is interpolated into SQL — so it is held to a character
// set that cannot terminate the statement or open a comment.
fn predicate(entry: &UserPurgeTable) -> Result<String> {
    let Some(raw) = entry.predicate else {
        return Ok(String::new());
    };
    let allowed = |c: char| c.is_ascii_alphanumeric() || " _'<>=!.()".contains(c);
    if raw.is_empty() || raw.contains("''") || !raw.chars().all(allowed) {
        return Err(UserError::Validation(format!(
            "purge predicate for {}: not a plain comparison: {raw}",
            entry.table
        )));
    }
    Ok(format!(" AND ({raw})"))
}

fn purge_where(entry: &UserPurgeTable) -> Result<String> {
    let table = identifier("table", entry.table)?;
    let column = identifier("column", entry.column)?;
    // Why: `$1::text` comparison — one purge table keys on a uuid column and
    // a text bind would not coerce.
    Ok(format!(
        "FROM {} WHERE {}::text = $1{}",
        table.quoted(),
        column.quoted(),
        predicate(entry)?
    ))
}

struct SweepSql {
    delete: String,
    preview: String,
}

fn sweep_sql(sweep: &OrphanSweep) -> Result<SweepSql> {
    let table = identifier("sweep table", sweep.table)?.quoted();
    let key = identifier("sweep key", sweep.key)?.quoted();
    let referenced_by = identifier("sweep referrer", sweep.referenced_by)?.quoted();
    let via = identifier("sweep reference", sweep.via)?.quoted();
    let user = identifier("sweep user column", sweep.user_column)?.quoted();
    let referrer = format!("SELECT 1 FROM {referenced_by} r WHERE r.{via} = t.{key}");
    Ok(SweepSql {
        delete: format!("DELETE FROM {table} t WHERE NOT EXISTS ({referrer})"),
        preview: format!(
            "SELECT COUNT(*) FROM {table} t \
             WHERE EXISTS ({referrer} AND r.{user}::text = $1) \
             AND NOT EXISTS ({referrer} AND r.{user}::text IS DISTINCT FROM $1)"
        ),
    })
}

impl UserRepository {
    pub(super) async fn purge_user_rows(
        tx: &mut Transaction<'_, Postgres>,
        id: &UserId,
    ) -> Result<Vec<PurgeCount>> {
        let mut removed = Vec::new();
        for entry in registered_user_purge_tables() {
            let sql = format!("DELETE {}", purge_where(entry)?);
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
        for sweep in registered_orphan_sweeps() {
            let result = sqlx::query(AssertSqlSafe(sweep_sql(sweep)?.delete))
                .execute(&mut **tx)
                .await?;
            removed.push(PurgeCount {
                owner: sweep.owner,
                table: sweep.table,
                rows: i64::try_from(result.rows_affected()).unwrap_or(i64::MAX),
            });
        }
        Ok(removed)
    }

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
        for entry in registered_user_purge_tables() {
            let sql = format!("SELECT COUNT(*) {}", purge_where(entry)?);
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
        for sweep in registered_orphan_sweeps() {
            let rows: i64 = sqlx::query_scalar(AssertSqlSafe(sweep_sql(sweep)?.preview))
                .bind(id.as_str())
                .fetch_one(&*self.pool)
                .await?;
            counts.push(PurgeCount {
                owner: sweep.owner,
                table: sweep.table,
                rows,
            });
        }
        Ok(counts)
    }
}
