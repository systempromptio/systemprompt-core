//! Outcome of a schema installation that succeeded but left typed drift
//! behind: a declared foreign key an established database could not create.
//! Callers surface the report — health endpoints, `infra db migrate` — rather
//! than reading it from the log.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_identifiers::ExtensionId;

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
}
