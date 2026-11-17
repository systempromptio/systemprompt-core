//! Who owns each table. A record in `schema_table_owners` and the kernel
//! ledgers come first, then a member's declarative schema, then the tables a
//! member's migrations create and do not drop again; the first claim in that
//! order is the owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_extension::{StandingTag, created_table_names};
use systemprompt_identifiers::ExtensionId;

use super::super::undeclared::qualify;
use super::objects::Created;
use super::{
    KERNEL_LEDGER_OWNER, KERNEL_LEDGERS, MigrationMembers, MigrationOwnershipError, RecordedTable,
    label,
};
use crate::lifecycle::schema_member::SchemaMember;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    Record,
    Schema,
    Migration,
}

#[derive(Debug, Clone)]
pub(super) struct Claim {
    rank: Rank,
    pub(super) id: ExtensionId,
    pub(super) standing: StandingTag,
}

#[derive(Default)]
pub(super) struct Ownership {
    tables: BTreeMap<String, Vec<Claim>>,
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

    pub(super) fn creators<'a>(
        &'a self,
        pick: impl Fn(&'a Created) -> &'a BTreeSet<String> + 'a,
        name: &'a str,
    ) -> impl Iterator<Item = &'a ExtensionId> + 'a {
        self.created
            .iter()
            .filter(move |(_, created)| pick(created).contains(name))
            .map(|(id, _)| id)
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

fn read_member(
    member: &dyn SchemaMember,
    id: &ExtensionId,
    findings: &mut Vec<MigrationOwnershipError>,
) -> (Created, BTreeSet<String>) {
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
    let mut declared = BTreeSet::new();
    for schema in member.schemas() {
        let read = created_table_names(&schema.sql).and_then(|tables| {
            created.read(&schema.sql)?;
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
    (created, declared)
}

pub(super) fn ownership(
    members: MigrationMembers<'_>,
    recorded: &[RecordedTable],
    findings: &mut Vec<MigrationOwnershipError>,
) -> Ownership {
    let mut ownership = Ownership::default();
    for record in recorded {
        ownership.claim(
            qualify(&record.table),
            Claim {
                rank: Rank::Record,
                id: record.owner.clone(),
                standing: record.standing,
            },
        );
    }
    for ledger in KERNEL_LEDGERS {
        ownership.claim(
            ledger.to_owned(),
            Claim {
                rank: Rank::Record,
                id: ExtensionId::new(KERNEL_LEDGER_OWNER),
                standing: StandingTag::Core,
            },
        );
    }
    let mut declared_by = Vec::new();
    for member in members.judged.iter().chain(members.held_back) {
        let id = ExtensionId::new(member.id());
        let (created, declared) = read_member(member.as_ref(), &id, findings);
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
        ownership.created.insert(id, created);
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
