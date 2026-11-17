//! The members a ledger `DELETE` names: a `WHERE extension_id = '<id>'` or
//! `WHERE extension_id IN ('<id>', ...)` of string literals, and nothing
//! else. And the kernel ledgers a view, materialized view or rule reads: a
//! simple view over a ledger is auto-updatable, so a write into the view is a
//! write into the ledger, and a rule's condition and actions run against what
//! they name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;
use std::collections::BTreeSet;

use pg_query::protobuf::{AExprKind, Node, ObjectType, a_const};

use super::super::undeclared::qualify;
use super::KERNEL_LEDGERS;
use super::kinds::TableMutation;

fn literal(node: &Node) -> Option<String> {
    let NodeEnum::AConst(constant) = node.node.as_ref()? else {
        return None;
    };
    match constant.val.as_ref()? {
        a_const::Val::Sval(s) => Some(s.sval.clone()),
        _ => None,
    }
}

fn is_member_column(node: &Node) -> bool {
    let Some(NodeEnum::ColumnRef(column)) = node.node.as_ref() else {
        return false;
    };
    matches!(
        column.fields.last().and_then(|field| field.node.as_ref()),
        Some(NodeEnum::String(s)) if s.sval == "extension_id"
    )
}

pub(super) fn pinned_members(where_clause: Option<&Node>) -> Option<Vec<String>> {
    let NodeEnum::AExpr(expr) = where_clause?.node.as_ref()? else {
        return None;
    };
    if !is_member_column(expr.lexpr.as_deref()?) {
        return None;
    }
    let right = expr.rexpr.as_deref()?;
    match AExprKind::try_from(expr.kind).ok()? {
        AExprKind::AexprOp => {
            let operator = expr
                .name
                .first()
                .and_then(|name| match name.node.as_ref()? {
                    NodeEnum::String(s) => Some(s.sval.as_str()),
                    _ => None,
                });
            (operator == Some("="))
                .then(|| literal(right))
                .flatten()
                .map(|id| vec![id])
        },
        AExprKind::AexprIn => {
            let NodeEnum::List(list) = right.node.as_ref()? else {
                return None;
            };
            list.items.iter().map(literal).collect()
        },
        _ => None,
    }
}

pub(super) fn ledger_reads(statement: &NodeEnum) -> Vec<(TableMutation, String)> {
    let (kind, read) = match statement {
        NodeEnum::ViewStmt(view) => (
            TableMutation::CreateObject,
            serde_json::to_value(&view.query),
        ),
        NodeEnum::CreateTableAsStmt(create)
            if create.objtype == ObjectType::ObjectMatview as i32 =>
        {
            (
                TableMutation::CreateObject,
                serde_json::to_value(&create.query),
            )
        },
        NodeEnum::RuleStmt(rule) => (
            TableMutation::CreateRule,
            serde_json::to_value((&rule.where_clause, &rule.actions)),
        ),
        _ => return Vec::new(),
    };
    let mut named = BTreeSet::new();
    match read {
        Ok(read) => range_vars(&read, &mut named),
        Err(_) => named.extend(KERNEL_LEDGERS.iter().map(|ledger| (*ledger).to_owned())),
    }
    named
        .into_iter()
        .filter(|table| KERNEL_LEDGERS.contains(&table.as_str()))
        .map(|table| (kind, table))
        .collect()
}

fn range_vars(value: &serde_json::Value, named: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, field) in fields {
                if key == "RangeVar"
                    && let Some(relname) = field.get("relname").and_then(serde_json::Value::as_str)
                    && !relname.is_empty()
                {
                    let schema = field
                        .get("schemaname")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    named.insert(if schema.is_empty() {
                        qualify(relname)
                    } else {
                        format!("{schema}.{relname}")
                    });
                }
                range_vars(field, named);
            }
        },
        serde_json::Value::Array(items) => {
            for item in items {
                range_vars(item, named);
            }
        },
        _ => {},
    }
}
