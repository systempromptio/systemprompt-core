//! Re-emission of a stripped `FOREIGN KEY` as `ALTER TABLE … ADD CONSTRAINT`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pg_query::NodeEnum;
use pg_query::protobuf::node::Node;
use pg_query::protobuf::{
    AlterTableCmd, AlterTableStmt, AlterTableType, Constraint, DropBehavior, ObjectType, RangeVar,
};

use super::{DeferredForeignKey, FkDeferralError};

// Why: Postgres truncates identifiers to `NAMEDATALEN - 1` bytes.
const MAX_IDENTIFIER_BYTES: usize = 63;

pub(super) fn deferred_key(
    relation: &RangeVar,
    mut key: Constraint,
) -> Result<DeferredForeignKey, FkDeferralError> {
    let Some(referenced) = key.pktable.clone() else {
        return Err(FkDeferralError::NoReferencedTable {
            table: relation.relname.clone(),
        });
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
    let sql = NodeEnum::AlterTableStmt(alter)
        .deparse()
        .map_err(|source| FkDeferralError::DeparseKey {
            table: relation.relname.clone(),
            source,
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

pub(super) fn string_node(value: &str) -> pg_query::protobuf::Node {
    pg_query::protobuf::Node {
        node: Some(Node::String(pg_query::protobuf::String {
            sval: value.to_owned(),
        })),
    }
}

pub(super) fn string_values(nodes: &[pg_query::protobuf::Node]) -> Vec<String> {
    nodes
        .iter()
        .filter_map(|n| match n.node.as_ref()? {
            Node::String(s) => Some(s.sval.clone()),
            _ => None,
        })
        .collect()
}
