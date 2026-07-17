//! In-input table graph and `(table, column)` resolution for `CREATE INDEX`
//! and `CREATE VIEW` statements.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::protobuf::node::Node;
use pg_query::protobuf::{ColumnDef, CreateStmt, IndexStmt, ViewStmt};

use super::location::StmtLoc;
use super::{LintError, LintSeverity};

#[derive(Debug, Clone)]
pub(super) struct TableDef {
    name: String,
    columns: Vec<String>,
}

impl TableDef {
    pub(super) fn name(&self) -> &str {
        &self.name
    }
}

pub(super) fn collect_create_stmt(create: &CreateStmt) -> Option<TableDef> {
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

fn find_table<'a>(tables: &'a [TableDef], name: &str) -> Option<&'a TableDef> {
    tables.iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

pub(super) fn check_index_columns(
    idx: &IndexStmt,
    tables: &[TableDef],
    loc: &StmtLoc<'_>,
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
                line: loc.line,
                column: loc.col,
                severity: LintSeverity::Error,
                message: format!(
                    "unknown column `{}` on table `{}` (index `{}`) — declare the column in the \
                     same schema or move the index to a migration",
                    column_name, table.name, idx.idxname
                ),
                source: loc.source.to_owned(),
            });
        }
    }
}

pub(super) fn check_view_columns(
    view: &ViewStmt,
    tables: &[TableDef],
    loc: &StmtLoc<'_>,
    errors: &mut Vec<LintError>,
) {
    let Some(query) = view.query.as_ref() else {
        return;
    };
    let Some(Node::SelectStmt(select)) = query.node.as_ref() else {
        return;
    };
    let Some(view_from) = analyze_view_from(select) else {
        return;
    };
    let view_name = view
        .view
        .as_ref()
        .map(|v| v.relname.clone())
        .unwrap_or_default();

    errors.extend(check_view_targets(
        select, &view_from, tables, loc, &view_name,
    ));
}

/// Built only for a `FROM` of plain table references; joins and subqueries
/// yield `None` and are deliberately left unresolved.
struct ViewFrom {
    alias_map: Vec<(String, String)>,
    single_table: Option<String>,
    from_count: usize,
}

fn analyze_view_from(select: &pg_query::protobuf::SelectStmt) -> Option<ViewFrom> {
    let mut alias_map: Vec<(String, String)> = Vec::new();
    let mut single_table: Option<String> = None;
    let mut from_count = 0usize;
    for f in &select.from_clause {
        let Some(Node::RangeVar(rv)) = f.node.as_ref() else {
            return None;
        };
        from_count += 1;
        single_table = if from_count == 1 {
            Some(rv.relname.clone())
        } else {
            None
        };
        if let Some(alias) = rv.alias.as_ref()
            && !alias.aliasname.is_empty()
        {
            alias_map.push((alias.aliasname.clone(), rv.relname.clone()));
        }
    }
    Some(ViewFrom {
        alias_map,
        single_table,
        from_count,
    })
}

fn check_view_targets(
    select: &pg_query::protobuf::SelectStmt,
    view_from: &ViewFrom,
    tables: &[TableDef],
    loc: &StmtLoc<'_>,
    view_name: &str,
) -> Vec<LintError> {
    let mut errors: Vec<LintError> = Vec::new();

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
            [c] if view_from.from_count == 1 => (view_from.single_table.clone(), c.clone()),
            _ => continue,
        };
        let Some(table_ref) = table_ref else {
            continue;
        };

        let resolved_table = view_from
            .alias_map
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
                line: loc.line,
                column: loc.col,
                severity: LintSeverity::Error,
                message: format!(
                    "unknown column `{}` on table `{}` (view `{}`)",
                    column_name, table.name, view_name
                ),
                source: loc.source.to_owned(),
            });
        }
    }

    errors
}
