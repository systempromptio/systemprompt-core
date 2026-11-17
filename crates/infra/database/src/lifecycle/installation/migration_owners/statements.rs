//! What a migration statement does, as the parser reads it: the objects it
//! changes and the SQL it hides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;
use pg_query::protobuf::{Node, ObjectType, RangeVar};

use super::super::undeclared::qualify;
use super::drops::{copied, dropped, granted, renamed};
use super::kinds::{HiddenSql, TableMutation};
use super::ledger::pinned_members;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Effect {
    Table(TableMutation, String),
    Write(TableMutation, String, Option<Vec<String>>),
    Schema(TableMutation, Option<String>),
    Object(TableMutation, String),
    Cascade(String),
    Hidden(HiddenSql),
}

pub(super) fn top_level(parsed: &pg_query::ParseResult) -> impl Iterator<Item = &NodeEnum> {
    parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| raw.stmt.as_ref().and_then(|s| s.node.as_ref()))
}

pub(super) fn relation_name(relation: &RangeVar) -> Option<String> {
    if relation.relname.is_empty() {
        return None;
    }
    Some(if relation.schemaname.is_empty() {
        qualify(&relation.relname)
    } else {
        format!("{}.{}", relation.schemaname, relation.relname)
    })
}

pub(super) fn strings(items: &[Node]) -> Vec<&str> {
    items
        .iter()
        .filter_map(|item| match item.node.as_ref()? {
            NodeEnum::String(s) => Some(s.sval.as_str()),
            _ => None,
        })
        .collect()
}

pub(super) fn qualified(parts: &[&str]) -> Option<String> {
    match parts {
        [name] => Some(qualify(name)),
        [schema, name] => Some(format!("{schema}.{name}")),
        _ => None,
    }
}

pub(super) fn object_name(object: &Node) -> Option<String> {
    match object.node.as_ref()? {
        NodeEnum::List(list) => qualified(&strings(&list.items)),
        NodeEnum::ObjectWithArgs(with) => qualified(&strings(&with.objname)),
        NodeEnum::TypeName(type_name) => qualified(&strings(&type_name.names)),
        NodeEnum::String(s) => Some(qualify(&s.sval)),
        _ => None,
    }
}

pub(super) fn object_on_table(object: &Node) -> Option<String> {
    let NodeEnum::List(list) = object.node.as_ref()? else {
        return None;
    };
    let parts = strings(&list.items);
    let (_, table) = parts.split_last()?;
    qualified(table)
}

pub(super) fn object_type(value: i32) -> Option<ObjectType> {
    ObjectType::try_from(value).ok()
}

pub(super) fn is_table(value: i32) -> bool {
    matches!(
        object_type(value),
        Some(ObjectType::ObjectTable | ObjectType::ObjectForeignTable)
    )
}

pub(super) fn on(kind: TableMutation, relation: Option<&RangeVar>) -> Vec<Effect> {
    relation
        .and_then(relation_name)
        .map(|table| Effect::Table(kind, table))
        .into_iter()
        .collect()
}

fn write(
    kind: TableMutation,
    relation: Option<&RangeVar>,
    with: &[Node],
    pinned: Option<Vec<String>>,
) -> Vec<Effect> {
    let mut effects: Vec<Effect> = relation
        .and_then(relation_name)
        .map(|table| Effect::Write(kind, table, pinned))
        .into_iter()
        .collect();
    effects.extend(ctes(with));
    effects
}

fn ctes(with: &[Node]) -> Vec<Effect> {
    with.iter()
        .filter_map(|cte| match cte.node.as_ref()? {
            NodeEnum::CommonTableExpr(cte) => cte.ctequery.as_ref()?.node.as_ref(),
            _ => None,
        })
        .flat_map(effects_of)
        .collect()
}

fn calls_a_routine(statement: &impl serde::Serialize) -> bool {
    serde_json::to_string(statement).map_or(true, |json| json.contains("\"FuncCall\""))
}

