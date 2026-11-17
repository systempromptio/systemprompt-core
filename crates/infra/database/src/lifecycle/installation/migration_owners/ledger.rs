//! The members a ledger `DELETE` names: a `WHERE extension_id = '<id>'` or
//! `WHERE extension_id IN ('<id>', ...)` of string literals, and nothing
//! else.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;
use pg_query::protobuf::{AExprKind, Node, a_const};

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
            let operator = expr.name.first().and_then(|name| match name.node.as_ref()? {
                NodeEnum::String(s) => Some(s.sval.as_str()),
                _ => None,
            });
            (operator == Some("=")).then(|| literal(right)).flatten().map(|id| vec![id])
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
