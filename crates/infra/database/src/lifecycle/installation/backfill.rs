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
//! Both repositories' static gates (`no_backfill_after_floor`) use this one
//! detector so "backfill" means the same thing on each side.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

/// Every statement in `sql` that writes rows from other rows: `UPDATE`,
/// `DELETE`, select-driven `INSERT` and any `SELECT` that calls a
/// `refresh_*` routine. The single sanctioned form — `INSERT INTO
/// projection_backlog SELECT <projection>, <key> FROM <one relation>` with no
/// join — is not reported. An unparseable body yields nothing;
/// `check_migration_references` already refuses it with the parse error.
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
// whole point of queuing instead of rebuilding.
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
        && select.distinct_clause.is_empty()
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
