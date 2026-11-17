//! The members an install reads for ownership, and the ones it judges.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::lifecycle::schema_member::SchemaMember;

/// The members of a composition, in composition order, and those of them an
/// install holds back: a held-back member's SQL is read for ownership, never
/// judged.
#[derive(Clone, Copy)]
pub struct MigrationMembers<'a> {
    composition: &'a [Arc<dyn SchemaMember>],
    held_back: &'a [Arc<dyn SchemaMember>],
}

impl<'a> MigrationMembers<'a> {
    #[must_use]
    pub const fn new(
        composition: &'a [Arc<dyn SchemaMember>],
        held_back: &'a [Arc<dyn SchemaMember>],
    ) -> Self {
        Self {
            composition,
            held_back,
        }
    }

    #[must_use]
    pub const fn held_back(&self) -> &'a [Arc<dyn SchemaMember>] {
        self.held_back
    }

    pub(crate) fn is_judged(&self, member: &dyn SchemaMember) -> bool {
        !self.held_back.iter().any(|held| held.id() == member.id())
    }

    pub(crate) fn all(&self) -> impl Iterator<Item = &'a Arc<dyn SchemaMember>> {
        let composition = self.composition;
        composition.iter().chain(
            self.held_back
                .iter()
                .filter(move |held| !composition.iter().any(|member| member.id() == held.id())),
        )
    }

    pub(crate) fn judged(self) -> impl Iterator<Item = &'a Arc<dyn SchemaMember>> {
        self.composition
            .iter()
            .filter(move |member| self.is_judged(member.as_ref()))
    }
}

impl std::fmt::Debug for MigrationMembers<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MigrationMembers")
            .field("composition", &self.composition.len())
            .field("held_back", &self.held_back.len())
            .finish()
    }
}
