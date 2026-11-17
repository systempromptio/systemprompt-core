//! Who owns each index, routine, view, materialized view, sequence, type,
//! domain and trigger: a `schema_object_owners` record first, then the first
//! member whose SQL creates it and does not drop it again, a core member
//! before an install member, then a declarative schema before a migration,
//! then composition order. A trigger is keyed by its name and its table.
//! Naming another member's object in a create never makes it the creator's
//! own.
//!
//! Every judgement of a named object goes through [`Ownership::resolve`],
//! which looks a name up across tables, objects, indexes and triggers, so a
//! statement that names a view, sequence or index through a table form is
//! judged against that object's owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::StandingTag;
use systemprompt_identifiers::ExtensionId;

use super::claims::{Claim, Ownership, Rank, push_claim};
use super::kinds::TableMutation;
use super::objects::Created;
use super::statements::top_level;
use super::{MigrationMembers, MigrationOwnershipError, ObjectKind, RecordedObject};
use crate::lifecycle::schema_member::SchemaMember;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Prefer {
    Table,
    Object,
    Index,
}

impl Ownership {
    pub(super) fn resolve(&self, name: &str, prefer: Prefer) -> Option<&Claim> {
        let order = match prefer {
            Prefer::Table => [&self.tables, &self.objects, &self.indexes],
            Prefer::Object => [&self.objects, &self.indexes, &self.tables],
            Prefer::Index => [&self.indexes, &self.objects, &self.tables],
        };
        order
            .into_iter()
            .chain([&self.triggers])
            .find_map(|claims| claims.get(name).and_then(|claims| claims.first()))
    }

    pub(super) fn object_owner(&self, kind: ObjectKind, name: &str) -> Option<&Claim> {
        let claims = match kind {
            ObjectKind::Object => &self.objects,
            ObjectKind::Index => &self.indexes,
            ObjectKind::Trigger => &self.triggers,
        };
        claims.get(name).and_then(|claims| claims.first())
    }

    pub(super) fn owns_anything_named(&self, member: &ExtensionId, name: &str) -> bool {
        [
            self.owner(name),
            self.object_owner(ObjectKind::Object, name),
            self.object_owner(ObjectKind::Index, name),
        ]
        .into_iter()
        .flatten()
        .any(|owner| owner.id == *member)
    }

    pub(super) fn claim_created(
        &mut self,
        id: &ExtensionId,
        (standing, position): (StandingTag, usize),
        created: &Created,
        declared: &Created,
    ) {
        for (names, declared_names, kind) in [
            (&created.objects, &declared.objects, ObjectKind::Object),
            (&created.indexes, &declared.indexes, ObjectKind::Index),
            (&created.triggers, &declared.triggers, ObjectKind::Trigger),
        ] {
            for name in names {
                let rank = if declared_names.contains(name) {
                    Rank::Schema
                } else {
                    Rank::Migration
                };
                let claim = Claim {
                    rank,
                    id: id.clone(),
                    standing,
                    position,
                };
                let claims = match kind {
                    ObjectKind::Object => &mut self.objects,
                    ObjectKind::Index => &mut self.indexes,
                    ObjectKind::Trigger => &mut self.triggers,
                };
                push_claim(claims, name.clone(), claim);
            }
        }
    }

    pub(super) fn foreign_object_creates(
        &self,
        id: &ExtensionId,
        created: &Created,
    ) -> Vec<(TableMutation, String, ExtensionId)> {
        let named = |kind: ObjectKind, statement: TableMutation, names: Vec<&String>| {
            names
                .into_iter()
                .filter_map(|name| {
                    let owner = self.object_owner(kind, name).filter(|o| o.id != *id)?;
                    Some((statement, name.clone(), owner.id.clone()))
                })
                .collect::<Vec<_>>()
        };
        let objects = created
            .objects
            .iter()
            .filter(|name| !created.tables.contains(*name))
            .collect();
        let mut refused = named(ObjectKind::Object, TableMutation::CreateObject, objects);
        refused.extend(named(
            ObjectKind::Index,
            TableMutation::CreateIndex,
            created.indexes.iter().collect(),
        ));
        refused.extend(named(
            ObjectKind::Trigger,
            TableMutation::CreateTrigger,
            created.triggers.iter().collect(),
        ));
        refused
    }

    pub(super) fn schema_creates(member: &dyn SchemaMember) -> Vec<Created> {
        member
            .schemas()
            .iter()
            .filter_map(|schema| pg_query::parse(&schema.sql).ok())
            .flat_map(|parsed| {
                top_level(&parsed)
                    .map(Created::of_statement)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub(super) fn schema_object_findings(
        &self,
        id: &ExtensionId,
        schema_created: &[Created],
    ) -> Vec<MigrationOwnershipError> {
        schema_created
            .iter()
            .flat_map(|created| self.foreign_object_creates(id, created))
            .map(
                |(statement, object, owner)| MigrationOwnershipError::ForeignObject {
                    member: id.clone(),
                    migration: String::from("schema"),
                    statement,
                    object,
                    owner,
                },
            )
            .collect()
    }

    pub(super) fn owned_objects(&self, members: MigrationMembers<'_>) -> Vec<RecordedObject> {
        let judged: Vec<ExtensionId> = members
            .judged()
            .map(|member| ExtensionId::new(member.id()))
            .collect();
        [
            (ObjectKind::Object, &self.objects),
            (ObjectKind::Index, &self.indexes),
            (ObjectKind::Trigger, &self.triggers),
        ]
        .into_iter()
        .flat_map(|(kind, claims)| {
            claims
                .iter()
                .filter(|(name, _)| !self.tables.contains_key(*name))
                .filter_map(move |(name, claims)| {
                    let owner = claims.first()?;
                    Some(RecordedObject {
                        kind,
                        name: name.clone(),
                        owner: owner.id.clone(),
                        standing: owner.standing,
                    })
                })
        })
        .filter(|record| judged.contains(&record.owner))
        .collect()
    }
}
