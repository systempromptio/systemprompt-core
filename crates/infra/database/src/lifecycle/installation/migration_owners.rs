//! Refuses, before any database write, a migration that alters, drops,
//! truncates or renames a table another member owns.
//!
//! A table is owned by the member whose declarative schema or migration
//! creates it (the same `created_table_names` parse the purge and the
//! ownership records use), or, for a table no admitted member creates, by
//! the member `schema_table_owners` records for it. A migration that mutates
//! a table owned by a different member is refused with
//! [`MigrationOwnershipError::ForeignTable`], naming the member, the
//! statement kind and the table. A table no member creates and none is
//! recorded for is not judged: no member owns it. A `cross_extension_tables`
//! declaration admits the mutation, except an install member's mutation of a
//! core member's table. A foreign-key `REFERENCES`, a read and a data write
//! are not judged.
//!
//! Only migrations that will run are judged: a migration recorded as applied
//! is history, and a fresh database stamps its migrations without running
//! them. A `DO` block hides the tables it mutates from the parser, so an
//! install member's pending migration holding one is refused
//! ([`MigrationOwnershipError::Unreadable`]); a core member's is logged at
//! `warn` and passes, since core migrations ship in this tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use pg_query::NodeEnum;
use pg_query::protobuf::{ObjectType, RangeVar};
use systemprompt_extension::{Migration, StandingTag, created_table_names};
use systemprompt_identifiers::ExtensionId;
use tracing::warn;

use super::undeclared::qualify;
use crate::lifecycle::schema_member::SchemaMember;

/// The kind of statement a migration mutates a table with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableMutation {
    Alter,
    Drop,
    Truncate,
    Rename,
}

impl TableMutation {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Alter => "ALTER TABLE",
            Self::Drop => "DROP TABLE",
            Self::Truncate => "TRUNCATE",
            Self::Rename => "RENAME",
        }
    }
}

impl std::fmt::Display for TableMutation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// One table's owner as `schema_table_owners` records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedTable {
    pub table: String,
    pub owner: ExtensionId,
    pub standing: StandingTag,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationOwnershipError {
    #[error("member {member} migration {migration}: {statement} of table {table}, owned by {owner}")]
    ForeignTable {
        member: ExtensionId,
        migration: String,
        statement: TableMutation,
        table: String,
        owner: ExtensionId,
    },

    #[error(
        "install member {member} migration {migration}: a {statement} statement hides the tables \
         it mutates"
    )]
    Unreadable {
        member: ExtensionId,
        migration: String,
        statement: &'static str,
    },

    #[error("member {member} migration {migration} could not be parsed")]
    Parse {
        member: ExtensionId,
        migration: String,
        #[source]
        source: pg_query::Error,
    },
}

impl MigrationOwnershipError {
    #[must_use]
    pub const fn member(&self) -> &ExtensionId {
        match self {
            Self::ForeignTable { member, .. }
            | Self::Unreadable { member, .. }
            | Self::Parse { member, .. } => member,
        }
    }
}

#[derive(Debug, Clone)]
struct Owner {
    id: ExtensionId,
    standing: StandingTag,
}

pub fn check_migration_ownership(
    members: &[Arc<dyn SchemaMember>],
    recorded: &[RecordedTable],
    pending: &dyn Fn(&ExtensionId, &Migration) -> bool,
) -> Result<(), MigrationOwnershipError> {
    migration_ownership_findings(members, recorded, pending)
        .into_iter()
        .next()
        .map_or(Ok(()), Err)
}

