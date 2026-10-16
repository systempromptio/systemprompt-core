//! Read-only check that the database schema matches the binary.
//!
//! A replica that boots without migrating (`--skip-migrate`, or
//! `database.migrate_on_boot: false`) must still refuse a database the
//! migration step never reached. [`schema_currency`] reports every enabled
//! extension whose owned tables are all absent (never installed), every
//! defined migration with no ledger row (pending), and every applied
//! migration whose checksum no longer matches its file (drift). It reads the
//! same primitives the installer uses and executes no extension DDL.
//!
//! The check also sees the opposite direction: an extension whose ledger holds
//! a migration numbered past every one this binary declares is *ahead* — a
//! newer release migrated it during a rolling update. That is reported in
//! [`SchemaCurrency::ahead`] and is not a failure: under the expand/contract
//! contract a newer schema stays readable by the previous release. Checksum
//! drift in an ahead extension is the newer release's to judge, so it moves to
//! [`SchemaCurrency::superseded_drift`] instead of failing the boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{ExtensionRegistry, LoaderError};
use systemprompt_identifiers::ExtensionId;

use super::super::prepare::prepare_extension_schema;
use crate::lifecycle::migrations::{
    ChecksumDrift, MigrationService, OrphanedMigration, PendingMigration,
};
use crate::services::DatabaseProvider;

/// How far the live schema is behind — or ahead of — the binary's extensions.
///
/// [`SchemaCurrency::is_current`] ignores `ahead` and `superseded_drift`: a
/// schema a newer release migrated is serviceable by this one.
#[derive(Debug, Default)]
pub struct SchemaCurrency {
    pub fresh_extensions: Vec<ExtensionId>,
    pub pending: Vec<PendingMigration>,
    pub drift: Vec<ChecksumDrift>,
    pub ahead: Vec<OrphanedMigration>,
    pub superseded_drift: Vec<ChecksumDrift>,
}

impl SchemaCurrency {
    #[must_use]
    pub const fn is_current(&self) -> bool {
        self.fresh_extensions.is_empty() && self.pending.is_empty() && self.drift.is_empty()
    }

    #[must_use]
    pub const fn is_ahead(&self) -> bool {
        !self.ahead.is_empty()
    }
}

pub async fn schema_currency(
    db: &dyn DatabaseProvider,
    registry: &ExtensionRegistry,
    disabled_extensions: &[ExtensionId],
) -> Result<SchemaCurrency, LoaderError> {
    let migrations = MigrationService::new(db);
    let mut currency = SchemaCurrency::default();

    for ext in registry.enabled_schema_extensions(disabled_extensions)? {
        let prepared = prepare_extension_schema(ext.as_ref())?;
        let freshness = migrations
            .assess_freshness(&prepared.extension_id, &prepared.owned_tables)
            .await?;
        if freshness.tables_total > 0 && freshness.tables_present == 0 {
            currency.fresh_extensions.push(prepared.extension_id);
            continue;
        }
        if ext.has_migrations() {
            let latest = ext.migrations().iter().map(|m| m.version).max();
            let status = migrations.status(ext.as_ref()).await?;
            let ahead: Vec<OrphanedMigration> = status
                .orphaned
                .into_iter()
                .filter(|m| latest.is_some_and(|latest| m.version > latest))
                .collect();
            currency.pending.extend(status.pending);
            if ahead.is_empty() {
                currency.drift.extend(status.drift);
            } else {
                currency.superseded_drift.extend(status.drift);
                currency.ahead.extend(ahead);
            }
        }
    }

    Ok(currency)
}
