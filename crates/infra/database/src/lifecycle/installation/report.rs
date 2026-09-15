//! Outcome of a schema installation that succeeded but left typed drift
//! behind: a declared foreign key an established database could not create.
//! Callers surface the report — health endpoints, `infra db migrate` — rather
//! than reading it from the log.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForeignKeyDrift {
    pub extension: String,
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
/// condition fails the install there.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SchemaInstallReport {
    pub foreign_key_drift: Vec<ForeignKeyDrift>,
}

impl SchemaInstallReport {
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.foreign_key_drift.is_empty()
    }
}
