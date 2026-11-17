//! What one member's own SQL creates, read from its migrations in version
//! order and then its declarative schema, less the tables its retirements
//! name (and the triggers on them).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_extension::created_table_names;
use systemprompt_identifiers::ExtensionId;

use super::super::undeclared::qualify;
use super::objects::Created;
use super::statements::{is_table, object_name, top_level};
use super::{MigrationOwnershipError, label};
use crate::lifecycle::schema_member::SchemaMember;

pub(in crate::lifecycle::installation) fn retired_tables(
    member: &dyn SchemaMember,
) -> BTreeSet<String> {
    let mut retired = BTreeSet::new();
    for retirement in member.retirements() {
        let Ok(parsed) = pg_query::parse(&retirement.sql) else {
            continue;
        };
        for statement in top_level(&parsed) {
            if let pg_query::NodeEnum::DropStmt(drop) = statement
                && is_table(drop.remove_type)
            {
                retired.extend(drop.objects.iter().filter_map(object_name));
            }
        }
    }
    retired
}

pub(super) fn read_member(
    member: &dyn SchemaMember,
    id: &ExtensionId,
    findings: &mut Vec<MigrationOwnershipError>,
) -> (Created, Created, BTreeSet<String>) {
    let mut migrations: Vec<_> = member
        .migrations()
        .into_iter()
        .filter(|m| !m.tombstone)
        .collect();
    migrations.sort_by_key(|m| m.version);
    let mut created = Created::default();
    for migration in &migrations {
        if let Err(source) = created.read(migration.sql) {
            findings.push(MigrationOwnershipError::Parse {
                member: id.clone(),
                migration: label(migration),
                source,
            });
        }
    }
    for table in retired_tables(member) {
        let suffix = format!(" ON {table}");
        created
            .triggers
            .retain(|trigger| !trigger.ends_with(&suffix));
        created.tables.remove(&table);
        created.objects.remove(&table);
    }
    let mut declared = BTreeSet::new();
    let mut declared_objects = Created::default();
    for schema in member.schemas() {
        let read = created_table_names(&schema.sql).and_then(|tables| {
            created.read(&schema.sql)?;
            declared_objects.read(&schema.sql)?;
            Ok(tables)
        });
        match read {
            Ok(tables) => declared.extend(tables.iter().map(|table| qualify(table))),
            Err(source) => findings.push(MigrationOwnershipError::Parse {
                member: id.clone(),
                migration: String::from("schema"),
                source,
            }),
        }
    }
    (created, declared_objects, declared)
}
