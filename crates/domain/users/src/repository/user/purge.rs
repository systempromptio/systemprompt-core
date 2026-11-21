//! Clearing a user's rows from the tables that cannot cascade from `users`.
//!
//! Every member owning a user-keyed table declares it as a `UserPurgeTable` in
//! `contribute` (`Contributions::user_purge`). The delete runs the admitted
//! set ([`AdmittedPurge`]) and the recorded set of every member absent from
//! this composition (`schema_purge_records`, written by the schema installer)
//! here, inside the deleting transaction; shared bodies its rows referenced
//! that nothing references any more are cleared in the same transaction. A
//! recorded table that no longer exists is skipped and reported, and a member
//! absent from this composition that owns tables but has no record is named
//! in the outcome: the purge cannot vouch for its rows. The same plan,
//! counted instead of deleted, is the dry run.
//!
//! The same file runs an account merge's owner reassignments
//! ([`AdmittedOwnerReassignments`]): each admitted declaration is rendered
//! into its statement (`purge_plan::reassign_sql`) and run inside the merging
//! transaction, in composition order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::types::Json;
use sqlx::{AssertSqlSafe, PgConnection, Postgres, Transaction};
use systemprompt_extension::{AdmittedOwnerReassignments, AdmittedPurge, PurgeDeclaration};
use systemprompt_identifiers::{ExtensionId, UserId};
use tracing::warn;

pub(crate) use super::purge_plan::USERS_OWNER;
pub use super::purge_plan::{PurgeCount, PurgeOutcome, SkippedPurge};
use super::purge_plan::{
    PurgeStep, RecordedPurge, plan_steps, reassign_sql, sweep_sql, table_sql,
};
use crate::error::{Result, UserError};
use crate::repository::UserRepository;

const RECORDS_PRESENT_SQL: &str = "SELECT to_regclass('schema_purge_records') IS NOT NULL";

const OWNERS_PRESENT_SQL: &str = "SELECT to_regclass('schema_table_owners') IS NOT NULL";

const RECORDS_SQL: &str = "SELECT extension_id, declarations FROM schema_purge_records \
     ORDER BY position, extension_id";

const UNRECORDED_SQL: &str = "SELECT DISTINCT o.extension_id FROM schema_table_owners o \
     WHERE NOT (o.extension_id = ANY($1)) \
     AND NOT EXISTS (SELECT 1 FROM schema_purge_records r WHERE r.extension_id = o.extension_id) \
     ORDER BY o.extension_id";

const UNRECORDED_WITHOUT_RECORDS_SQL: &str = "SELECT DISTINCT extension_id \
     FROM schema_table_owners WHERE NOT (extension_id = ANY($1)) ORDER BY extension_id";

const TABLE_PRESENT_SQL: &str = "SELECT to_regclass($1) IS NOT NULL";

struct Plan {
    steps: Vec<PurgeStep>,
    outcome: PurgeOutcome,
}

async fn present(conn: &mut PgConnection, sql: &'static str) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, Option<bool>>(sql)
        .fetch_one(&mut *conn)
        .await?
        .unwrap_or(false))
}

