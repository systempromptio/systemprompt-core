//! The `@compat: breaking` ledger: contract migrations recorded where a
//! previous release, which never saw their files, can still read them.
//!
//! The ledger is created on first use by the migrate step, under the
//! bootstrap lock, and also declared by the database extension's schema.
//! A row is written before the migration runs, so a crash between the two
//! leaves a marker for an unapplied version; readers only consult markers for
//! versions the ledger records as applied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;

use super::MigrationService;

const RECORD_BREAKING_SQL: &str = "INSERT INTO extension_migration_compat (extension_id, \
                                   version, breaks_older) VALUES ($1, $2, TRUE) ON CONFLICT \
                                   (extension_id, version) DO UPDATE SET breaks_older = TRUE";

const LEDGER_EXISTS_SQL: &str =
    "SELECT to_regclass('extension_migration_compat') IS NOT NULL AS present";

const BREAKING_VERSIONS_SQL: &str = "SELECT version FROM extension_migration_compat WHERE \
                                     extension_id = $1 AND breaks_older ORDER BY version";

impl MigrationService<'_> {
    pub(super) async fn record_breaking(
        &self,
        ext_id: &ExtensionId,
        version: u32,
    ) -> Result<(), LoaderError> {
        let ledger = include_str!("../../../schema/extension_migration_compat.sql");
        crate::services::SqlExecutor::execute_statements_parsed(self.db, ledger)
            .await
            .map_err(|e| LoaderError::MigrationStepFailed {
                extension: ext_id.clone(),
                context: "Failed to ensure the migration compatibility ledger exists".to_owned(),
                source: Box::new(e),
            })?;
        self.db
            .execute(&RECORD_BREAKING_SQL, &[&ext_id, &version])
            .await
            .map_err(|e| LoaderError::MigrationStepFailed {
                extension: ext_id.clone(),
                context: format!("Failed to record migration {version} as breaking"),
                source: Box::new(e),
            })?;
        Ok(())
    }

    // Why: this runs on verify-only boots, outside the bootstrap lock, so it
    // must not race other replicas creating the ledger; a database no migrate
    // step has touched since the ledger was introduced has no breaking rows.
    pub async fn breaking_versions(&self, ext_id: &ExtensionId) -> Result<Vec<u32>, LoaderError> {
        let ledger = self
            .db
            .query_raw_with(&LEDGER_EXISTS_SQL, &[])
            .await
            .map_err(|e| LoaderError::MigrationStepFailed {
                extension: ext_id.clone(),
                context: "Failed to look up the migration compatibility ledger".to_owned(),
                source: Box::new(e),
            })?;
        let present = ledger
            .rows
            .first()
            .and_then(|row| row.get("present"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !present {
            return Ok(Vec::new());
        }
        let result = self
            .db
            .query_raw_with(&BREAKING_VERSIONS_SQL, &[&ext_id])
            .await
            .map_err(|e| LoaderError::MigrationStepFailed {
                extension: ext_id.clone(),
                context: "Failed to read the migration compatibility ledger".to_owned(),
                source: Box::new(e),
            })?;
        Ok(result
            .rows
            .iter()
            .filter_map(|row| row.get("version").and_then(serde_json::Value::as_i64))
            .filter_map(|v| u32::try_from(v).ok())
            .collect())
    }
}
