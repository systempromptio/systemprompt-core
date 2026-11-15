//! Deleting a user's rows from every table that keys on them.
//!
//! The schema cannot cascade a user delete: activity tables carry `user_id`
//! without a foreign key, because the column also holds sentinel principals
//! and the identifiers a session column means differ table to table. Each
//! owning crate therefore declares its tables in the
//! `contribute` (`Contributions::user_purge`) and the delete runs the admitted
//! set ([`AdmittedPurge`]) here,
//! inside the same transaction that deletes the `users` row. Content
//! the user's rows referenced but did not own — an artifact body shared by
//! digest — is declared as an orphan sweep: the keys the user's rows reference
//! are captured first, and once the user-keyed tables are gone only those keys
//! that nothing references any more are cleared, in the same transaction. The
//! same lists, counted instead of deleted, are the dry-run report: what a
//! deletion would remove, table by table — and, run against a user who no
//! longer exists, an orphan report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{AssertSqlSafe, Postgres, Transaction};
use systemprompt_database::admin::SafeIdentifier;
use systemprompt_extension::{AdmittedPurge, OrphanSweep, UserPurgeTable};
use systemprompt_identifiers::{ExtensionId, UserId};

use crate::error::{Result, UserError};
use crate::repository::UserRepository;

pub(crate) const USERS_OWNER: &str = "users";

pub(crate) const CORE_PURGE_TABLES: &[UserPurgeTable] = &[
    purge_table("ai_requests"),
    purge_table("mcp_artifacts"),
    purge_table("mcp_tool_executions"),
    purge_table("mcp_sessions"),
    purge_table("mcp_external_sessions"),
    purge_table("governance_decisions"),
    purge_table("agent_tasks"),
    purge_table("task_messages"),
    purge_table("files"),
    purge_table("event_outbox"),
    purge_table("logs"),
    purge_table("user_rate_limit_buckets"),
    purge_table("oauth_jti_revocations"),
    purge_table("sso_login_events"),
];

pub(crate) const CORE_ORPHAN_SWEEPS: &[OrphanSweep] = &[OrphanSweep {
    table: "artifact_payloads",
    key: "sha256",
    referenced_by: "mcp_artifacts",
    via: "payload_sha256",
    user_column: "user_id",
}];

const fn purge_table(table: &'static str) -> UserPurgeTable {
    UserPurgeTable {
        table,
        column: "user_id",
        predicate: None,
    }
}

/// How many rows one purge table holds for a user.
#[derive(Debug, Clone)]
pub struct PurgeCount {
    pub owner: ExtensionId,
    pub table: &'static str,
    pub rows: i64,
}

fn identifier(kind: &'static str, raw: &str) -> Result<SafeIdentifier> {
    SafeIdentifier::parse(raw).map_err(|source| UserError::PurgeIdentifier {
        kind,
        name: raw.to_owned(),
        source,
    })
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
    // Why: `::text` comparison — purge tables key on TEXT and VARCHAR
    // columns, and the cast keeps one text bind valid for every one of them.
    Ok(format!(
        "FROM {} WHERE {}::text = $1{}",
        table.quoted(),
        column.quoted(),
        predicate(entry)?
    ))
}

struct SweepSql {
    candidates: String,
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
        candidates: format!(
            "SELECT DISTINCT r.{via}::text FROM {referenced_by} r \
             WHERE r.{user}::text = $1 AND r.{via} IS NOT NULL"
        ),
        delete: format!(
            "DELETE FROM {table} t WHERE t.{key}::text = ANY($1) AND NOT EXISTS ({referrer})"
        ),
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
        plan: &AdmittedPurge,
    ) -> Result<Vec<PurgeCount>> {
        let mut sweeps = Vec::new();
        for (owner, sweep) in plan.sweeps() {
            let sql = sweep_sql(sweep)?;
            let keys: Vec<String> = sqlx::query_scalar(AssertSqlSafe(sql.candidates))
                .bind(id.as_str())
                .fetch_all(&mut **tx)
                .await?;
            sweeps.push((owner, sweep, sql.delete, keys));
        }
        let mut removed = Vec::new();
        for (owner, entry) in plan.tables() {
            let sql = format!("DELETE {}", purge_where(entry)?);
            let result = sqlx::query(AssertSqlSafe(sql))
                .bind(id.as_str())
                .execute(&mut **tx)
                .await?;
            removed.push(PurgeCount {
                owner: owner.clone(),
                table: entry.table,
                rows: i64::try_from(result.rows_affected()).unwrap_or(i64::MAX),
            });
        }
        for (owner, sweep, delete, keys) in sweeps {
            let result = sqlx::query(AssertSqlSafe(delete))
                .bind(keys)
                .execute(&mut **tx)
                .await?;
            removed.push(PurgeCount {
                owner: owner.clone(),
                table: sweep.table,
                rows: i64::try_from(result.rows_affected()).unwrap_or(i64::MAX),
            });
        }
        Ok(removed)
    }

    pub async fn purge_preview(
        &self,
        id: &UserId,
        plan: &AdmittedPurge,
    ) -> Result<Vec<PurgeCount>> {
        let mut counts = Vec::new();
        let sessions: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM user_sessions WHERE user_id = $1"#,
            id.as_str()
        )
        .fetch_one(&*self.pool)
        .await?;
        counts.push(PurgeCount {
            owner: ExtensionId::new(USERS_OWNER),
            table: "user_sessions",
            rows: sessions,
        });
        for (owner, entry) in plan.tables() {
            let sql = format!("SELECT COUNT(*) {}", purge_where(entry)?);
            let rows: i64 = sqlx::query_scalar(AssertSqlSafe(sql))
                .bind(id.as_str())
                .fetch_one(&*self.pool)
                .await?;
            counts.push(PurgeCount {
                owner: owner.clone(),
                table: entry.table,
                rows,
            });
        }
        for (owner, sweep) in plan.sweeps() {
            let rows: i64 = sqlx::query_scalar(AssertSqlSafe(sweep_sql(sweep)?.preview))
                .bind(id.as_str())
                .fetch_one(&*self.pool)
                .await?;
            counts.push(PurgeCount {
                owner: owner.clone(),
                table: sweep.table,
                rows,
            });
        }
        Ok(counts)
    }
}
