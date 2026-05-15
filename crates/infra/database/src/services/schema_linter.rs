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
//!   EXISTS`. These objects are stateless derived artifacts: dropping one
//!   loses no data and the sibling `CREATE …` statement rebuilds it, so the
//!   pair stays idempotent. `DROP TABLE`/`DROP COLUMN` remain rejected.
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

use std::fmt;

use pg_query::protobuf::node::Node;
use pg_query::protobuf::{ColumnDef, CreateStmt, DropStmt, IndexStmt, ObjectType, ViewStmt};

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

/// Lint a single declarative schema file. Returns the list of violations,
/// or `Ok(())` if the script is purely declarative.
///
/// `source` is the label included in error messages (typically the schema
/// table name or the file path).
pub fn lint_declarative_schema(sql: &str, source: &str) -> Result<(), Vec<LintError>> {
    let parsed = match pg_query::parse(sql) {
        Ok(p) => p,
        Err(e) => {
            return Err(vec![LintError {
                line: 1,
                column: 1,
                severity: LintSeverity::Error,
                message: format!("SQL parse failed: {e}"),
                source: source.to_string(),
            }]);
        },
    };

    let line_index = LineIndex::new(sql);
    let mut errors: Vec<LintError> = Vec::new();
    let mut tables: Vec<TableDef> = Vec::new();

    for raw in &parsed.protobuf.stmts {
        let location = stmt_start_offset(sql, raw.stmt_location.max(0) as usize);
        let (line, col) = line_index.position(location);

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
                if let Some(warn) =
                    warn_create_table_missing_if_not_exists(create, line, col, source)
                {
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
                        source: source.to_string(),
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
                        source: source.to_string(),
                    });
                }
            },
        }
    }

    for raw in &parsed.protobuf.stmts {
        let Some(stmt) = raw.stmt.as_ref() else {
            continue;
        };
        let Some(node) = stmt.node.as_ref() else {
            continue;
        };
        let location = stmt_start_offset(sql, raw.stmt_location.max(0) as usize);
        let (line, col) = line_index.position(location);

        match node {
            Node::IndexStmt(idx) => {
                check_index_columns(idx, &tables, line, col, source, &mut errors);
            },
            Node::ViewStmt(view) => {
                check_view_columns(view, &tables, line, col, source, &mut errors);
            },
            _ => {},
        }
    }

    if errors.iter().any(|e| e.severity == LintSeverity::Error) {
        return Err(errors);
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct TableDef {
    name: String,
    columns: Vec<String>,
}

fn collect_create_stmt(create: &CreateStmt) -> Option<TableDef> {
    let relation = create.relation.as_ref()?;
    let name = relation.relname.clone();
    if name.is_empty() {
        return None;
    }
    let mut columns = Vec::new();
    for elt in &create.table_elts {
        if let Some(Node::ColumnDef(cd)) = elt.node.as_ref() {
            push_column(&mut columns, cd);
        }
    }
    Some(TableDef { name, columns })
}

fn push_column(columns: &mut Vec<String>, cd: &ColumnDef) {
    if !cd.colname.is_empty() {
        columns.push(cd.colname.clone());
    }
}

fn warn_create_table_missing_if_not_exists(
    create: &CreateStmt,
    line: u32,
    col: u32,
    source: &str,
) -> Option<LintError> {
    if create.if_not_exists {
        return None;
    }
    Some(LintError {
        line,
        column: col,
        severity: LintSeverity::Warning,
        message: "CREATE TABLE without IF NOT EXISTS — add IF NOT EXISTS for idempotency".into(),
        source: source.to_string(),
    })
}

fn imperative_reason(node: &Node) -> Option<&'static str> {
    Some(match node {
        Node::AlterTableStmt(_) => "ALTER TABLE",
        Node::DropStmt(drop) if is_safe_drop(drop) => return None,
        Node::DropStmt(_) => "DROP",
        Node::InsertStmt(_) => "INSERT",
        Node::UpdateStmt(_) => "UPDATE",
        Node::DeleteStmt(_) => "DELETE",
        Node::TruncateStmt(_) => "TRUNCATE",
        Node::GrantStmt(_) => "GRANT/REVOKE",
        Node::RenameStmt(_) => "RENAME",
        Node::DoStmt(_) => "DO $$ block",
        Node::SelectStmt(_) => "SELECT",
        Node::CopyStmt(_) => "COPY",
        Node::AlterDatabaseStmt(_)
        | Node::AlterDatabaseSetStmt(_)
        | Node::AlterRoleStmt(_)
        | Node::AlterRoleSetStmt(_)
        | Node::AlterOwnerStmt(_)
        | Node::AlterSeqStmt(_)
        | Node::AlterEnumStmt(_)
        | Node::AlterFunctionStmt(_)
        | Node::AlterObjectSchemaStmt(_)
        | Node::AlterDefaultPrivilegesStmt(_) => "ALTER",
        _ => return None,
    })
}

