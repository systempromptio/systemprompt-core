//! Every admitted member's user-purge tables and orphan sweeps, stamped with
//! their owners in composition order.
//!
//! The kernel builds one [`AdmittedPurge`] from the admitted composition and
//! seeds it into state; the users domain's delete, purge and dry run take it
//! as an argument, so only admitted members' declarations are run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ExtensionId;

use crate::admitted::BuiltState;
use crate::purge::{OrphanSweep, UserPurgeTable};

/// The owner-stamped purge tables and orphan sweeps of an admitted
/// composition.
#[derive(Debug, Clone, Default)]
pub struct AdmittedPurge {
    tables: Vec<(ExtensionId, UserPurgeTable)>,
    sweeps: Vec<(ExtensionId, OrphanSweep)>,
}

impl AdmittedPurge {
    #[must_use]
    pub const fn new(
        tables: Vec<(ExtensionId, UserPurgeTable)>,
        sweeps: Vec<(ExtensionId, OrphanSweep)>,
    ) -> Self {
        Self { tables, sweeps }
    }

    #[must_use]
    pub fn of(built: &BuiltState) -> Self {
        let mut tables = Vec::new();
        let mut sweeps = Vec::new();
        for member in built.members() {
            tables.extend(
                member
                    .sink
                    .user_purges()
                    .iter()
                    .map(|table| (member.owner.clone(), *table)),
            );
            sweeps.extend(
                member
                    .sink
                    .orphan_sweeps()
                    .iter()
                    .map(|sweep| (member.owner.clone(), *sweep)),
            );
        }
        Self { tables, sweeps }
    }

    #[must_use]
    pub fn tables(&self) -> &[(ExtensionId, UserPurgeTable)] {
        &self.tables
    }

    #[must_use]
    pub fn sweeps(&self) -> &[(ExtensionId, OrphanSweep)] {
        &self.sweeps
    }
}
