//! Who owns each table. A record in `schema_table_owners` and the kernel
//! ledgers come first, then a member's declarative schema, then the tables a
//! member's migrations create and do not drop again; the first claim in that
//! order is the owner.
//!
//! A member's retirements may name a table (`DROP TABLE IF EXISTS`): the
//! member says its migration chain has retired that table. The table is then
//! not the member's, neither by its migrations nor by a record naming it,
//! unless its declarative schema still creates it. A table no member claims
//! has no owner, and a mutation of it is not judged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_extension::{StandingTag, created_table_names};
use systemprompt_identifiers::ExtensionId;

use super::super::undeclared::qualify;
use super::objects::Created;
use super::statements::{is_table, object_name, top_level};
use super::{
    KERNEL_LEDGER_OWNER, KERNEL_LEDGERS, MigrationMembers, MigrationOwnershipError, RecordedTable,
    label,
};
use crate::lifecycle::schema_member::SchemaMember;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Rank {
    Record,
    Schema,
    Migration,
}

#[derive(Debug, Clone)]
pub(super) struct Claim {
    pub(super) rank: Rank,
    pub(super) id: ExtensionId,
    pub(super) standing: StandingTag,
}

#[derive(Default)]
pub(super) struct Ownership {
    tables: BTreeMap<String, Vec<Claim>>,
    pub(super) objects: BTreeMap<String, Vec<Claim>>,
    pub(super) indexes: BTreeMap<String, Vec<Claim>>,
    pub(super) created: BTreeMap<ExtensionId, Created>,
}

impl Ownership {
    fn claim(&mut self, table: String, claim: Claim) {
        let claims = self.tables.entry(table).or_default();
        if !claims
            .iter()
            .any(|held| held.id == claim.id && held.rank <= claim.rank)
        {
            claims.push(claim);
            claims.sort_by_key(|held| held.rank);
        }
    }

    pub(super) fn owner(&self, table: &str) -> Option<&Claim> {
        self.tables.get(table).and_then(|claims| claims.first())
    }

    pub(super) fn knows(&self, member: &str) -> bool {
        self.created.keys().any(|id| id.as_str() == member)
            || self
                .tables
                .values()
                .flatten()
                .any(|claim| claim.id.as_str() == member)
    }

    pub(super) fn held_by_another(&self, schema: &str, member: &ExtensionId) -> bool {
        let prefix = format!("{schema}.");
        let another_table = self.tables.iter().any(|(table, claims)| {
            table.starts_with(&prefix) && claims.first().is_some_and(|owner| owner.id != *member)
        });
        let another_creates = self.created.iter().any(|(id, created)| {
            id != member
                && (created.schemas.contains(schema)
                    || created
                        .objects
                        .iter()
                        .any(|object| object.starts_with(&prefix)))
        });
        another_table || another_creates
    }

    pub(super) fn foreign_creates(
        &self,
        id: &ExtensionId,
        source: &str,
        sql: &str,
    ) -> Vec<MigrationOwnershipError> {
        let Ok(tables) = created_table_names(sql) else {
            return Vec::new();
        };
        tables
            .iter()
            .map(|table| qualify(table))
            .filter_map(|table| {
                let owner = self.owner(&table).filter(|owner| owner.id != *id)?;
                Some(MigrationOwnershipError::SharedTable {
                    member: id.clone(),
                    source_sql: source.to_owned(),
                    table,
                    owner: owner.id.clone(),
                })
            })
            .collect()
    }
}

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

fn read_member(
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

impl Ownership {
    fn claim_records(&mut self, members: MigrationMembers<'_>, recorded: &[RecordedTable]) {
        let retired: BTreeMap<ExtensionId, BTreeSet<String>> = members
            .judged
            .iter()
            .chain(members.held_back)
            .map(|member| {
                (
                    ExtensionId::new(member.id()),
                    retired_tables(member.as_ref()),
                )
            })
            .collect();
        for record in recorded.iter().filter(|record| {
            retired
                .get(&record.owner)
                .is_none_or(|tables| !tables.contains(&qualify(&record.table)))
        }) {
            self.claim(
                qualify(&record.table),
                Claim {
                    rank: Rank::Record,
                    id: record.owner.clone(),
                    standing: record.standing,
                },
            );
        }
        for ledger in KERNEL_LEDGERS {
            self.claim(
                ledger.to_owned(),
                Claim {
                    rank: Rank::Record,
                    id: ExtensionId::new(KERNEL_LEDGER_OWNER),
                    standing: StandingTag::Core,
                },
            );
        }
    }
}

pub(super) fn ownership(
    members: MigrationMembers<'_>,
    recorded: &[RecordedTable],
    findings: &mut Vec<MigrationOwnershipError>,
) -> Ownership {
    let mut ownership = Ownership::default();
    ownership.claim_records(members, recorded);
    let mut declared_by = Vec::new();
    let mut declared_objects_by = Vec::new();
    for member in members.judged.iter().chain(members.held_back) {
        let id = ExtensionId::new(member.id());
        let (created, declared_objects, declared) = read_member(member.as_ref(), &id, findings);
        ownership.claim_created(&id, member.standing(), &created, &declared_objects);
        for table in &created.tables {
            let rank = if declared.contains(table) {
                Rank::Schema
            } else {
                Rank::Migration
            };
            ownership.claim(
                qualify(table),
                Claim {
                    rank,
                    id: id.clone(),
                    standing: member.standing(),
                },
            );
        }
        declared_by.push((id.clone(), declared));
        if member.standing() == StandingTag::Install {
            declared_objects_by.push((id.clone(), Ownership::schema_creates(member.as_ref())));
        }
        ownership.created.insert(id, created);
    }
    for (id, schema_created) in declared_objects_by {
        findings.extend(ownership.schema_object_findings(&id, &schema_created));
    }
    for (id, declared) in declared_by {
        for table in declared {
            if let Some(owner) = ownership.owner(&table).filter(|owner| owner.id != id) {
                findings.push(MigrationOwnershipError::SharedTable {
                    member: id.clone(),
                    source_sql: String::from("schema"),
                    table,
                    owner: owner.id.clone(),
                });
            }
        }
    }
    ownership
}
