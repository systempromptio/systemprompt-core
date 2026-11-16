//! The user purge's plan and statement text.
//!
//! A purge runs the admitted members' declarations ([`AdmittedPurge`]) and,
//! after them, the recorded declarations of every member absent from this
//! composition, in recorded composition order. Statement text is rendered
//! here from typed declarations: every table and column is a validated,
//! quoted identifier, the user id is bound as `$1` and a predicate's values
//! as `$2`, so nothing a declaration carries is spliced in as SQL.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::admin::SafeIdentifier;
use systemprompt_extension::{AdmittedPurge, OrphanSweep, PurgeDeclaration, PurgeFilter, UserPurgeTable};
use systemprompt_identifiers::ExtensionId;

use crate::error::{Result, UserError};

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

/// How many rows one purge table holds (or held) for a user. `absent` marks
/// a table of a member outside this composition, purged from its recorded
/// declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgeCount {
    pub owner: ExtensionId,
    pub table: String,
    pub rows: i64,
    pub absent: bool,
}

/// A recorded declaration of an absent member whose table no longer exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedPurge {
    pub owner: ExtensionId,
    pub table: String,
}

/// What a purge removed, or a dry run would remove, and what it could not
/// cover. `unrecorded_members` are members absent from this composition that
/// own tables but whose purge declarations were never recorded; the purge
/// cannot vouch for their rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PurgeOutcome {
    pub counts: Vec<PurgeCount>,
    pub skipped: Vec<SkippedPurge>,
    pub unrecorded_members: Vec<ExtensionId>,
}

impl PurgeOutcome {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.skipped.is_empty() && self.unrecorded_members.is_empty()
    }

    #[must_use]
    pub fn rows(&self) -> i64 {
        self.counts.iter().map(|c| c.rows).sum()
    }
}

pub(super) struct RecordedPurge {
    pub(super) owner: ExtensionId,
    pub(super) declarations: Vec<PurgeDeclaration>,
}

pub(super) struct PurgeStep {
    pub(super) owner: ExtensionId,
    pub(super) absent: bool,
    pub(super) declaration: PurgeDeclaration,
}

impl PurgeStep {
    pub(super) fn count(&self, rows: u64) -> PurgeCount {
        PurgeCount {
            owner: self.owner.clone(),
            table: self.declaration.table().to_owned(),
            rows: i64::try_from(rows).unwrap_or(i64::MAX),
            absent: self.absent,
        }
    }
}

pub(super) fn plan_steps(plan: &AdmittedPurge, recorded: Vec<RecordedPurge>) -> Vec<PurgeStep> {
    let admitted = plan.entries().iter().map(|entry| PurgeStep {
        owner: entry.owner.clone(),
        absent: false,
        declaration: entry.declaration.clone(),
    });
    let absent = recorded
        .into_iter()
        .filter(|record| !plan.contains(&record.owner))
        .flat_map(|record| {
            let owner = record.owner;
            record
                .declarations
                .into_iter()
                .map(move |declaration| PurgeStep {
                    owner: owner.clone(),
                    absent: true,
                    declaration,
                })
        });
    admitted.chain(absent).collect()
}

fn identifier(kind: &'static str, raw: &str) -> Result<String> {
    SafeIdentifier::parse(raw)
        .map(|id| id.quoted())
        .map_err(|source| UserError::PurgeIdentifier {
            kind,
            name: raw.to_owned(),
            source,
        })
}

pub(super) struct TableSql {
    pub(super) from_where: String,
    pub(super) any_of: Option<Vec<String>>,
}

pub(super) fn table_sql(
    table: &str,
    column: &str,
    predicate: Option<&PurgeFilter>,
) -> Result<TableSql> {
    let table = identifier("table", table)?;
    let column = identifier("column", column)?;
    // Why: `::text` comparison — purge tables key on TEXT and VARCHAR
    // columns, and the cast keeps one text bind valid for every one of them.
    let mut from_where = format!("FROM {table} WHERE {column}::text = $1");
    let any_of = match predicate {
        None => None,
        Some(filter) => {
            let narrowed = identifier("predicate column", &filter.column)?;
            from_where.push_str(&format!(" AND {narrowed}::text = ANY($2)"));
            Some(filter.any_of.clone())
        },
    };
    Ok(TableSql { from_where, any_of })
}

pub(super) struct SweepSql {
    pub(super) candidates: String,
    pub(super) delete: String,
    pub(super) preview: String,
}

pub(super) fn sweep_sql(declaration: &PurgeDeclaration) -> Result<Option<SweepSql>> {
    let PurgeDeclaration::Sweep {
        table,
        key,
        referenced_by,
        via,
        user_column,
    } = declaration
    else {
        return Ok(None);
    };
    let table = identifier("sweep table", table)?;
    let key = identifier("sweep key", key)?;
    let referenced_by = identifier("sweep referrer", referenced_by)?;
    let via = identifier("sweep reference", via)?;
    let user = identifier("sweep user column", user_column)?;
    let referrer = format!("SELECT 1 FROM {referenced_by} r WHERE r.{via} = t.{key}");
    Ok(Some(SweepSql {
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
    }))
}
