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

use pg_query::NodeEnum;
use pg_query::protobuf::node::Node;
use pg_query::protobuf::{
    AlterTableCmd, AlterTableStmt, AlterTableType, ColumnDef, ConstrType, Constraint, CreateStmt,
    DropBehavior, ObjectType, RangeVar,
};

/// Postgres truncates identifiers to `NAMEDATALEN - 1` bytes.
const MAX_IDENTIFIER_BYTES: usize = 63;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredForeignKey {
    /// The constrained table as a `to_regclass` argument (`"schema"."table"`).
    pub table: String,
    /// The constrained table as written, for diagnostics.
    pub source_table: String,
    pub columns: Vec<String>,
    /// The referenced table as a `to_regclass` argument.
    pub referenced_table: String,
    /// Empty when the declaration was `REFERENCES t` — the referenced primary
    /// key.
    pub referenced_columns: Vec<String>,
    pub constraint_name: String,
    /// `ALTER TABLE … ADD CONSTRAINT …` without a validation clause.
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitCreateTable {
    pub create_table_sql: String,
    pub foreign_keys: Vec<DeferredForeignKey>,
}

/// Split the foreign keys out of one parsed `CREATE TABLE`.
///
/// `original_sql` is returned untouched when the statement declares none.
pub(super) fn split_foreign_keys(
    original_sql: &str,
    create: &CreateStmt,
) -> Result<SplitCreateTable, String> {
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

    let create_table_sql = NodeEnum::CreateStmt(stripped)
        .deparse()
        .map_err(|e| format!("could not re-emit CREATE TABLE without its foreign keys: {e}"))?;

    let foreign_keys = constraints
        .into_iter()
        .map(|c| deferred_key(relation, c))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SplitCreateTable {
        create_table_sql,
        foreign_keys,
    })
}

/// Split the foreign keys out of a single `CREATE TABLE` given as text — the
/// seam the unit tests and diagnostics use; the installer works on the
/// already-parsed statement.
pub fn split_create_table_foreign_keys(sql: &str) -> Result<SplitCreateTable, String> {
    let parsed = pg_query::parse(sql).map_err(|e| format!("SQL parse failed: {e}"))?;
    let create = parsed
        .protobuf
        .stmts
        .iter()
        .find_map(|raw| match raw.stmt.as_ref()?.node.as_ref()? {
            Node::CreateStmt(create) => Some(create),
            _ => None,
        })
        .ok_or_else(|| "not a CREATE TABLE statement".to_owned())?;
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

/// Remove a column-level `REFERENCES` from a column definition, folding the
/// `DEFERRABLE` / `INITIALLY …` attributes the grammar emits as sibling nodes
/// into the key itself, and naming the constrained column in `fk_attrs`.
fn strip_column_references(cd: &ColumnDef) -> (ColumnDef, Vec<Constraint>) {
    let mut column = (*cd).clone();
    let mut kept = Vec::with_capacity(cd.constraints.len());
    let mut found = Vec::new();

    for node in &cd.constraints {
        let Some(Node::Constraint(c)) = node.node.as_ref() else {
            kept.push(node.clone());
            continue;
        };
        if is_foreign(c) {
            let mut key = (**c).clone();
            key.fk_attrs = vec![string_node(&cd.colname)];
            found.push(key);
            continue;
        }
        // Why: `REFERENCES t DEFERRABLE INITIALLY DEFERRED` parses as three
        // sibling constraints; Postgres attaches the attributes to the
        // preceding key in analysis, which is what the deferred ALTER needs.
        if let Some(previous) = found.last_mut()
            && fold_attribute(previous, c)
        {
            continue;
        }
        kept.push(node.clone());
    }

    column.constraints = kept;
    (column, found)
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

fn deferred_key(relation: &RangeVar, mut key: Constraint) -> Result<DeferredForeignKey, String> {
    let Some(referenced) = key.pktable.clone() else {
        return Err(format!(
            "foreign key on {} names no referenced table",
            relation.relname
        ));
    };
    let columns = string_values(&key.fk_attrs);
    let referenced_columns = string_values(&key.pk_attrs);
    if key.conname.is_empty() {
        key.conname = default_name(&relation.relname, &columns);
    }
    let constraint_name = key.conname.clone();

    let mut target = relation.clone();
    // Why: no ONLY — the key must reach partitions and inheritance children,
    // which is also what an inline declaration does.
    target.inh = true;

    let alter = AlterTableStmt {
        relation: Some(target),
        cmds: vec![pg_query::protobuf::Node {
            node: Some(Node::AlterTableCmd(Box::new(AlterTableCmd {
                subtype: AlterTableType::AtAddConstraint as i32,
                def: Some(Box::new(pg_query::protobuf::Node {
                    node: Some(Node::Constraint(Box::new(key))),
                })),
                // Why: the C side converts every enum field, and rejects the
                // protobuf default 0 — `Default` alone aborts the process.
                behavior: DropBehavior::DropRestrict as i32,
                ..AlterTableCmd::default()
            }))),
        }],
        objtype: ObjectType::ObjectTable as i32,
        missing_ok: false,
    };
    let sql = NodeEnum::AlterTableStmt(alter).deparse().map_err(|e| {
        format!(
            "could not emit the deferred foreign key on {}: {e}",
            relation.relname
        )
    })?;

    Ok(DeferredForeignKey {
        table: regclass_argument(relation),
        source_table: relation.relname.clone(),
        columns,
        referenced_table: regclass_argument(&referenced),
        referenced_columns,
        constraint_name,
        sql,
    })
}

/// The name Postgres itself would pick: `<table>_<col>[_<col>…]_fkey`, cut
/// to the identifier limit on a character boundary.
fn default_name(table: &str, columns: &[String]) -> String {
    let mut name = format!("{table}_{}_fkey", columns.join("_"));
    if name.len() > MAX_IDENTIFIER_BYTES {
        let mut cut = MAX_IDENTIFIER_BYTES;
        while !name.is_char_boundary(cut) {
            cut -= 1;
        }
        name.truncate(cut);
    }
    name
}

fn regclass_argument(rv: &RangeVar) -> String {
    let quote = |ident: &str| format!("\"{}\"", ident.replace('"', "\"\""));
    if rv.schemaname.is_empty() {
        quote(&rv.relname)
    } else {
        format!("{}.{}", quote(&rv.schemaname), quote(&rv.relname))
    }
}

fn string_node(value: &str) -> pg_query::protobuf::Node {
    pg_query::protobuf::Node {
        node: Some(Node::String(pg_query::protobuf::String {
            sval: value.to_owned(),
        })),
    }
}

fn string_values(nodes: &[pg_query::protobuf::Node]) -> Vec<String> {
    nodes
        .iter()
        .filter_map(|n| match n.node.as_ref()? {
            Node::String(s) => Some(s.sval.clone()),
            _ => None,
        })
        .collect()
}
