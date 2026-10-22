//! Finds the statements in a migration that populate data rather than
//! declare structure: a backfill.
//!
//! Under the data paradigm (`crate::projection`) an upgrade migration is DDL
//! plus one keys-only `INSERT INTO projection_backlog … SELECT`. Everything a
//! fact needs is in the base schema — the projector, the statement trigger
//! that queues keys, the drainer job — so a fresh install (which stamps
//! migrations and never runs them) and an upgraded one reach the same state
//! by the same code. A migration that instead `UPDATE`s a fact column,
//! `INSERT … SELECT`s a rollup or calls `refresh_*()` inline populates the
//! upgraded database by a path the fresh one never takes, on the boot path,
//! at a cost that grows with history.
//!
//! The detector reports `UPDATE`, `DELETE`, select-driven `INSERT` and any
//! `SELECT` that calls a `refresh_*` routine; the single sanctioned keys-only
//! `INSERT INTO projection_backlog SELECT <projection>, <key> FROM <one
//! relation>` is not reported. An unparseable body yields nothing, since
//! `check_migration_references` already refuses it with the parse error.
//!
//! A `DELETE` from `projection_backlog`, or from a projection's own fact
//! table, is cleanup rather than backfill: it removes rows the base schema's
//! projector would no longer produce, and a fresh install never had them.
//! [`projection_tables`] derives that set from the base schema — every
//! projection a schema queues through `queue_projection_backlog('<name>', …)`
//! whose name is also a table the base schema creates — and
//! [`BackfillStatement::is_projection_cleanup`] applies it.
//!
//! Both repositories' static gates (`no_backfill_after_floor`) use this one
//! detector so "backfill" means the same thing on each side.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use pg_query::NodeEnum;
use pg_query::protobuf::{self, Node};

pub const PROJECTION_BACKLOG: &str = "projection_backlog";

/// One data-populating statement. `position` is 1-based, matching how the
/// runner numbers statements when one fails. `target` is the table written or
/// the function called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillStatement {
    pub position: usize,
    pub target: String,
    pub form: &'static str,
}

impl BackfillStatement {
    #[must_use]
    pub fn is_projection_cleanup(&self, projection_tables: &BTreeSet<String>) -> bool {
        self.form == "DELETE"
            && (self.target == PROJECTION_BACKLOG || projection_tables.contains(&self.target))
    }
}

#[must_use]
pub fn projection_tables<S: AsRef<str>>(schemas: impl IntoIterator<Item = S>) -> BTreeSet<String> {
    let mut queued = BTreeSet::new();
    let mut created = BTreeSet::new();
    for sql in schemas {
        let sql = sql.as_ref();
        queued.extend(queued_projections(sql));
        created.extend(created_tables(sql));
    }
    queued.intersection(&created).cloned().collect()
}

// Why: the queueing calls sit inside PL/pgSQL bodies, which pg_query keeps
// as opaque strings, so the projection names are read from the text.
fn queued_projections(sql: &str) -> Vec<String> {
    const CALL: &str = "queue_projection_backlog(";
    sql.match_indices(CALL)
        .filter_map(|(at, _)| {
            let rest = sql[at + CALL.len()..].trim_start().strip_prefix('\'')?;
            let end = rest.find('\'')?;
            Some(rest[..end].to_owned())
        })
        .collect()
}

fn created_tables(sql: &str) -> Vec<String> {
    let Ok(parsed) = pg_query::parse(sql) else {
        return Vec::new();
    };
    parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| match raw.stmt.as_ref()?.node.as_ref()? {
            NodeEnum::CreateStmt(stmt) => Some(stmt.relation.as_ref()?.relname.clone()),
            _ => None,
        })
        .collect()
}

#[must_use]
pub fn backfill_statements(sql: &str) -> Vec<BackfillStatement> {
    let Ok(parsed) = pg_query::parse(sql) else {
        return Vec::new();
    };
    parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| raw.stmt.as_ref().and_then(|s| s.node.as_ref()))
        .enumerate()
        .filter_map(|(index, node)| {
            classify(node).map(|(target, form)| BackfillStatement {
                position: index + 1,
                target,
                form,
            })
        })
        .collect()
}

fn classify(node: &NodeEnum) -> Option<(String, &'static str)> {
    match node {
        NodeEnum::UpdateStmt(stmt) => Some((stmt.relation.as_ref()?.relname.clone(), "UPDATE")),
        NodeEnum::DeleteStmt(stmt) => Some((stmt.relation.as_ref()?.relname.clone(), "DELETE")),
        NodeEnum::InsertStmt(stmt) => {
            let select = select_of(stmt.select_stmt.as_deref())?;
            let table = stmt.relation.as_ref()?.relname.clone();
            if table == PROJECTION_BACKLOG && is_keys_only(select) {
                return None;
            }
            Some((table, "INSERT … SELECT"))
        },
        NodeEnum::SelectStmt(select) => {
            refresh_call(select).map(|name| (name, "SELECT refresh_*()"))
        },
        _ => None,
    }
}

// Why: pg_query models `INSERT … VALUES` as a SelectStmt carrying
// `values_lists`; only a select without them reads other rows.
fn select_of(node: Option<&Node>) -> Option<&protobuf::SelectStmt> {
    match node?.node.as_ref()? {
        NodeEnum::SelectStmt(select) if select.values_lists.is_empty() => Some(select),
        _ => None,
    }
}

// Why: a keys-only queue insert projects two columns from one relation with
// no join, aggregate or grouping — it costs one index scan, which is the
// whole point of queuing instead of rebuilding. DISTINCT is allowed: it
// collapses the keys before the insert instead of leaving one conflict check
// per ledger row to `ON CONFLICT DO NOTHING`.
fn is_keys_only(select: &protobuf::SelectStmt) -> bool {
    let single_relation = select.from_clause.len() == 1
        && matches!(
            select.from_clause[0].node.as_ref(),
            Some(NodeEnum::RangeVar(_))
        );
    single_relation
        && select.target_list.len() == 2
        && select.group_clause.is_empty()
        && select.having_clause.is_none()
        && select.with_clause.is_none()
}

fn refresh_call(select: &protobuf::SelectStmt) -> Option<String> {
    select.target_list.iter().find_map(|target| {
        let NodeEnum::ResTarget(res) = target.node.as_ref()? else {
            return None;
        };
        let NodeEnum::FuncCall(call) = res.val.as_ref()?.node.as_ref()? else {
            return None;
        };
        let name = call.funcname.last().and_then(|n| match n.node.as_ref()? {
            NodeEnum::String(s) => Some(s.sval.clone()),
            _ => None,
        })?;
        name.starts_with("refresh_").then_some(name)
    })
}
