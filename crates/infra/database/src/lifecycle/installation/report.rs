//! Outcome of a schema installation that succeeded but left typed drift
//! behind: a declared foreign key an established database could not create,
//! or a member held back because the database records migrations for it that
//! this binary does not know ([`HeldBackMember`]; none of its schema is
//! applied).
//! Callers surface the report — health endpoints, `infra db migrate` — rather
//! than reading it from the log. `superseded` is set when a newer release
//! installed the database and this one applied nothing (see
//! `extension::release_order`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_identifiers::ExtensionId;
use tracing::{info, warn};

use super::undeclared::SchemaResidue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForeignKeyDrift {
    pub extension: ExtensionId,
    pub table: String,
    pub constraint: String,
    pub sql: String,
    pub cause: String,
}

/// What `install_extension_schemas*` applied and what it could not.
///
/// `foreign_key_drift` is non-empty only for an established extension whose
/// declarative foreign key cannot be created; the install still committed
/// every other statement. A fresh database never reports drift — the same
/// condition fails the install there. `residue` is what the database holds
/// that no registered extension declares (tables and migration ledgers a
/// deleted crate left behind); it never fails an install and is reported so
/// a drop migration can be written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SchemaInstallReport {
    pub foreign_key_drift: Vec<ForeignKeyDrift>,
    pub residue: SchemaResidue,
    pub ahead: Vec<SchemaAhead>,
    pub superseded: Option<SupersededInstall>,
}

/// A newer release installed this database; this release applied nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SupersededInstall {
    pub release: String,
    pub recorded: String,
}

impl SchemaInstallReport {
    pub(crate) fn log_outcome(&self) {
        let report = self;
        info!(
            foreign_key_drift = report.foreign_key_drift.len(),
            undeclared_tables = report.residue.undeclared_tables.len(),
            orphan_migration_ledgers = report.residue.orphan_migration_ledgers.len(),
            "Extension schema installation complete"
        );
        for table in &report.residue.undeclared_tables {
            warn!(
                schema = table.schema,
                table = table.table,
                live_rows = table.live_rows,
                "live table declared by no registered extension — add a DROP TABLE migration"
            );
        }
        for table in &report.residue.absent_member_tables {
            info!(
                owner = %table.owner,
                schema = table.schema,
                table = table.table,
                live_rows = table.live_rows,
                "live table kept for a member not in this composition"
            );
        }
        for ledger in &report.residue.orphan_migration_ledgers {
            warn!(
                extension = %ledger.extension_id,
                rows = ledger.rows,
                "extension_migrations ledger for an extension that no longer exists"
            );
        }
    }
}

impl SchemaAhead {
    #[must_use]
    pub fn from_currency(currency: &super::extension::current::SchemaCurrency) -> Vec<Self> {
        currency
            .ahead
            .iter()
            .map(|m| Self {
                extension: m.extension_id.clone(),
                version: m.version,
                name: m.name.clone(),
                breaks_older: currency
                    .breaking
                    .iter()
                    .any(|b| b.extension_id == m.extension_id && b.version == m.version),
            })
            .collect()
    }
}

/// A schema member the install held back: the database records migrations
/// for it that this binary does not know, so none of its schema was applied
/// and the database is not at this binary's schema for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeldBackMember {
    pub extension: ExtensionId,
    pub ahead: Vec<SchemaAhead>,
}

impl std::fmt::Display for HeldBackMember {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: the database records migration(s) this binary does not know:",
            self.extension
        )?;
        for migration in &self.ahead {
            write!(f, " {} {}", migration.version, migration.name)?;
        }
        Ok(())
    }
}

/// A ledger row a newer release wrote: the database is ahead of this binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaAhead {
    pub extension: ExtensionId,
    pub version: u32,
    pub name: String,
    pub breaks_older: bool,
}

impl SchemaInstallReport {
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.foreign_key_drift.is_empty()
    }

    #[must_use]
    pub const fn is_tidy(&self) -> bool {
        self.residue.is_empty()
    }

    #[must_use]
    pub const fn is_ahead(&self) -> bool {
        !self.ahead.is_empty()
    }

    #[must_use]
    pub fn breaks_this_release(&self) -> bool {
        self.ahead.iter().any(|m| m.breaks_older)
    }

    #[must_use]
    pub fn held_back(&self) -> Vec<HeldBackMember> {
        let mut held: Vec<HeldBackMember> = Vec::new();
        for migration in &self.ahead {
            match held.iter_mut().find(|m| m.extension == migration.extension) {
                Some(member) => member.ahead.push(migration.clone()),
                None => held.push(HeldBackMember {
                    extension: migration.extension.clone(),
                    ahead: vec![migration.clone()],
                }),
            }
        }
        held
    }
}
