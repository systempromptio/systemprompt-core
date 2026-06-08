//! Declarative-schema linter.
//!
//! Parses each schema with [`pg_query`] (the actual `PostgreSQL` parser,
//! exposed as a protobuf AST) and walks top-level statements. Classification is
//! by AST node variant rather than keyword tokens, so identifier-equal strings
//! such as a column literally named `alter` do not produce false positives,
//! and dollar-quoted PL/pgSQL bodies are skipped at the parser level.
//!
//! ## Allowed top-level statements
//!
//! - `CreateStmt` — `CREATE TABLE`
//! - `IndexStmt` — `CREATE [UNIQUE] INDEX`
//! - `CreateFunctionStmt`
//! - `ViewStmt` — `CREATE [OR REPLACE] VIEW`
//! - `CreateTrigStmt`
//! - `CompositeTypeStmt` — `CREATE TYPE … AS (…)`
//! - `CreateEnumStmt` — `CREATE TYPE … AS ENUM`
//! - `CreateExtensionStmt`
//! - `CommentStmt` — `COMMENT ON …`
//! - `DropStmt` — only `DROP VIEW`/`MATERIALIZED VIEW`/`INDEX`/`TRIGGER … IF
//!   EXISTS`. These objects are stateless derived artifacts: dropping one loses
//!   no data and the sibling `CREATE …` statement rebuilds it, so the pair
//!   stays idempotent. `DROP TABLE`/`DROP COLUMN` remain rejected.
//!
//! ## Rejected top-level statements
//!
//! - `AlterTableStmt`
//! - `DropStmt` — except the stateless-object carve-out above
//! - `InsertStmt` / `UpdateStmt` / `DeleteStmt` / `TruncateStmt`
//! - `GrantStmt` / `RevokeStmt`
//! - `RenameStmt` — any object rename
//! - `DoStmt` — anonymous `DO $$ … $$` blocks
//! - Any bare `SELECT`/`COPY`/imperative statement
//!
//! ## Semantic checks
//!
//! For statements that reference columns of a table defined elsewhere in the
//! same input (`CREATE INDEX`, `CREATE VIEW`), the linter resolves the
//! `(table, column)` pair against an in-input schema graph built from sibling
//! `CREATE TABLE` nodes. References to tables that are not declared in the
//! same input (e.g. cross-extension `REFERENCES`) are intentionally not
//! resolved — the parser sees those as forward references the database itself
//! validates at apply-time.
//!
//! Column resolution does not descend into:
//!
//! - PL/pgSQL function bodies (resolved by Postgres at function call time)
//! - `CHECK` constraint expressions (resolved by Postgres at table creation)
//! - Trigger function bodies
//!
//! These are deferred so the linter behaves identically to the database for
//! anything it cannot statically prove, avoiding false positives on
//! late-bound names.

mod classify;
mod columns;
mod location;

use std::fmt;

use pg_query::protobuf::node::Node;

use classify::{imperative_reason, warn_create_table_missing_if_not_exists};
use columns::{TableDef, check_index_columns, check_view_columns, collect_create_stmt};
use location::{LineIndex, StmtLoc, stmt_start_offset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    Error,
    Warning,
}

impl fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
            Self::Warning => f.write_str("warning"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintError {
    pub line: u32,
    pub column: u32,
    pub severity: LintSeverity,
    pub message: String,
    pub source: String,
}

impl fmt::Display for LintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}: {}",
            self.source, self.line, self.column, self.severity, self.message
        )
    }
}

/// The single source of truth for which tables an extension *owns*.
///
/// Ownership is derived from the declarative schema, never hand-authored. A
/// parse failure yields an empty list — the linter reports the parse error
/// separately.
#[must_use]
pub fn created_table_names(sql: &str) -> Vec<String> {
    let Ok(parsed) = pg_query::parse(sql) else {
        return Vec::new();
    };
    parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| match raw.stmt.as_ref()?.node.as_ref()? {
            Node::CreateStmt(create) => collect_create_stmt(create).map(|t| t.name().to_owned()),
            _ => None,
        })
        .collect()
}

/// `source` is not read; it is the label stamped into error messages (typically
/// the schema table name or the file path).
pub fn lint_declarative_schema(sql: &str, source: &str) -> Result<(), Vec<LintError>> {
    let parsed = match pg_query::parse(sql) {
        Ok(p) => p,
        Err(e) => {
            return Err(vec![LintError {
                line: 1,
                column: 1,
                severity: LintSeverity::Error,
                message: format!("SQL parse failed: {e}"),
                source: source.to_owned(),
            }]);
        },
    };

    let line_index = LineIndex::new(sql);
    let stmts = &parsed.protobuf.stmts;
    let (tables, mut errors) = classify_pass(stmts, sql, &line_index, source);
    errors.extend(column_ref_pass(stmts, sql, &line_index, &tables, source));

    if errors.iter().any(|e| e.severity == LintSeverity::Error) {
        return Err(errors);
    }
    Ok(())
}

fn classify_pass(
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

fn column_ref_pass(
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
