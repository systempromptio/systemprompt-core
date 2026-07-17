//! Record an extension migration as applied without running its SQL.
//!
//! Recovers the partial-state case where a migration's schema changes are
//! present in the database but no row exists in `extension_migrations` to
//! track them. Distinct from [`super::repair::repair_drift`], which deletes
//! and re-applies migrations whose stored checksum has drifted: that path
//! requires the migration to be idempotent and re-executable. Here, the
//! operator asserts the migration is already applied; the service only
//! computes the current checksum and writes the tracking row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::MigrationService;
use systemprompt_extension::{Extension, LoaderError};

#[derive(Debug, Clone)]
pub struct MarkAppliedOutcome {
    pub extension_id: String,
    pub version: u32,
    pub name: String,
    pub checksum: String,
}

impl MigrationService<'_> {
    pub async fn mark_applied(
        &self,
        extension: &dyn Extension,
        version: u32,
    ) -> Result<MarkAppliedOutcome, LoaderError> {
        let ext_id = extension.metadata().id;

        let migration = extension
            .migrations()
            .into_iter()
            .find(|m| m.version == version)
            .ok_or_else(|| LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!(
                    "Migration version {version} is not defined for extension '{ext_id}'"
                ),
            })?;

        self.ensure_migrations_table_exists().await?;

        let applied = self.get_applied_migrations(ext_id).await?;
        if applied.iter().any(|m| m.version == version) {
            return Err(LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!(
                    "Migration {version} ('{}') is already tracked as applied for extension \
                     '{ext_id}'; nothing to do",
                    migration.name
                ),
            });
        }

        let id = format!("{ext_id}_{:03}", migration.version);
        let checksum = migration.checksum();

        self.db
            .execute(
                &"INSERT INTO extension_migrations (id, extension_id, version, name, checksum) \
                  VALUES ($1, $2, $3, $4, $5)",
                &[&id, &ext_id, &migration.version, &migration.name, &checksum],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!("Failed to record migration as applied: {e}"),
            })?;

        Ok(MarkAppliedOutcome {
            extension_id: ext_id.to_owned(),
            version: migration.version,
            name: migration.name.clone(),
            checksum,
        })
    }
}