#[must_use]
pub fn migration_ownership_findings(
    members: &[Arc<dyn SchemaMember>],
    recorded: &[RecordedTable],
    pending: &dyn Fn(&ExtensionId, &Migration) -> bool,
) -> Vec<MigrationOwnershipError> {
    let mut findings = Vec::new();
    let mut owners: BTreeMap<String, Owner> = BTreeMap::new();
    let mut own: BTreeMap<ExtensionId, BTreeSet<String>> = BTreeMap::new();
    for member in members {
        let id = ExtensionId::new(member.id());
        let created = own.entry(id.clone()).or_default();
        let sources = member
            .schemas()
            .into_iter()
            .map(|schema| (String::from("schema"), schema.sql))
            .chain(
                member
                    .migrations()
                    .into_iter()
                    .filter(|m| !m.tombstone)
                    .map(|m| (label(&m), m.sql.to_owned())),
            );
        for (migration, sql) in sources {
            match created_table_names(&sql) {
                Ok(tables) => created.extend(tables.iter().map(|table| qualify(table))),
                Err(source) => findings.push(MigrationOwnershipError::Parse {
                    member: id.clone(),
                    migration,
                    source,
                }),
            }
        }
        for table in created.iter() {
            owners.entry(table.clone()).or_insert_with(|| Owner {
                id: id.clone(),
                standing: member.standing(),
            });
        }
    }
    for record in recorded {
        owners.entry(qualify(&record.table)).or_insert_with(|| Owner {
            id: record.owner.clone(),
            standing: record.standing,
        });
    }

    for member in members {
        let id = ExtensionId::new(member.id());
        let standing = member.standing();
        let created = own.get(&id).cloned().unwrap_or_default();
        let cross: BTreeSet<String> = member
            .cross_extension_tables()
            .into_iter()
            .map(qualify)
            .collect();
        for migration in member.migrations().into_iter().filter(|m| !m.tombstone) {
            if !pending(&id, &migration) {
                continue;
            }
            let migration_label = label(&migration);
            let parsed = match pg_query::parse(migration.sql) {
                Ok(parsed) => parsed,
                Err(source) => {
                    findings.push(MigrationOwnershipError::Parse {
                        member: id.clone(),
                        migration: migration_label,
                        source,
                    });
                    continue;
                },
            };
            for statement in top_level(&parsed) {
                if matches!(statement, NodeEnum::DoStmt(_)) {
                    if standing == StandingTag::Install {
                        findings.push(MigrationOwnershipError::Unreadable {
                            member: id.clone(),
                            migration: migration_label.clone(),
                            statement: "DO",
                        });
                    } else {
                        warn!(
                            member = %id,
                            migration = %migration_label,
                            "A core migration's DO block is not judged for the tables it mutates"
                        );
                    }
                    continue;
                }
                for (kind, table) in mutations_of(statement) {
                    if created.contains(&table) {
                        continue;
                    }
                    let Some(owner) = owners.get(&table).filter(|owner| owner.id != id) else {
                        continue;
                    };
                    let admitted_cross = cross.contains(&table)
                        && !(standing == StandingTag::Install && owner.standing == StandingTag::Core);
                    if admitted_cross {
                        continue;
                    }
                    findings.push(MigrationOwnershipError::ForeignTable {
                        member: id.clone(),
                        migration: migration_label.clone(),
                        statement: kind,
                        table,
                        owner: owner.id.clone(),
                    });
                }
            }
        }
    }
    findings
}

fn label(migration: &Migration) -> String {
    format!("{:03}_{}", migration.version, migration.name)
}

fn top_level(parsed: &pg_query::ParseResult) -> impl Iterator<Item = &NodeEnum> {
    parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| raw.stmt.as_ref().and_then(|s| s.node.as_ref()))
}

fn relation_name(relation: &RangeVar) -> Option<String> {
    if relation.relname.is_empty() {
        return None;
    }
    Some(if relation.schemaname.is_empty() {
        qualify(&relation.relname)
    } else {
        format!("{}.{}", relation.schemaname, relation.relname)
    })
}

fn is_table(object: i32) -> bool {
    matches!(
        ObjectType::try_from(object),
        Ok(ObjectType::ObjectTable | ObjectType::ObjectForeignTable)
    )
}

fn mutations_of(statement: &NodeEnum) -> Vec<(TableMutation, String)> {
    let named = |kind, relation: Option<&RangeVar>| {
        relation
            .and_then(relation_name)
            .map(|table| (kind, table))
            .into_iter()
            .collect::<Vec<_>>()
    };
    match statement {
        NodeEnum::AlterTableStmt(alter) if is_table(alter.objtype) => {
            named(TableMutation::Alter, alter.relation.as_ref())
        },
        NodeEnum::RenameStmt(rename)
            if matches!(
                ObjectType::try_from(rename.rename_type),
                Ok(ObjectType::ObjectTable
                    | ObjectType::ObjectColumn
                    | ObjectType::ObjectTabconstraint)
            ) =>
        {
            named(TableMutation::Rename, rename.relation.as_ref())
        },
        NodeEnum::TruncateStmt(truncate) => truncate
            .relations
            .iter()
            .filter_map(|node| match node.node.as_ref()? {
                NodeEnum::RangeVar(relation) => relation_name(relation),
                _ => None,
            })
            .map(|table| (TableMutation::Truncate, table))
            .collect(),
        NodeEnum::DropStmt(drop) if is_table(drop.remove_type) => drop
            .objects
            .iter()
            .filter_map(dropped_table)
            .map(|table| (TableMutation::Drop, table))
            .collect(),
        _ => Vec::new(),
    }
}

fn dropped_table(object: &pg_query::protobuf::Node) -> Option<String> {
    let NodeEnum::List(list) = object.node.as_ref()? else {
        return None;
    };
    let parts: Vec<&str> = list
        .items
        .iter()
        .filter_map(|item| match item.node.as_ref()? {
            NodeEnum::String(s) => Some(s.sval.as_str()),
            _ => None,
        })
        .collect();
    match parts.as_slice() {
        [table] => Some(qualify(table)),
        [schema, table] => Some(format!("{schema}.{table}")),
        _ => None,
    }
}
