//! Every admitted member's declared migrations, with its metadata, in
//! composition order.
//!
//! The kernel builds one [`AdmittedMigrations`] from the admitted composition
//! and seeds it into state, open: it carries no secret, so a member's command
//! (`cloud doctor`'s schema-version check) reads which migrations this binary
//! declares without naming the admitted composition. A disabled member is
//! never admitted, so its migrations are absent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::admitted::BuiltState;
use crate::metadata::ExtensionMetadata;
use crate::migration::Migration;

/// The migrations each admitted member declares, with its metadata.
#[derive(Debug, Clone, Default)]
pub struct AdmittedMigrations {
    members: Vec<(ExtensionMetadata, Vec<Migration>)>,
}

impl AdmittedMigrations {
    #[must_use]
    pub fn of(built: &BuiltState) -> Self {
        let members = built
            .members()
            .filter(|member| !member.sink.migrations().is_empty())
            .map(|member| (member.metadata, member.sink.migrations().to_vec()))
            .collect();
        Self { members }
    }

    #[must_use]
    pub fn members(&self) -> &[(ExtensionMetadata, Vec<Migration>)] {
        &self.members
    }
}
