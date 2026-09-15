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
//! A `FOREIGN KEY` whose referenced table is declared in the same input must
//! find a `PRIMARY KEY` or `UNIQUE` on exactly the referenced columns in that
//! table's `CREATE TABLE`. The installer applies foreign keys last, after
//! migrations and indexes, so the key itself installs anywhere — but on a
//! fresh database only the declarative schema runs, and the uniqueness has to
//! be declared where the key can see it. For this rule the "input" is every
//! schema file of one extension together ([`lint_declarative_schemas`]);
//! positions are still reported per file.
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
//!
//! The per-statement rules and column references are checked per input with
//! that input's own line numbers; table definitions accumulate across inputs
//! so a foreign key in one file resolves the table another file declares.
//!
//! Both lint entry points return `Ok(warnings)` when no error was found and
//! `Err(findings)` — every warning and error — otherwise, so a caller never
//! has to drop the advisory findings to learn the verdict. Table names from
//! [`created_table_names`] are schema-qualified (`kb.docs`) when the
//! `CREATE TABLE` names a schema and bare otherwise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod classify;
mod columns;
mod foreign_keys;
mod location;
mod passes;

use std::fmt;

use pg_query::protobuf::node::Node;

use columns::{TableDef, collect_create_stmt};
use location::LineIndex;
use passes::{classify_pass, column_ref_pass, foreign_key_pass};

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

pub fn created_table_names(sql: &str) -> Result<Vec<String>, pg_query::Error> {
    let parsed = pg_query::parse(sql)?;
    Ok(parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| match raw.stmt.as_ref()?.node.as_ref()? {
            Node::CreateStmt(create) => collect_create_stmt(create).map(|t| t.qualified_name()),
            _ => None,
        })
        .collect())
}

pub fn lint_declarative_schema(sql: &str, source: &str) -> Result<Vec<LintError>, Vec<LintError>> {
    lint_declarative_schemas(&[(source, sql)])
}

pub fn lint_declarative_schemas(inputs: &[(&str, &str)]) -> Result<Vec<LintError>, Vec<LintError>> {
    let mut errors: Vec<LintError> = Vec::new();
    let mut parsed_inputs = Vec::with_capacity(inputs.len());
    let mut tables: Vec<TableDef> = Vec::new();

    for (source, sql) in inputs {
        let parsed = match pg_query::parse(sql) {
            Ok(p) => p,
            Err(e) => {
                errors.push(LintError {
                    line: 1,
                    column: 1,
                    severity: LintSeverity::Error,
                    message: format!("SQL parse failed: {e}"),
                    source: (*source).to_owned(),
                });
                continue;
            },
        };
        let line_index = LineIndex::new(sql);
        let (found, mut found_errors) =
            classify_pass(&parsed.protobuf.stmts, sql, &line_index, source);
        errors.append(&mut found_errors);
        errors.extend(column_ref_pass(
            &parsed.protobuf.stmts,
            sql,
            &line_index,
            &found,
            source,
        ));
        tables.extend(found);
        parsed_inputs.push((*source, *sql, parsed, line_index));
    }

    for (source, sql, parsed, line_index) in &parsed_inputs {
        errors.extend(foreign_key_pass(
            &parsed.protobuf.stmts,
            sql,
            line_index,
            &tables,
            source,
        ));
    }

    if errors.iter().any(|e| e.severity == LintSeverity::Error) {
        return Err(errors);
    }
    Ok(errors)
}
