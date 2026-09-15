//! Split `FOREIGN KEY` constraints out of a declarative `CREATE TABLE`.
//!
//! The installer creates every table in the structural phase, before any
//! migration or `CREATE INDEX` has run. A foreign key needs a unique index on
//! the referenced columns at the moment it is created, and on an existing
//! database that index may only arrive through a migration — so a key
//! declared inline installs on a fresh database and fails on an upgraded one
//! with "no unique constraint matching given keys". The table is therefore
//! created without its foreign keys, and each key is re-emitted as an
//! `ALTER TABLE … ADD CONSTRAINT` the installer runs after every extension's
//! migrations and dependent DDL, exactly as `pg_dump` orders a schema.
//!
//! A statement that declares no foreign key is returned verbatim, so error
//! messages for such tables still quote the author's text. A statement that
//! does is re-emitted through `pg_query`'s deparser, which preserves
//! `IF NOT EXISTS`, column defaults, `CHECK`, `GENERATED`, identity, `UNIQUE`
//! and `PRIMARY KEY` clauses; only comments and formatting are lost.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod emit;

use pg_query::NodeEnum;
use pg_query::protobuf::node::Node;
use pg_query::protobuf::{ColumnDef, ConstrType, Constraint, CreateStmt};
use thiserror::Error;

use emit::{deferred_key, string_node};

#[derive(Debug, Error)]
pub enum FkDeferralError {
    #[error("SQL parse failed: {0}")]
    Parse(#[source] pg_query::Error),
    #[error("not a CREATE TABLE statement")]
    NotCreateTable,
    #[error("could not re-emit CREATE TABLE {table} without its foreign keys: {source}")]
    DeparseTable {
        table: String,
        #[source]
        source: pg_query::Error,
    },
    #[error("could not emit the deferred foreign key on {table}: {source}")]
    DeparseKey {
        table: String,
        #[source]
        source: pg_query::Error,
    },
    #[error("foreign key on {table} names no referenced table")]
    NoReferencedTable { table: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredForeignKey {
    pub table: String,
    pub source_table: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub constraint_name: String,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitCreateTable {
    pub create_table_sql: String,
    pub foreign_keys: Vec<DeferredForeignKey>,
}

pub(super) fn split_foreign_keys(
    original_sql: &str,
    create: &CreateStmt,
) -> Result<SplitCreateTable, FkDeferralError> {
    let Some(relation) = create.relation.as_ref() else {
        return Ok(verbatim(original_sql));
    };

    let mut stripped = create.clone();
    let mut constraints: Vec<Constraint> = Vec::new();
    stripped.table_elts = create
        .table_elts
        .iter()
        .filter_map(|elt| match elt.node.as_ref() {
            Some(Node::Constraint(c)) if is_foreign(c) => {
                constraints.push((**c).clone());
                None
            },
            Some(Node::ColumnDef(cd)) => {
                let (column, mut found) = strip_column_references(cd);
                constraints.append(&mut found);
                Some(pg_query::protobuf::Node {
                    node: Some(Node::ColumnDef(Box::new(column))),
                })
            },
            _ => Some(elt.clone()),
        })
        .collect();

    if constraints.is_empty() {
        return Ok(verbatim(original_sql));
    }

    let create_table_sql = NodeEnum::CreateStmt(stripped).deparse().map_err(|source| {
        FkDeferralError::DeparseTable {
            table: relation.relname.clone(),
            source,
        }
    })?;

    let foreign_keys = constraints
        .into_iter()
        .map(|c| deferred_key(relation, c))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SplitCreateTable {
        create_table_sql,
        foreign_keys,
    })
}

pub fn split_create_table_foreign_keys(sql: &str) -> Result<SplitCreateTable, FkDeferralError> {
    let parsed = pg_query::parse(sql).map_err(FkDeferralError::Parse)?;
    let create = parsed
        .protobuf
        .stmts
        .iter()
        .find_map(|raw| match raw.stmt.as_ref()?.node.as_ref()? {
            Node::CreateStmt(create) => Some(create),
            _ => None,
        })
        .ok_or(FkDeferralError::NotCreateTable)?;
    split_foreign_keys(sql, create)
}

fn verbatim(original_sql: &str) -> SplitCreateTable {
    SplitCreateTable {
        create_table_sql: original_sql.to_owned(),
        foreign_keys: Vec::new(),
    }
}

fn is_foreign(c: &Constraint) -> bool {
    ConstrType::try_from(c.contype) == Ok(ConstrType::ConstrForeign)
}

fn strip_column_references(cd: &ColumnDef) -> (ColumnDef, Vec<Constraint>) {
    let mut column = (*cd).clone();
    let mut kept = Vec::with_capacity(cd.constraints.len());
    let mut found = Vec::new();
    let mut last_primary_is_foreign = false;

    for node in &cd.constraints {
        let Some(Node::Constraint(c)) = node.node.as_ref() else {
            kept.push(node.clone());
            continue;
        };
        if is_foreign(c) {
            let mut key = (**c).clone();
            key.fk_attrs = vec![string_node(&cd.colname)];
            found.push(key);
            last_primary_is_foreign = true;
            continue;
        }
        // Why: `REFERENCES t DEFERRABLE INITIALLY DEFERRED` parses as three
        // sibling constraints; Postgres attaches the attributes to the last
        // key-like constraint (`lastprimarycon`: PRIMARY KEY, UNIQUE, FOREIGN
        // KEY or EXCLUDE) on the column, so an attribute is folded into the
        // deferred key only when that is the foreign key.
        if last_primary_is_foreign
            && let Some(previous) = found.last_mut()
            && fold_attribute(previous, c)
        {
            continue;
        }
        if is_key_like(c) {
            last_primary_is_foreign = false;
        }
        kept.push(node.clone());
    }

    column.constraints = kept;
    (column, found)
}

fn is_key_like(c: &Constraint) -> bool {
    matches!(
        ConstrType::try_from(c.contype),
        Ok(ConstrType::ConstrPrimary | ConstrType::ConstrUnique | ConstrType::ConstrExclusion)
    )
}

fn fold_attribute(key: &mut Constraint, attribute: &Constraint) -> bool {
    match ConstrType::try_from(attribute.contype) {
        Ok(ConstrType::ConstrAttrDeferrable) => key.deferrable = true,
        Ok(ConstrType::ConstrAttrNotDeferrable) => key.deferrable = false,
        Ok(ConstrType::ConstrAttrDeferred) => {
            key.deferrable = true;
            key.initdeferred = true;
        },
        Ok(ConstrType::ConstrAttrImmediate) => key.initdeferred = false,
        _ => return false,
    }
    true
}