/// `DROP` of a stateless derived object — `VIEW`, `MATERIALIZED VIEW`,
/// `INDEX`, or `TRIGGER` — guarded by `IF EXISTS` is declarative-safe: the
/// object holds no data and is rebuilt from the same schema file on the next
/// statement. `DROP TABLE` / `DROP COLUMN` are not — they destroy data and
/// must move to a migration.
fn is_safe_drop(drop: &DropStmt) -> bool {
    if !drop.missing_ok {
        return false;
    }
    matches!(
        ObjectType::try_from(drop.remove_type),
        Ok(ObjectType::ObjectView
            | ObjectType::ObjectMatview
            | ObjectType::ObjectIndex
            | ObjectType::ObjectTrigger)
    )
}

fn find_table<'a>(tables: &'a [TableDef], name: &str) -> Option<&'a TableDef> {
    tables.iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

#[allow(clippy::too_many_arguments)]
fn check_index_columns(
    idx: &IndexStmt,
    tables: &[TableDef],
    line: u32,
    col: u32,
    source: &str,
    errors: &mut Vec<LintError>,
) {
    let Some(rel) = idx.relation.as_ref() else {
        return;
    };
    let Some(table) = find_table(tables, &rel.relname) else {
        return;
    };
    for param in &idx.index_params {
        let Some(Node::IndexElem(ie)) = param.node.as_ref() else {
            continue;
        };
        if ie.expr.is_some() {
            continue;
        }
        let column_name = &ie.name;
        if column_name.is_empty() {
            continue;
        }
        if !table
            .columns
            .iter()
            .any(|c| c.eq_ignore_ascii_case(column_name))
        {
            errors.push(LintError {
                line,
                column: col,
                severity: LintSeverity::Error,
                message: format!(
                    "unknown column `{}` on table `{}` (index `{}`) — declare the column in the \
                     same schema or move the index to a migration",
                    column_name, table.name, idx.idxname
                ),
                source: source.to_string(),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_view_columns(
    view: &ViewStmt,
    tables: &[TableDef],
    line: u32,
    col: u32,
    source: &str,
    errors: &mut Vec<LintError>,
) {
    let Some(query) = view.query.as_ref() else {
        return;
    };
    let Some(Node::SelectStmt(select)) = query.node.as_ref() else {
        return;
    };

    let mut alias_map: Vec<(String, String)> = Vec::new();
    let mut single_table: Option<String> = None;
    let mut from_count = 0usize;
    for f in &select.from_clause {
        if let Some(Node::RangeVar(rv)) = f.node.as_ref() {
            from_count += 1;
            if from_count == 1 {
                single_table = Some(rv.relname.clone());
            } else {
                single_table = None;
            }
            if let Some(alias) = rv.alias.as_ref() {
                if !alias.aliasname.is_empty() {
                    alias_map.push((alias.aliasname.clone(), rv.relname.clone()));
                }
            }
        } else {
            return;
        }
    }

    let view_name = view
        .view
        .as_ref()
        .map(|v| v.relname.clone())
        .unwrap_or_default();

    for target in &select.target_list {
        let Some(Node::ResTarget(rt)) = target.node.as_ref() else {
            continue;
        };
        let Some(val) = rt.val.as_ref() else {
            continue;
        };
        let Some(Node::ColumnRef(cref)) = val.node.as_ref() else {
            continue;
        };

        let parts: Vec<String> = cref
            .fields
            .iter()
            .filter_map(|f| match f.node.as_ref()? {
                Node::String(s) => Some(s.sval.clone()),
                _ => None,
            })
            .collect();

        if parts.iter().any(|p| p == "*") {
            continue;
        }

        let (table_ref, column_name) = match parts.as_slice() {
            [t, c] => (Some(t.clone()), c.clone()),
            [c] if from_count == 1 => (single_table.clone(), c.clone()),
            _ => continue,
        };
        let Some(table_ref) = table_ref else {
            continue;
        };

        let resolved_table = alias_map
            .iter()
            .find(|(a, _)| a.eq_ignore_ascii_case(&table_ref))
            .map_or(table_ref.as_str(), |(_, t)| t.as_str());

        let Some(table) = find_table(tables, resolved_table) else {
            continue;
        };
        if !table
            .columns
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&column_name))
        {
            errors.push(LintError {
                line,
                column: col,
                severity: LintSeverity::Error,
                message: format!(
                    "unknown column `{}` on table `{}` (view `{}`)",
                    column_name, table.name, view_name
                ),
                source: source.to_string(),
            });
        }
    }
}

fn stmt_start_offset(sql: &str, start: usize) -> usize {
    let bytes = sql.as_bytes();
    let mut i = start;
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            let mut depth = 1u32;
            while i + 1 < bytes.len() && depth > 0 {
                if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        break;
    }
    i
}

struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut line_starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }

    fn position(&self, byte_offset: usize) -> (u32, u32) {
        let line_idx = match self.line_starts.binary_search(&byte_offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line_start = self.line_starts[line_idx];
        let line = (line_idx as u32) + 1;
        let col = ((byte_offset - line_start) as u32) + 1;
        (line, col)
    }
}
