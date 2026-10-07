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
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{ExtensionRegistry, LoaderError};
use systemprompt_identifiers::ExtensionId;

use super::super::prepare::prepare_extension_schema;
use crate::lifecycle::migrations::{ChecksumDrift, MigrationService, PendingMigration};
use crate::services::DatabaseProvider;

/// How far the live schema is behind the binary's extensions.
#[derive(Debug, Default)]
pub struct SchemaCurrency {
    pub fresh_extensions: Vec<ExtensionId>,
    pub pending: Vec<PendingMigration>,
    pub drift: Vec<ChecksumDrift>,
}

impl SchemaCurrency {
    #[must_use]
    pub const fn is_current(&self) -> bool {
        self.fresh_extensions.is_empty() && self.pending.is_empty() && self.drift.is_empty()
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
            let status = migrations.status(ext.as_ref()).await?;
            currency.pending.extend(status.pending);
            currency.drift.extend(status.drift);
        }
    }

    Ok(currency)
}
