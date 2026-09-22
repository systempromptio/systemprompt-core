//! Running one migration: the transactional path, and the `no_transaction`
//! path whose statement and lock timeouts are set on the connection and reset
//! after, because there is no transaction to scope them to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{Extension, LoaderError, Migration};
use systemprompt_identifiers::ToDbValue;
use tracing::info;

use super::exec::{TrackingWrite, check_cross_extension_alters, execute_statements_transactional};
use super::{MigrationService, RECORD_MIGRATION_SQL, budget};
use crate::services::SqlExecutor;

impl MigrationService<'_> {
    pub(super) async fn execute_migration(
        &self,
        extension: &dyn Extension,
        migration: &Migration,
    ) -> Result<(), LoaderError> {
        let ext_id = extension.metadata().id;

        check_cross_extension_alters(extension, migration)?;

        info!(
            extension = %ext_id,
            version = migration.version,
            name = %migration.name,
            no_transaction = migration.no_transaction,
            "Running migration"
        );

        let id = format!("{}_{:03}", ext_id, migration.version);
        let checksum = migration.checksum();
        let record_params: [&dyn ToDbValue; 5] =
            [&id, &ext_id, &migration.version, &migration.name, &checksum];

        if migration.no_transaction {
            // Why: no transaction to scope the bound to, so it is set on the
            // connection and reset after — `SET LOCAL` would be a silent
            // no-op here, leaving this path the only unbounded one.
            self.apply_timeouts(ext_id, migration).await?;
            let outcome = SqlExecutor::execute_statements_parsed(self.db, migration.sql)
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to execute migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                });
            self.clear_timeouts(ext_id).await?;
            outcome?;
            self.db
                .execute(&RECORD_MIGRATION_SQL, &record_params)
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!("Failed to record migration: {e}"),
                })?;
        } else {
            let statements = SqlExecutor::parse_sql_statements(migration.sql).map_err(|e| {
                LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to parse migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                }
            })?;
            execute_statements_transactional(
                self.db,
                &statements,
                ext_id,
                migration,
                Some(TrackingWrite {
                    sql: RECORD_MIGRATION_SQL,
                    params: &record_params,
                }),
            )
            .await?;
        }

        Ok(())
    }

    async fn apply_timeouts(&self, ext_id: &str, migration: &Migration) -> Result<(), LoaderError> {
        let timeout = budget::statement_timeout(migration);
        self.set_timeouts(ext_id, &budget::timeout_statements(timeout, false))
            .await
    }

    // Why: the connection outlives this migration, so a bound left on it
    // would apply to whatever ran next — including the application's own
    // queries if the pool hands the connection back.
    async fn clear_timeouts(&self, ext_id: &str) -> Result<(), LoaderError> {
        self.set_timeouts(
            ext_id,
            &[
                "SET statement_timeout = DEFAULT".to_owned(),
                "SET lock_timeout = DEFAULT".to_owned(),
            ],
        )
        .await
    }

    async fn set_timeouts(&self, ext_id: &str, statements: &[String]) -> Result<(), LoaderError> {
        for statement in statements {
            self.db
                .execute(&statement.as_str(), &[])
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!("Failed to run `{statement}`: {e}"),
                })?;
        }
        Ok(())
    }
}
