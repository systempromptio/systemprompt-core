//! The data writes a migration statement makes, and the routine calls they
//! hide.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;

use super::drops::copied;
use super::kinds::{HiddenSql, TableMutation};
use super::ledger::pinned_members;
use super::statements::{Effect, ctes, write};

fn calls_a_routine(statement: &impl serde::Serialize) -> bool {
    serde_json::to_string(statement).map_or(true, |json| json.contains("\"FuncCall\""))
}

pub(super) fn write_effects(statement: &NodeEnum) -> Option<Vec<Effect>> {
    let calls = match statement {
        NodeEnum::InsertStmt(insert) => calls_a_routine(insert.as_ref()),
        NodeEnum::UpdateStmt(update) => calls_a_routine(update.as_ref()),
        NodeEnum::DeleteStmt(delete) => calls_a_routine(delete.as_ref()),
        NodeEnum::MergeStmt(merge) => calls_a_routine(merge.as_ref()),
        _ => false,
    };
    let mut effects = written(statement)?;
    if calls {
        effects.push(Effect::Hidden(HiddenSql::RoutineCall));
    }
    Some(effects)
}

fn written(statement: &NodeEnum) -> Option<Vec<Effect>> {
    Some(match statement {
        NodeEnum::InsertStmt(insert) => write(
            TableMutation::Insert,
            insert.relation.as_ref(),
            insert.with_clause.as_ref().map_or(&[], |w| &w.ctes),
            None,
        ),
        NodeEnum::UpdateStmt(update) => write(
            TableMutation::Update,
            update.relation.as_ref(),
            update.with_clause.as_ref().map_or(&[], |w| &w.ctes),
            None,
        ),
        NodeEnum::DeleteStmt(delete) => write(
            TableMutation::Delete,
            delete.relation.as_ref(),
            delete.with_clause.as_ref().map_or(&[], |w| &w.ctes),
            pinned_members(delete.where_clause.as_deref()),
        ),
        NodeEnum::MergeStmt(merge) => write(
            TableMutation::Merge,
            merge.relation.as_ref(),
            merge.with_clause.as_ref().map_or(&[], |w| &w.ctes),
            None,
        ),
        NodeEnum::CopyStmt(copy) => copied(copy),
        NodeEnum::SelectStmt(select) => {
            let mut effects = ctes(select.with_clause.as_ref().map_or(&[], |w| &w.ctes));
            if calls_a_routine(select.as_ref()) {
                effects.push(Effect::Hidden(HiddenSql::RoutineCall));
            }
            effects
        },
        _ => return None,
    })
}