async fn load_plan(conn: &mut PgConnection, plan: &AdmittedPurge) -> Result<Plan> {
    let records_present = present(conn, RECORDS_PRESENT_SQL).await?;
    let recorded = if records_present {
        sqlx::query_as::<_, (String, Json<Vec<PurgeDeclaration>>)>(RECORDS_SQL)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(|(owner, Json(declarations))| RecordedPurge {
                owner: ExtensionId::new(owner),
                declarations,
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut outcome = PurgeOutcome::default();
    if present(conn, OWNERS_PRESENT_SQL).await? {
        let admitted: Vec<String> = plan.members().iter().map(ToString::to_string).collect();
        let sql = if records_present {
            UNRECORDED_SQL
        } else {
            UNRECORDED_WITHOUT_RECORDS_SQL
        };
        outcome.unrecorded_members = sqlx::query_scalar::<_, String>(sql)
            .bind(admitted)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(ExtensionId::new)
            .collect();
    }
    for member in &outcome.unrecorded_members {
        warn!(
            member = %member,
            "user purge: a member absent from this composition owns tables but has no recorded \
             purge declarations; its user rows are not covered"
        );
    }
    let mut steps = Vec::new();
    for step in plan_steps(plan, recorded) {
        if step.absent
            && let Some(missing) = missing_table(conn, &step.declaration).await?
        {
            warn!(
                member = %step.owner,
                table = %missing,
                "user purge: a recorded purge table of an absent member no longer exists; skipped"
            );
            outcome.skipped.push(SkippedPurge {
                owner: step.owner.clone(),
                table: missing,
            });
            continue;
        }
        steps.push(step);
    }
    Ok(Plan { steps, outcome })
}

async fn missing_table(
    conn: &mut PgConnection,
    declaration: &PurgeDeclaration,
) -> Result<Option<String>> {
    for table in declaration.tables() {
        let exists = sqlx::query_scalar::<_, Option<bool>>(TABLE_PRESENT_SQL)
            .bind(table)
            .fetch_one(&mut *conn)
            .await?
            .unwrap_or(false);
        if !exists {
            return Ok(Some(table.to_owned()));
        }
    }
    Ok(None)
}

async fn table_rows(
    conn: &mut PgConnection,
    id: &UserId,
    declaration: &PurgeDeclaration,
    verb: &str,
) -> Result<Option<u64>> {
    let PurgeDeclaration::Table {
        table,
        column,
        predicate,
    } = declaration
    else {
        return Ok(None);
    };
    let sql = table_sql(table, column, predicate.as_ref())?;
    let text = format!("{verb} {}", sql.from_where);
    if verb == "DELETE" {
        let mut query = sqlx::query(AssertSqlSafe(text)).bind(id.as_str());
        if let Some(values) = sql.any_of {
            query = query.bind(values);
        }
        return Ok(Some(query.execute(&mut *conn).await?.rows_affected()));
    }
    let mut query = sqlx::query_scalar::<_, i64>(AssertSqlSafe(text)).bind(id.as_str());
    if let Some(values) = sql.any_of {
        query = query.bind(values);
    }
    let rows = query.fetch_one(&mut *conn).await?;
    Ok(Some(u64::try_from(rows).unwrap_or(0)))
}

impl UserRepository {
    pub(super) async fn purge_user_rows(
        tx: &mut Transaction<'_, Postgres>,
        id: &UserId,
        plan: &AdmittedPurge,
    ) -> Result<PurgeOutcome> {
        let Plan { steps, mut outcome } = load_plan(tx, plan).await?;
        let mut sweeps = Vec::new();
        for step in &steps {
            if let Some(sql) = sweep_sql(&step.declaration)? {
                let keys: Vec<String> = sqlx::query_scalar(AssertSqlSafe(sql.candidates))
                    .bind(id.as_str())
                    .fetch_all(&mut **tx)
                    .await?;
                sweeps.push((step, sql.delete, keys));
            }
        }
        for step in &steps {
            if let Some(rows) = table_rows(tx, id, &step.declaration, "DELETE").await? {
                outcome.counts.push(step.count(rows));
            }
        }
        for (step, delete, keys) in sweeps {
            let result = sqlx::query(AssertSqlSafe(delete))
                .bind(keys)
                .execute(&mut **tx)
                .await?;
            outcome.counts.push(step.count(result.rows_affected()));
        }
        Ok(outcome)
    }

    pub async fn purge_preview(&self, id: &UserId, plan: &AdmittedPurge) -> Result<PurgeOutcome> {
        let mut conn = self.pool.acquire().await?;
        let sessions: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM user_sessions WHERE user_id = $1"#,
            id.as_str()
        )
        .fetch_one(&mut *conn)
        .await?;
        let Plan { steps, mut outcome } = load_plan(&mut conn, plan).await?;
        outcome.counts.push(PurgeCount {
            owner: ExtensionId::new(USERS_OWNER),
            table: "user_sessions".to_owned(),
            rows: sessions,
            absent: false,
        });
        for step in &steps {
            if let Some(rows) =
                table_rows(&mut conn, id, &step.declaration, "SELECT COUNT(*)").await?
            {
                outcome.counts.push(step.count(rows));
            }
        }
        for step in &steps {
            if let Some(sql) = sweep_sql(&step.declaration)? {
                let rows: i64 = sqlx::query_scalar(AssertSqlSafe(sql.preview))
                    .bind(id.as_str())
                    .fetch_one(&mut *conn)
                    .await?;
                outcome
                    .counts
                    .push(step.count(u64::try_from(rows).unwrap_or(0)));
            }
        }
        Ok(outcome)
    }

    pub(super) async fn reassign_owned_rows(
        tx: &mut Transaction<'_, Postgres>,
        from: &UserId,
        to: &UserId,
        plan: &AdmittedOwnerReassignments,
    ) -> Result<Vec<(&'static str, u64)>> {
        let mut moved = Vec::with_capacity(plan.entries().len());
        for owned in plan.entries() {
            let declared = &owned.reassignment;
            let sql = reassign_sql(declared)?;
            let mut query = sqlx::query(AssertSqlSafe(sql.text)).bind(from.as_str());
            if sql.binds_target {
                query = query.bind(to.as_str());
            }
            if let Some(values) = sql.any_of {
                query = query.bind(values);
            }
            let rows = query
                .execute(&mut **tx)
                .await
                .map_err(|source| UserError::OwnerReassignment {
                    owner: owned.owner.clone(),
                    table: declared.table.to_owned(),
                    column: declared.column.to_owned(),
                    source,
                })?
                .rows_affected();
            moved.push((declared.table, rows));
        }
        Ok(moved)
    }
}
