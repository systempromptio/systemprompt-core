//! Parser/validator for admin-supplied SQL strings.
//!
//! Both modes parse with `pg_query` and accept exactly one statement.
//! [`AdminSql::parse_readonly`] additionally requires the statement root to
//! be a `SELECT`, `EXPLAIN` of a `SELECT`, or `SHOW`, and refuses any
//! data-modifying, DDL or utility node anywhere in the tree — including a
//! CTE that writes. The executor runs read-only statements inside a
//! `READ ONLY` transaction so Postgres refuses what the parse cannot see
//! (a volatile function that writes).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;
use thiserror::Error;

pub const DEFAULT_READONLY_ROW_LIMIT: usize = 1000;

#[derive(Debug, Error)]
pub enum AdminSqlError {
    #[error("SQL query is empty")]
    Empty,
    #[error("SQL query could not be parsed: {0}")]
    Parse(#[from] pg_query::Error),
    #[error("SQL query contains multiple statements; only one is allowed")]
    MultipleStatements,
    #[error("SQL query must be a SELECT, an EXPLAIN of a SELECT, or SHOW")]
    NotReadOnly,
    #[error("SQL query contains a data-modifying, DDL or utility statement in read-only mode")]
    WriteInReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminSql(String);

impl AdminSql {
    pub fn parse_readonly(raw: &str) -> Result<Self, AdminSqlError> {
        let (text, root) = single_statement(raw)?;
        let inner = match &root {
            NodeEnum::ExplainStmt(explain) => explain
                .query
                .as_ref()
                .and_then(|q| q.node.as_ref())
                .ok_or(AdminSqlError::NotReadOnly)?,
            other => other,
        };
        match inner {
            NodeEnum::SelectStmt(_) | NodeEnum::VariableShowStmt(_) => {},
            _ => return Err(AdminSqlError::NotReadOnly),
        }
        let nested = statement_kinds(inner)?;
        if nested
            .iter()
            .any(|kind| !READONLY_ROOTS.contains(&kind.as_str()))
        {
            return Err(AdminSqlError::WriteInReadOnly);
        }
        Ok(Self(text))
    }

    pub fn parse_unrestricted(raw: &str) -> Result<Self, AdminSqlError> {
        let (text, _) = single_statement(raw)?;
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn single_statement(raw: &str) -> Result<(String, NodeEnum), AdminSqlError> {
    let parsed = pg_query::parse(raw)?;
    let mut stmts = parsed.protobuf.stmts.into_iter();
    let Some(first) = stmts.next() else {
        return Err(AdminSqlError::Empty);
    };
    if stmts.next().is_some() {
        return Err(AdminSqlError::MultipleStatements);
    }
    let node = first
        .stmt
        .and_then(|s| s.node)
        .ok_or(AdminSqlError::Empty)?;
    let start = usize::try_from(first.stmt_location).unwrap_or(0);
    let end = if first.stmt_len > 0 {
        start.saturating_add(usize::try_from(first.stmt_len).unwrap_or(0))
    } else {
        raw.len()
    };
    let text = raw.get(start..end).unwrap_or(raw).trim();
    let text = text.strip_suffix(';').unwrap_or(text).trim_end();
    Ok((text.to_owned(), node))
}

const READONLY_ROOTS: &[&str] = &["SelectStmt", "VariableShowStmt"];

// Why: the typed `nodes()` walk skips node kinds it does not model, so the
// exhaustive check serialises the protobuf tree and looks at every statement
// node wherever it sits — a CTE, a subquery, a function argument.
fn statement_kinds(root: &NodeEnum) -> Result<Vec<String>, AdminSqlError> {
    // JSON: pg_query protobuf AST, externally tagged by node variant name
    let tree = serde_json::to_value(root)
        .map_err(|e| AdminSqlError::Parse(pg_query::Error::InvalidJson(e.to_string())))?;
    let mut kinds = Vec::new();
    collect_statement_kinds(&tree, &mut kinds);
    Ok(kinds)
}

fn collect_statement_kinds(value: &serde_json::Value, kinds: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if key.ends_with("Stmt") {
                    kinds.push(key.clone());
                }
                collect_statement_kinds(child, kinds);
            }
        },
        serde_json::Value::Array(items) => {
            for item in items {
                collect_statement_kinds(item, kinds);
            }
        },
        _ => {},
    }
}
