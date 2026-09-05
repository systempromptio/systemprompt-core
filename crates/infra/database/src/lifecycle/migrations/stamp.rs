//! Fresh-install baseline stamping.
//!
//! The declarative schema (`schema/*.sql`) is the baseline: a fresh database
//! reaches target shape from the structural/dependent DDL alone, so its
//! migrations carry no information and must not execute. [`MigrationService::
//! assess_freshness`] decides, before any DDL has run, whether an extension is
//! landing on a fresh database; [`MigrationService::baseline_stamp_rows`] then
//! yields the `extension_migrations` rows recording every defined migration as
//! applied, which the installer commits alongside the structural DDL rather
//! than executing their SQL. Established databases (any tracking history, or
//! any owned table already present) take the normal incremental path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::MigrationService;
use systemprompt_extension::{Extension, LoaderError};
use tracing::warn;

/// One `extension_migrations` row recording a migration as applied without
/// having executed it.
#[derive(Debug, Clone)]
pub struct BaselineStamp {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub checksum: String,
}

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

    // Why: the rows only, executed by the caller — the installer commits them
    // in the same transaction as the extension's structural DDL. Stamping in a
    // transaction of its own left a window in which the tables existed and the
    // baseline did not, and a database in that state is no longer fresh: the
    // next install calls it established and executes migration SQL written for
    // a schema shape the declarative baseline has already moved past.
    #[must_use]
    pub fn baseline_stamp_rows(extension: &dyn Extension) -> Vec<BaselineStamp> {
        let ext_id = extension.metadata().id;
        extension
            .migrations()
            .iter()
            // Why: a tombstone has no SQL, so stamping it would record a
            // checksum of the empty string against a slot this database never
            // used. The slot stays free of tracking rows here and spent in the
            // tree, which is exactly the truth.
            .filter(|migration| !migration.tombstone)
            .map(|migration| BaselineStamp {
                id: format!("{}_{:03}", ext_id, migration.version),
                version: migration.version,
                name: migration.name.clone(),
                checksum: migration.checksum(),
            })
            .collect()
    }
}
