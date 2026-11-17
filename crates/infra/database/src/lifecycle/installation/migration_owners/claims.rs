//! Who owns each table. A record in `schema_table_owners` and the kernel
//! ledgers come first; then a core member before an install member; then a
//! member's declarative schema before the tables a member's migrations create
//! and do not drop again; then composition order. The first claim in that
//! order is the owner. Every member of the composition claims, a held-back
//! member included; a disabled member, absent from the composition, claims
//! through its records.
//!
//! A record is a claim only while it is live (see `record_claims`).
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
use super::member_reads::{read_member, retired_tables};
use super::objects::Created;
use super::record_claims::{MemberRead, Stale};
use super::{MigrationMembers, MigrationOwnershipError, Recorded};

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
    pub(super) position: usize,
}

impl Claim {
    pub(super) fn held_by(&self, id: &ExtensionId, standing: StandingTag) -> bool {
        self.id == *id && !(standing == StandingTag::Install && self.standing == StandingTag::Core)
    }

    const fn precedence(&self) -> (bool, u8, Rank, usize) {
        let standing = match self.standing {
            StandingTag::Core => 0,
            StandingTag::Install => 1,
        };
        (
            !matches!(self.rank, Rank::Record),
            standing,
            self.rank,
            self.position,
        )
    }
}

pub(super) fn push_claim(claims: &mut BTreeMap<String, Vec<Claim>>, name: String, claim: Claim) {
    let claims = claims.entry(name).or_default();
    if claims
        .iter()
        .any(|held| held.id == claim.id && held.rank <= claim.rank)
    {
        return;
    }
    claims.push(claim);
    claims.sort_by_key(Claim::precedence);
}

#[derive(Default)]
pub(super) struct Ownership {
    pub(super) tables: BTreeMap<String, Vec<Claim>>,
    pub(super) objects: BTreeMap<String, Vec<Claim>>,
    pub(super) indexes: BTreeMap<String, Vec<Claim>>,
    pub(super) triggers: BTreeMap<String, Vec<Claim>>,
    pub(super) created: BTreeMap<ExtensionId, Created>,
    pub(super) judged: BTreeSet<ExtensionId>,
    pub(super) stale: Stale,
}

impl Ownership {
    pub(super) fn claim(&mut self, table: String, claim: Claim) {
        push_claim(&mut self.tables, table, claim);
    }

    pub(super) fn unjudged_record(&self, claim: &Claim) -> bool {
        claim.rank == Rank::Record
            && claim.standing == StandingTag::Install
            && !self.judged.contains(&claim.id)
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
        standing: StandingTag,
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
                let owner = self
                    .owner(&table)
                    .filter(|owner| !owner.held_by(id, standing))?;
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

impl Ownership {
    fn declared_findings(
        &self,
        read: &MemberRead,
        declared_objects: &Created,
        declared: &BTreeSet<String>,
    ) -> Vec<MigrationOwnershipError> {
        let mut findings = Vec::new();
        if read.standing == StandingTag::Core {
            findings.extend(
                self.foreign_object_creates(&read.id, read.standing, declared_objects)
                    .into_iter()
                    .filter(|refused| refused.3)
                    .map(
                        |(statement, object, owner, _)| MigrationOwnershipError::ForeignObject {
                            member: read.id.clone(),
                            migration: String::from("schema"),
                            statement,
                            object,
                            owner,
                        },
                    ),
            );
        }
        findings.extend(declared.iter().filter_map(|table| {
            let owner = self
                .owner(table)
                .filter(|owner| !owner.held_by(&read.id, read.standing))?;
            Some(MigrationOwnershipError::SharedTable {
                member: read.id.clone(),
                source_sql: String::from("schema"),
                table: table.clone(),
                owner: owner.id.clone(),
            })
        }));
        findings
    }
}

pub(super) fn ownership(
    members: MigrationMembers<'_>,
    recorded: &Recorded<'_>,
    findings: &mut Vec<MigrationOwnershipError>,
) -> Ownership {
    let mut ownership = Ownership::default();
    let mut reads = Vec::new();
    let mut declarations = Vec::new();
    for (position, member) in members.all().enumerate() {
        let id = ExtensionId::new(member.id());
        let judged = members.is_judged(member.as_ref());
        let mut read_findings = Vec::new();
        let (created, declared_objects, declared) =
            read_member(member.as_ref(), &id, &mut read_findings);
        if judged {
            findings.extend(read_findings);
        }
        declarations.push((position + 1, declared_objects, declared));
        if judged {
            ownership.judged.insert(id.clone());
        }
        reads.push(MemberRead {
            id,
            standing: member.standing(),
            judged,
            tombstoned: member.migrations().iter().any(|m| m.tombstone),
            retired: retired_tables(member.as_ref()),
            created,
        });
    }
    ownership.claim_records(members, recorded, &reads);
    for (read, (position, declared_objects, declared)) in reads.iter().zip(&declarations) {
        ownership.claim_created(
            &read.id,
            (read.standing, *position),
            &read.created,
            declared_objects,
        );
        for table in &read.created.tables {
            let rank = if declared.contains(table) {
                Rank::Schema
            } else {
                Rank::Migration
            };
            ownership.claim(
                qualify(table),
                Claim {
                    rank,
                    id: read.id.clone(),
                    standing: read.standing,
                    position: *position,
                },
            );
        }
    }
    for (read, (_, declared_objects, declared)) in reads.iter().zip(&declarations) {
        if read.judged {
            findings.extend(ownership.declared_findings(read, declared_objects, declared));
        }
    }
    for read in reads {
        ownership.created.insert(read.id, read.created);
    }
    ownership
}
