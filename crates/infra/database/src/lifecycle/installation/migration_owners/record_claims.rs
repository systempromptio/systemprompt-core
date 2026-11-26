//! Which ownership records count as claims. A record is a claim only while
//! it is live. An object record (index, routine, view, materialized view,
//! sequence, type, domain, trigger) is stale when its owner is a judged
//! member of this install whose SQL no longer creates the object: its code
//! is present, so its code decides, and the record never counts as the
//! create that keeps it. A member with a tombstoned migration cannot show from
//! its code that it no longer creates an object (a tombstone carries no SQL),
//! so its object records stay, except a trigger on a table its retirements
//! name. A core-standing object record is also stale when its owner's
//! declarative schema no longer creates the object and another core member's
//! does (its migrations or a tombstone may still name it): the object passes
//! to that member, as a fresh install ranks it. Any record, table or object, is
//! stale when it carries install standing, its owner is an install member
//! judged in this install, and a core member of the composition creates the
//! same name: the owner's own code is present, and its create of a core
//! member's name is refused. A stale record is no claim, and the install
//! deletes it after its DDL (`Stale`). A record of a held-back member, or of a
//! member outside the composition, is never stale: it outranks every member's
//! code, a core member's included, so a core member creating that name is
//! refused; a table record is released by the rules in `extension::ownership`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_extension::StandingTag;
use systemprompt_identifiers::ExtensionId;

use super::super::undeclared::qualify;
use super::claims::{Claim, Ownership, Rank, push_claim};
use super::member_reads::retired_tables;
use super::objects::Created;
use super::{
    KERNEL_LEDGER_OWNER, KERNEL_LEDGERS, MigrationMembers, ObjectKind, Recorded, RecordedObject,
    RecordedTable,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct Stale {
    pub(crate) tables: Vec<RecordedTable>,
    pub(crate) objects: Vec<RecordedObject>,
}

pub(super) struct MemberRead {
    pub(super) id: ExtensionId,
    pub(super) standing: StandingTag,
    pub(super) judged: bool,
    pub(super) tombstoned: bool,
    pub(super) retired: BTreeSet<String>,
    pub(super) created: Created,
    pub(super) declared: Created,
}

fn names(created: &Created, kind: Option<ObjectKind>, name: &str) -> bool {
    match kind {
        None => created.tables.contains(name),
        Some(ObjectKind::Object) => created.objects.contains(name),
        Some(ObjectKind::Index) => created.indexes.contains(name),
        Some(ObjectKind::Trigger) => created.triggers.contains(name),
    }
}

impl MemberRead {
    fn creates(&self, kind: Option<ObjectKind>, name: &str) -> bool {
        names(&self.created, kind, name)
    }

    fn declares(&self, kind: Option<ObjectKind>, name: &str) -> bool {
        names(&self.declared, kind, name)
    }

    fn retired_trigger(&self, kind: Option<ObjectKind>, name: &str) -> bool {
        kind == Some(ObjectKind::Trigger)
            && self
                .retired
                .iter()
                .any(|table| name.ends_with(&format!(" ON {table}")))
    }
}

fn stale(
    reads: &[MemberRead],
    kind: Option<ObjectKind>,
    name: &str,
    owner: &ExtensionId,
    standing: StandingTag,
) -> bool {
    let owner_judged = reads
        .iter()
        .any(|read| read.judged && read.id == *owner && read.standing == StandingTag::Install);
    let core_creates = standing == StandingTag::Install
        && owner_judged
        && reads
            .iter()
            .any(|read| read.standing == StandingTag::Core && read.creates(kind, name));
    let core_peer_declares = standing == StandingTag::Core
        && reads.iter().any(|read| {
            read.id != *owner && read.standing == StandingTag::Core && read.declares(kind, name)
        });
    let owner_dropped = kind.is_some()
        && reads.iter().any(|read| {
            read.judged
                && read.id == *owner
                && !(read.standing == StandingTag::Install && standing == StandingTag::Core)
                && ((!read.creates(kind, name)
                    && (!read.tombstoned || read.retired_trigger(kind, name)))
                    || (core_peer_declares && !read.declares(kind, name)))
        });
    core_creates || owner_dropped
}

impl Ownership {
    pub(super) fn claim_records(
        &mut self,
        members: MigrationMembers<'_>,
        recorded: &Recorded<'_>,
        reads: &[MemberRead],
    ) {
        let retired: BTreeMap<ExtensionId, BTreeSet<String>> = members
            .all()
            .map(|member| {
                (
                    ExtensionId::new(member.id()),
                    retired_tables(member.as_ref()),
                )
            })
            .collect();
        for record in recorded.tables.iter().filter(|record| {
            retired
                .get(&record.owner)
                .is_none_or(|tables| !tables.contains(&qualify(&record.table)))
        }) {
            let table = qualify(&record.table);
            if stale(reads, None, &table, &record.owner, record.standing) {
                self.stale.tables.push(record.clone());
                continue;
            }
            self.claim(
                table,
                Claim {
                    rank: Rank::Record,
                    id: record.owner.clone(),
                    standing: record.standing,
                    position: 0,
                },
            );
        }
        for record in recorded.objects {
            if stale(
                reads,
                Some(record.kind),
                &record.name,
                &record.owner,
                record.standing,
            ) {
                self.stale.objects.push(record.clone());
                continue;
            }
            let claims = match record.kind {
                ObjectKind::Object => &mut self.objects,
                ObjectKind::Index => &mut self.indexes,
                ObjectKind::Trigger => &mut self.triggers,
            };
            push_claim(
                claims,
                record.name.clone(),
                Claim {
                    rank: Rank::Record,
                    id: record.owner.clone(),
                    standing: record.standing,
                    position: 0,
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
                    position: 0,
                },
            );
        }
    }
}
