//! The tables a migration statement mutates, as the parser reads them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;
use pg_query::protobuf::{ObjectType, RangeVar};

use super::TableMutation;
use super::super::undeclared::qualify;

pub(super) fn top_level(parsed: &pg_query::ParseResult) -> impl Iterator<Item = &NodeEnum> {
    parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| raw.stmt.as_ref().and_then(|s| s.node.as_ref()))
}

fn relation_name(relation: &RangeVar) -> Option<String> {
    if relation.relname.is_empty() {
        return None;
    }
    Some(if relation.schemaname.is_empty() {
        qualify(&relation.relname)
    } else {
        format!("{}.{}", relation.schemaname, relation.relname)
    })
}

fn is_table(object: i32) -> bool {
    matches!(
        ObjectType::try_from(object),
        Ok(ObjectType::ObjectTable | ObjectType::ObjectForeignTable)
    )
}

pub(super) fn mutations_of(statement: &NodeEnum) -> Vec<(TableMutation, String)> {
    let named = |kind, relation: Option<&RangeVar>| {
        relation
            .and_then(relation_name)
            .map(|table| (kind, table))
            .into_iter()
            .collect::<Vec<_>>()
    };
    match statement {
        NodeEnum::AlterTableStmt(alter) if is_table(alter.objtype) => {
            named(TableMutation::Alter, alter.relation.as_ref())
        },
        NodeEnum::RenameStmt(rename)
            if matches!(
                ObjectType::try_from(rename.rename_type),
                Ok(ObjectType::ObjectTable
                    | ObjectType::ObjectColumn
                    | ObjectType::ObjectTabconstraint)
            ) =>
        {
            named(TableMutation::Rename, rename.relation.as_ref())
        },
        NodeEnum::TruncateStmt(truncate) => truncate
            .relations
            .iter()
            .filter_map(|node| match node.node.as_ref()? {
                NodeEnum::RangeVar(relation) => relation_name(relation),
                _ => None,
            })
            .map(|table| (TableMutation::Truncate, table))
            .collect(),
        NodeEnum::DropStmt(drop) if is_table(drop.remove_type) => drop
            .objects
            .iter()
            .filter_map(dropped_table)
            .map(|table| (TableMutation::Drop, table))
            .collect(),
        _ => Vec::new(),
    }
}

fn dropped_table(object: &pg_query::protobuf::Node) -> Option<String> {
    let NodeEnum::List(list) = object.node.as_ref()? else {
        return None;
    };
    let parts: Vec<&str> = list
        .items
        .iter()
        .filter_map(|item| match item.node.as_ref()? {
            NodeEnum::String(s) => Some(s.sval.as_str()),
            _ => None,
        })
        .collect();
    match parts.as_slice() {
        [table] => Some(qualify(table)),
        [schema, table] => Some(format!("{schema}.{table}")),
        _ => None,
    }
}
