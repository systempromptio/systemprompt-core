//! Who owns each index, routine, view, sequence, type and domain: the first
//! member whose SQL creates it and does not drop it again, a core member
//! before an install member, then a declarative schema before a migration,
//! then composition order. Naming another member's object in a create never
//! makes it the creator's own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_extension::StandingTag;
use systemprompt_identifiers::ExtensionId;

use super::MigrationOwnershipError;
use super::claims::{Claim, Ownership, Rank};
use super::kinds::TableMutation;
use super::objects::Created;
use super::statements::top_level;
use crate::lifecycle::schema_member::SchemaMember;

const fn standing_rank(standing: StandingTag) -> u8 {
    match standing {
        StandingTag::Core => 0,
        StandingTag::Install => 1,
    }
}

fn claim_object(claims: &mut BTreeMap<String, Vec<Claim>>, name: &str, claim: Claim) {
    let claims = claims.entry(name.to_owned()).or_default();
    if claims.iter().any(|held| held.id == claim.id) {
        return;
    }
    claims.push(claim);
    claims.sort_by_key(|held| (standing_rank(held.standing), held.rank));
}

impl Ownership {
    pub(super) fn object_owner(&self, index: bool, name: &str) -> Option<&Claim> {
        let claims = if index { &self.indexes } else { &self.objects };
        claims.get(name).and_then(|claims| claims.first())
    }

    pub(super) fn owns_anything_named(&self, member: &ExtensionId, name: &str) -> bool {
        [
            self.owner(name),
            self.object_owner(false, name),
            self.object_owner(true, name),
        ]
        .into_iter()
        .flatten()
        .any(|owner| owner.id == *member)
    }

    pub(super) fn claim_created(
        &mut self,
        id: &ExtensionId,
        standing: StandingTag,
        created: &Created,
        declared: &Created,
    ) {
        for (names, declared_names, index) in [
            (&created.objects, &declared.objects, false),
            (&created.indexes, &declared.indexes, true),
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
                };
                let claims = if index {
                    &mut self.indexes
                } else {
                    &mut self.objects
                };
                claim_object(claims, name, claim);
            }
        }
    }

    pub(super) fn foreign_object_creates(
        &self,
        id: &ExtensionId,
        created: &Created,
    ) -> Vec<(TableMutation, String, ExtensionId)> {
        let objects = created
            .objects
            .iter()
            .filter(|name| !created.tables.contains(*name))
            .filter_map(|name| {
                let owner = self.object_owner(false, name).filter(|o| o.id != *id)?;
                Some((TableMutation::CreateObject, name.clone(), owner.id.clone()))
            });
        let indexes = created.indexes.iter().filter_map(|name| {
            let owner = self.object_owner(true, name).filter(|o| o.id != *id)?;
            Some((TableMutation::CreateIndex, name.clone(), owner.id.clone()))
        });
        objects.chain(indexes).collect()
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
}
