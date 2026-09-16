//! Referenced-uniqueness check for foreign keys declared in the same
//! extension.
//!
//! The installer applies a declarative foreign key after every migration and
//! index, so a key can be *created* on any database once the referenced
//! uniqueness exists — but on a fresh database nothing runs except the
//! declarative schema itself. A referenced table that only gains its unique
//! index from a `CREATE UNIQUE INDEX` still installs (indexes precede the
//! deferred keys); one that only gains it from a migration does not, because
//! migrations are stamped, not run, on a fresh database. The rule therefore
//! asks for the uniqueness to be visible where a reader looks for it: a
//! `PRIMARY KEY` or `UNIQUE` on the referenced `CREATE TABLE`. Tables not
//! declared in the same extension are skipped, as every other cross-extension
//! reference is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::protobuf::node::Node;
use pg_query::protobuf::{ConstrType, Constraint, CreateStmt};

use super::columns::{TableDef, find_table, string_values};
use super::location::StmtLoc;
use super::{LintError, LintSeverity};

pub(super) fn check_foreign_keys(
    create: &CreateStmt,
    tables: &[TableDef],
    loc: &StmtLoc<'_>,
    errors: &mut Vec<LintError>,
) {
    let Some(relation) = create.relation.as_ref() else {
        return;
    };
    for (columns, key) in declared_keys(create) {
        let Some(pktable) = key.pktable.as_ref() else {
            continue;
        };
        let Some(referenced) = find_table(tables, &pktable.schemaname, &pktable.relname) else {
            continue;
        };
        let referenced_columns = if key.pk_attrs.is_empty() {
            if let Some(pk) = referenced.primary_key() {
                pk.to_vec()
            } else {
                errors.push(error(
                    loc,
                    format!(
                        "foreign key on `{}`({}) references `{}` without naming columns, \
                         and `{}` declares no PRIMARY KEY",
                        relation.relname,
                        columns.join(", "),
                        referenced.name(),
                        referenced.name(),
                    ),
                ));
                continue;
            }
        } else {
            string_values(&key.pk_attrs)
        };
        if referenced.has_unique_key(&referenced_columns) {
            continue;
        }
        errors.push(error(
            loc,
            format!(
                "foreign key on `{}`({}) references `{}`({}) but `{}` declares no PRIMARY KEY \
                 or UNIQUE constraint on exactly those columns — declare it on the referenced \
                 CREATE TABLE (project rule: a CREATE UNIQUE INDEX installs but is not accepted \
                 by the linter, and a migration alone does not satisfy a fresh install)",
                relation.relname,
                columns.join(", "),
                referenced.name(),
                referenced_columns.join(", "),
                referenced.name(),
            ),
        ));
    }
}

fn declared_keys(create: &CreateStmt) -> Vec<(Vec<String>, &Constraint)> {
    let mut keys = Vec::new();
    for elt in &create.table_elts {
        match elt.node.as_ref() {
            Some(Node::Constraint(c)) if is_foreign(c) => {
                keys.push((string_values(&c.fk_attrs), c.as_ref()));
            },
            Some(Node::ColumnDef(cd)) => {
                for node in &cd.constraints {
                    if let Some(Node::Constraint(c)) = node.node.as_ref()
                        && is_foreign(c)
                    {
                        keys.push((vec![cd.colname.clone()], c.as_ref()));
                    }
                }
            },
            _ => {},
        }
    }
    keys
}

fn is_foreign(c: &Constraint) -> bool {
    ConstrType::try_from(c.contype) == Ok(ConstrType::ConstrForeign)
}

fn error(loc: &StmtLoc<'_>, message: String) -> LintError {
    LintError {
        line: loc.line,
        column: loc.col,
        severity: LintSeverity::Error,
        message,
        source: loc.source.to_owned(),
    }
}
