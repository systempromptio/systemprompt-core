//! Reverting applied migrations via their declared `down` SQL.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::exec::{TrackingWrite, execute_statements_transactional};
use super::{MigrationResult, MigrationService};
use crate::services::SqlExecutor;
use systemprompt_extension::{Extension, LoaderError, Migration};
use systemprompt_identifiers::ToDbValue;
use tracing::info;

impl MigrationService<'_> {
    pub async fn run_down_migrations(
        &self,
        extension: &dyn Extension,
        count: u32,
    ) -> Result<MigrationResult, LoaderError> {
        if count == 0 {
            return Ok(MigrationResult::default());
        }

        let ext_id = extension.metadata().id;
        self.ensure_migrations_table_exists().await?;

        let result = self
            .db
            .query_raw_with(
                &"SELECT version FROM extension_migrations WHERE extension_id = $1 ORDER BY \
                  version DESC LIMIT $2",
                &[&ext_id, &count],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!("Failed to query applied migrations for revert: {e}"),
            })?;

        let versions: Vec<u32> = result
            .rows
            .iter()
            .filter_map(|row| row.get("version")?.as_i64().map(|v| v as u32))
            .collect();

        if versions.is_empty() {
            return Ok(MigrationResult::default());
        }

        let migrations = extension.migrations();
        let mut migrations_run = 0;

        for version in versions {
            self.revert_version(ext_id, version, &migrations).await?;
            migrations_run += 1;
        }

        Ok(MigrationResult {
            migrations_run,
            migrations_skipped: 0,
        })
    }

    async fn revert_version(
        &self,
        ext_id: &str,
        version: u32,
        migrations: &[Migration],
    ) -> Result<(), LoaderError> {
        let migration = migrations
            .iter()
            .find(|m| m.version == version)
            .ok_or_else(|| LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!(
                    "Cannot revert migration {version}: not declared in Extension::migrations()"
                ),
            })?;

        let down_sql = migration
            .down
            .ok_or_else(|| LoaderError::MigrationNotReversible {
                extension: ext_id.to_owned(),
                version,
            })?;

        info!(
            extension = %ext_id,
            version = migration.version,
            name = %migration.name,
            "Reverting migration"
        );

        let statements = SqlExecutor::parse_sql_statements(down_sql).map_err(|e| {
            LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!(
                    "Failed to parse down migration {} ({}): {e}",
                    migration.version, migration.name
                ),
            }
        })?;
        let delete_params: [&dyn ToDbValue; 2] = [&ext_id, &version];
        execute_statements_transactional(
            self.db,
            &statements,
            ext_id,
            migration,
            Some(TrackingWrite {
                sql: "DELETE FROM extension_migrations WHERE extension_id = $1 AND version = $2",
                params: &delete_params,
            }),
        )
        .await
    }
}
