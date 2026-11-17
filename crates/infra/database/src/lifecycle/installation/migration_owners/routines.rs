//! A routine a migration creates: a `sql`-language body is read as part of
//! the migration, any other language hides the SQL it runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;
use pg_query::protobuf::{CreateFunctionStmt, Node};

use super::kinds::HiddenSql;
use super::statements::{Effect, effects_of, top_level};

fn option<'a>(function: &'a CreateFunctionStmt, name: &str) -> Option<&'a NodeEnum> {
    function
        .options
        .iter()
        .find_map(|option| match option.node.as_ref()? {
            NodeEnum::DefElem(element) if element.defname == name => {
                element.arg.as_ref()?.node.as_ref()
            },
            _ => None,
        })
}

fn language(function: &CreateFunctionStmt) -> String {
    match option(function, "language") {
        Some(NodeEnum::String(s)) => s.sval.to_ascii_lowercase(),
        _ => String::from("sql"),
    }
}

fn body(function: &CreateFunctionStmt) -> Option<&str> {
    let Some(NodeEnum::List(list)) = option(function, "as") else {
        return None;
    };
    list.items
        .first()
        .and_then(|item| match item.node.as_ref()? {
            NodeEnum::String(s) => Some(s.sval.as_str()),
            _ => None,
        })
}

fn atomic_effects(node: &Node) -> Vec<Effect> {
    match node.node.as_ref() {
        Some(NodeEnum::List(list)) => list.items.iter().flat_map(atomic_effects).collect(),
        Some(statement) => effects_of(statement),
        None => Vec::new(),
    }
}

pub(super) fn routine_effects(function: &CreateFunctionStmt) -> Vec<Effect> {
    let language = language(function);
    if language != "sql" {
        return vec![Effect::Hidden(HiddenSql::Routine { language })];
    }
    if let Some(atomic) = function.sql_body.as_deref() {
        return atomic_effects(atomic);
    }
    let Some(sql) = body(function) else {
        return Vec::new();
    };
    pg_query::parse(sql).map_or_else(
        |_| vec![Effect::Hidden(HiddenSql::UnparseableBody)],
        |parsed| top_level(&parsed).flat_map(effects_of).collect(),
    )
}