pub(super) fn effects_of(statement: &NodeEnum) -> Vec<Effect> {
    match statement {
        NodeEnum::AlterTableStmt(alter) => match object_type(alter.objtype) {
            Some(ObjectType::ObjectTable | ObjectType::ObjectForeignTable) => {
                on(TableMutation::Alter, alter.relation.as_ref())
            },
            Some(ObjectType::ObjectIndex) => alter
                .relation
                .as_ref()
                .and_then(relation_name)
                .map(|index| Effect::Object(TableMutation::AlterIndex, index))
                .into_iter()
                .collect(),
            _ => Vec::new(),
        },
        NodeEnum::RenameStmt(rename) => renamed(rename),
        NodeEnum::AlterObjectSchemaStmt(moved) => match object_type(moved.object_type) {
            Some(
                ObjectType::ObjectTable
                | ObjectType::ObjectForeignTable
                | ObjectType::ObjectView
                | ObjectType::ObjectMatview
                | ObjectType::ObjectSequence,
            ) => on(TableMutation::SetSchema, moved.relation.as_ref()),
            _ => moved
                .object
                .as_deref()
                .and_then(object_name)
                .map(|name| Effect::Object(TableMutation::SetSchema, name))
                .into_iter()
                .collect(),
        },
        NodeEnum::AlterTableMoveAllStmt(_) => vec![Effect::Schema(TableMutation::MoveAll, None)],
        NodeEnum::TruncateStmt(truncate) => truncate
            .relations
            .iter()
            .filter_map(|node| match node.node.as_ref()? {
                NodeEnum::RangeVar(relation) => relation_name(relation),
                _ => None,
            })
            .map(|table| Effect::Table(TableMutation::Truncate, table))
            .collect(),
        NodeEnum::DropStmt(drop) => dropped(drop),
        NodeEnum::IndexStmt(index) => on(TableMutation::CreateIndex, index.relation.as_ref()),
        NodeEnum::CreateTrigStmt(trigger) => {
            on(TableMutation::CreateTrigger, trigger.relation.as_ref())
        },
        NodeEnum::RuleStmt(rule) => on(TableMutation::CreateRule, rule.relation.as_ref()),
        NodeEnum::CreatePolicyStmt(policy) => {
            on(TableMutation::CreatePolicy, policy.table.as_ref())
        },
        NodeEnum::AlterPolicyStmt(policy) => on(TableMutation::AlterPolicy, policy.table.as_ref()),
        NodeEnum::GrantStmt(grant) => granted(grant),
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
        NodeEnum::CreateFunctionStmt(function) => super::routines::routine_effects(function),
        NodeEnum::CreateSchemaStmt(schema) => schema
            .schema_elts
            .iter()
            .filter_map(|element| element.node.as_ref())
            .flat_map(effects_of)
            .collect(),
        NodeEnum::DoStmt(_) => vec![Effect::Hidden(HiddenSql::Do)],
        NodeEnum::CallStmt(_) => vec![Effect::Hidden(HiddenSql::Call)],
        NodeEnum::ExecuteStmt(_) => vec![Effect::Hidden(HiddenSql::Execute)],
        NodeEnum::CreateStmt(_)
        | NodeEnum::CreateTableAsStmt(_)
        | NodeEnum::ViewStmt(_)
        | NodeEnum::CreateSeqStmt(_)
        | NodeEnum::AlterSeqStmt(_)
        | NodeEnum::CreateEnumStmt(_)
        | NodeEnum::AlterEnumStmt(_)
        | NodeEnum::CompositeTypeStmt(_)
        | NodeEnum::CreateDomainStmt(_)
        | NodeEnum::AlterDomainStmt(_)
        | NodeEnum::CreateRangeStmt(_)
        | NodeEnum::CommentStmt(_)
        | NodeEnum::VariableSetStmt(_)
        | NodeEnum::TransactionStmt(_)
        | NodeEnum::CreateExtensionStmt(_)
        | NodeEnum::LockStmt(_)
        | NodeEnum::AlterFunctionStmt(_)
        | NodeEnum::RefreshMatViewStmt(_)
        | NodeEnum::NotifyStmt(_)
        | NodeEnum::VacuumStmt(_)
        | NodeEnum::ReindexStmt(_) => Vec::new(),
        other => vec![Effect::Hidden(HiddenSql::Unclassified {
            kind: statement_kind(other),
        })],
    }
}

fn statement_kind(statement: &NodeEnum) -> String {
    let debug = format!("{statement:?}");
    debug
        .split(['(', ' ', '{'])
        .next()
        .unwrap_or_default()
        .to_owned()
}
