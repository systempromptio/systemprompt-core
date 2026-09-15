//! The three statement walks over one parsed input: classification of
//! top-level statements (which also collects the in-input table graph),
//! column resolution for `CREATE INDEX` / `CREATE VIEW`, and the foreign-key
//! uniqueness rule that runs once every input's tables are known.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::protobuf::node::Node;

use super::classify::{imperative_reason, warn_create_table_missing_if_not_exists};
use super::columns::{TableDef, check_index_columns, check_view_columns, collect_create_stmt};
use super::foreign_keys::check_foreign_keys;
use super::location::{LineIndex, StmtLoc, stmt_start_offset};
use super::{LintError, LintSeverity};

pub(super) fn foreign_key_pass(
    stmts: &[pg_query::protobuf::RawStmt],
    sql: &str,
    line_index: &LineIndex,
    tables: &[TableDef],
    source: &str,
) -> Vec<LintError> {
    let mut errors: Vec<LintError> = Vec::new();
    for raw in stmts {
        let Some(Node::CreateStmt(create)) = raw.stmt.as_ref().and_then(|s| s.node.as_ref()) else {
            continue;
        };
        let location = stmt_start_offset(sql, raw.stmt_location.max(0) as usize);
        let (line, col) = line_index.position(location);
        let loc = StmtLoc { line, col, source };
        check_foreign_keys(create, tables, &loc, &mut errors);
    }
    errors
}

pub(super) fn classify_pass(
    stmts: &[pg_query::protobuf::RawStmt],
    sql: &str,
    line_index: &LineIndex,
    source: &str,
) -> (Vec<TableDef>, Vec<LintError>) {
    let mut errors: Vec<LintError> = Vec::new();
    let mut tables: Vec<TableDef> = Vec::new();

    for raw in stmts {
        let location = stmt_start_offset(sql, raw.stmt_location.max(0) as usize);
        let (line, col) = line_index.position(location);
        let loc = StmtLoc { line, col, source };

        let Some(stmt) = raw.stmt.as_ref() else {
            continue;
        };
        let Some(node) = stmt.node.as_ref() else {
            continue;
        };

        match node {
            Node::CreateStmt(create) => {
                if let Some(table) = collect_create_stmt(create) {
                    tables.push(table);
                }
                if let Some(warn) = warn_create_table_missing_if_not_exists(create, &loc) {
                    errors.push(warn);
                }
            },
            Node::IndexStmt(_)
            | Node::CreateFunctionStmt(_)
            | Node::ViewStmt(_)
            | Node::CreateTrigStmt(_)
            | Node::CompositeTypeStmt(_)
            | Node::CreateEnumStmt(_)
            | Node::CommentStmt(_) => {},
            Node::CreateExtensionStmt(ext) => {
                if !ext.if_not_exists {
                    errors.push(LintError {
                        line,
                        column: col,
                        severity: LintSeverity::Warning,
                        message: "CREATE EXTENSION without IF NOT EXISTS".into(),
                        source: source.to_owned(),
                    });
                }
            },
            other => {
                if let Some(reason) = imperative_reason(other) {
                    errors.push(LintError {
                        line,
                        column: col,
                        severity: LintSeverity::Error,
                        message: format!(
                            "imperative SQL in declarative schema: {reason} — move to \
                             schema/migrations/NNN_<name>.sql"
                        ),
                        source: source.to_owned(),
                    });
                }
            },
        }
    }

    (tables, errors)
}

pub(super) fn column_ref_pass(
    stmts: &[pg_query::protobuf::RawStmt],
    sql: &str,
    line_index: &LineIndex,
    tables: &[TableDef],
    source: &str,
) -> Vec<LintError> {
    let mut errors: Vec<LintError> = Vec::new();

    for raw in stmts {
        let Some(stmt) = raw.stmt.as_ref() else {
            continue;
        };
        let Some(node) = stmt.node.as_ref() else {
            continue;
        };
        let location = stmt_start_offset(sql, raw.stmt_location.max(0) as usize);
        let (line, col) = line_index.position(location);
        let loc = StmtLoc { line, col, source };

        match node {
            Node::IndexStmt(idx) => {
                check_index_columns(idx, tables, &loc, &mut errors);
            },
            Node::ViewStmt(view) => {
                check_view_columns(view, tables, &loc, &mut errors);
            },
            _ => {},
        }
    }

    errors
}
