//! Finds the statements in a migration that rewrite a hot table, and pairs
//! them with the cost its author measured.
//!
//! A migration runs inside the boot, before the HTTP listener is bound, in
//! one transaction, holding its locks, and every per-row trigger on the table
//! fires for every row it touches. That is how a 3,644-row `UPDATE
//! ai_requests` took 27 minutes on a production instance: one trigger
//! re-enqueued the whole client session per row, 113 ms a row. Suspending it
//! took the same statement to 2.0 s. The same shape emptied `logs` once and
//! wrote 77,797 outbox tombstones and as many `pg_notify` calls in a single
//! transaction.
//!
//! What this module reports is deliberately blunt: any write to a table on
//! the hot list, plus the `ALTER`/`CREATE INDEX` forms that take a full scan
//! or a blocking lock. Judging whether a `WHERE` clause is selective is not
//! something a parser can do — the author is the one who can measure it, and
//! [`CostDirective`] is where they say so.
//!
//! Two consumers, one detector. At boot this warns: a missing comment must
//! never brick an upgrade, and the statement timeout derived from `measured`
//! is what actually bounds the damage. In each repo's test suite the same
//! findings are a hard failure, which is where an unmeasured backfill is
//! supposed to be caught — in the pull request, not on a customer's server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use pg_query::NodeEnum;
use pg_query::protobuf::AlterTableType;
use systemprompt_extension::Extension;
use systemprompt_extension::cost::{self, CostDirective};

/// The tables core ships that are large enough for a blind rewrite to matter.
/// Row counts are from the 2026-09-22 production analysis; every one of them
/// grows with traffic and none is ever pruned to a bounded size.
pub const HOT_TABLES: &[&str] = &[
    "ai_requests",
    "ai_request_messages",
    "ai_request_payloads",
    "ai_request_client_evidence",
    "ai_request_tool_calls",
    "analytics_events",
    "event_outbox",
    "logs",
    "user_sessions",
];

/// One statement that rewrites or rescans a hot table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpensiveStatement {
    /// 1-based, matching how the runner numbers statements when one fails.
    pub position: usize,
    pub table: String,
    pub form: &'static str,
}

/// One migration's expensive statements and what it declared about them.
#[derive(Debug, Clone)]
pub struct MigrationCost {
    pub extension: String,
    pub migration: String,
    pub statements: Vec<ExpensiveStatement>,
    pub declared: Option<CostDirective>,
    /// Set when the body carries a `@cost` line that does not parse.
    pub malformed: Option<String>,
}

impl MigrationCost {
    /// The migration does bulk work and never says what it costs.
    #[must_use]
    pub fn is_undeclared(&self) -> bool {
        !self.statements.is_empty() && self.declared.is_none()
    }

    #[must_use]
    pub fn label(&self) -> String {
        format!("{}/{}", self.extension, self.migration)
    }

    #[must_use]
    pub fn statement_summary(&self) -> String {
        self.statements
            .iter()
            .map(|s| format!("statement {} {} {}", s.position, s.form, s.table))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Audits every non-tombstone migration of every extension.
///
/// `hot` is passed rather than read from [`HOT_TABLES`] so an installation
/// can add the tables it owns — core cannot know about a downstream repo's
/// hottest table.
#[must_use]
pub fn audit_migration_cost(extensions: &[Arc<dyn Extension>], hot: &[&str]) -> Vec<MigrationCost> {
    let mut out = Vec::new();
    for ext in extensions {
        let extension = ext.id().to_owned();
        for migration in ext.migrations().into_iter().filter(|m| !m.tombstone) {
            let label = format!("{:03}_{}", migration.version, migration.name);
            if let Some(cost) = audit_one(&extension, &label, migration.sql, hot) {
                out.push(cost);
            }
        }
    }
    out
}

/// Audits one migration body. `None` when it neither does bulk work nor
/// declares a cost, which is the ordinary case.
#[must_use]
pub fn audit_one(
    extension: &str,
    migration: &str,
    sql: &str,
    hot: &[&str],
) -> Option<MigrationCost> {
    let (declared, malformed) = match cost::parse(sql) {
        Ok(found) => (found, None),
        Err(e) => (None, Some(e.to_string())),
    };
    let statements = expensive_statements(sql, hot);
    if statements.is_empty() && declared.is_none() && malformed.is_none() {
        return None;
    }
    Some(MigrationCost {
        extension: extension.to_owned(),
        migration: migration.to_owned(),
        statements,
        declared,
        malformed,
    })
}

// Why: an unparseable body is not this check's business — `migration_refs`
// already refuses it with the parse error, and reporting it twice would only
// bury that message.
fn expensive_statements(sql: &str, hot: &[&str]) -> Vec<ExpensiveStatement> {
    let Ok(parsed) = pg_query::parse(sql) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (index, node) in parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| raw.stmt.as_ref().and_then(|s| s.node.as_ref()))
        .enumerate()
    {
        let position = index + 1;
        if let Some((table, form)) = classify(node) {
            if hot.contains(&table.as_str()) {
                out.push(ExpensiveStatement {
                    position,
                    table,
                    form,
                });
            }
        }
    }
    out
}

fn is_select_driven(select: Option<&pg_query::protobuf::Node>) -> bool {
    let Some(NodeEnum::SelectStmt(select)) = select.and_then(|n| n.node.as_ref()) else {
        return false;
    };
    select.values_lists.is_empty()
}

fn classify(node: &NodeEnum) -> Option<(String, &'static str)> {
    match node {
        NodeEnum::UpdateStmt(stmt) => Some((stmt.relation.as_ref()?.relname.clone(), "UPDATE on")),
        NodeEnum::DeleteStmt(stmt) => {
            Some((stmt.relation.as_ref()?.relname.clone(), "DELETE from"))
        },
        // Why: `INSERT … VALUES` writes what the author typed; only a
        // select-driven insert scales with the table it reads. The parser
        // models both as a SelectStmt hanging off the insert, so the two are
        // told apart by that node carrying rows of its own rather than a
        // FROM — without this, every literal insert reads as a backfill.
        NodeEnum::InsertStmt(stmt) if is_select_driven(stmt.select_stmt.as_deref()) => Some((
            stmt.relation.as_ref()?.relname.clone(),
            "INSERT … SELECT into",
        )),
        // Why: a non-concurrent index build holds a write lock for the whole
        // build; concurrently is the form that does not stop traffic.
        NodeEnum::IndexStmt(stmt) if !stmt.concurrent => Some((
            stmt.relation.as_ref()?.relname.clone(),
            "CREATE INDEX (not CONCURRENTLY) on",
        )),
        NodeEnum::AlterTableStmt(stmt) => {
            let table = stmt.relation.as_ref()?.relname.clone();
            let form = stmt.cmds.iter().find_map(|cmd| match cmd.node.as_ref() {
                Some(NodeEnum::AlterTableCmd(c)) => scanning_alter(c.subtype),
                _ => None,
            })?;
            Some((table, form))
        },
        _ => None,
    }
}

// Why: both forms read every existing row before they can be recorded, and
// both take an ACCESS EXCLUSIVE or SHARE UPDATE EXCLUSIVE lock while doing it.
fn scanning_alter(subtype: i32) -> Option<&'static str> {
    if subtype == AlterTableType::AtValidateConstraint as i32 {
        return Some("ALTER TABLE … VALIDATE CONSTRAINT on");
    }
    if subtype == AlterTableType::AtSetNotNull as i32 {
        return Some("ALTER TABLE … SET NOT NULL on");
    }
    None
}
