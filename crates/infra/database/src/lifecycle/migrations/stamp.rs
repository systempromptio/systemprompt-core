//! Fresh-install baseline stamping.
//!
//! The declarative schema (`schema/*.sql`) is the baseline: a fresh database
//! reaches target shape from the structural/dependent DDL alone, so its
//! migrations carry no information and must not execute. [`MigrationService::
//! assess_freshness`] decides, before any DDL has run, whether an extension is
//! landing on a fresh database; [`MigrationService::stamp_all_migrations`]
//! then records every defined migration in `extension_migrations` without
//! executing its SQL. Established databases (any tracking history, or any
//! owned table already present) take the normal incremental path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::MigrationService;
use systemprompt_extension::{Extension, LoaderError};
use tracing::{info, warn};

#[derive(Debug, Clone, Copy)]
pub struct FreshnessCheck {
    pub no_history: bool,
    pub tables_present: usize,
    pub tables_total: usize,
}

impl FreshnessCheck {
    #[must_use]
    pub const fn is_fresh(&self) -> bool {
        self.no_history && self.tables_present == 0
    }
}

impl MigrationService<'_> {
    pub async fn assess_freshness(
        &self,
        extension_id: &str,
        owned_tables: &[String],
    ) -> Result<FreshnessCheck, LoaderError> {
        self.ensure_migrations_table_exists().await?;

        let no_history = self.get_applied_migrations(extension_id).await?.is_empty();

        let mut tables_present = 0usize;
        for table in owned_tables {
            let (schema, name) = table.split_once('.').unwrap_or(("public", table.as_str()));
            let result = self
                .db
                .query_raw_with(
                    &"SELECT 1 AS present FROM information_schema.tables WHERE table_schema = $1 \
                      AND table_name = $2",
                    &[&schema, &name],
                )
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: extension_id.to_owned(),
                    message: format!("Failed to check for existing table '{table}': {e}"),
                })?;
            if !result.rows.is_empty() {
                tables_present += 1;
            }
        }

        let check = FreshnessCheck {
            no_history,
            tables_present,
            tables_total: owned_tables.len(),
        };

        if check.no_history && check.tables_present > 0 && check.tables_present < check.tables_total
        {
            warn!(
                extension = %extension_id,
                tables_present = check.tables_present,
                tables_total = check.tables_total,
                "Extension has no migration history but some owned tables already exist; \
                 treating as an established database and executing migrations normally"
            );
        }

        Ok(check)
    }

    pub async fn stamp_all_migrations(
        &self,
        extension: &dyn Extension,
    ) -> Result<u32, LoaderError> {
        let ext_id = extension.metadata().id;
        let migrations = extension.migrations();

        if migrations.is_empty() {
            return Ok(0);
        }

        let mut tx =
            self.db
                .begin_transaction()
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!("Failed to begin baseline stamp transaction: {e}"),
                })?;

        let mut stamped = 0u32;
        for migration in &migrations {
            // Why: a tombstone has no SQL, so stamping it would record a
            // checksum of the empty string against a slot this database never
            // used. The slot stays free of tracking rows here and spent in the
            // tree, which is exactly the truth.
            if migration.tombstone {
                continue;
            }
            let id = format!("{}_{:03}", ext_id, migration.version);
            let checksum = migration.checksum();
            if let Err(e) = tx
                .execute(
                    &"INSERT INTO extension_migrations (id, extension_id, version, name, \
                      checksum) VALUES ($1, $2, $3, $4, $5)",
                    &[&id, &ext_id, &migration.version, &migration.name, &checksum],
                )
                .await
            {
                let rollback_note = match tx.rollback().await {
                    Ok(()) => String::new(),
                    Err(rb) => format!(" (rollback also failed: {rb})"),
                };
                return Err(LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to stamp migration {} ({}) as applied: {e}{rollback_note}",
                        migration.version, migration.name
                    ),
                });
            }
            stamped += 1;
        }

        tx.commit()
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!("Failed to commit baseline stamp: {e}"),
            })?;

        info!(
            extension = %ext_id,
            migrations_stamped = stamped,
            "Fresh install: stamped migrations as baseline without executing them"
        );

        Ok(stamped)
    }
}
