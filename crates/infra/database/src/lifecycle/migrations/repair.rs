//! Migration checksum-drift repair.
//!
//! When an already-applied migration file is edited in place, its stored
//! checksum stops matching the file and the runner refuses to proceed.
//! [`MigrationService::repair_drift`] reconciles the `extension_migrations`
//! tracking table by dropping the drifted rows and re-applying those
//! migrations — every migration is idempotent (guarded seeds or
//! `CREATE ... IF NOT EXISTS`), so re-running them re-records the current
//! checksum without touching real data.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ChecksumDrift, MigrationService};
use systemprompt_extension::{Extension, LoaderError};

#[derive(Debug, Default, Clone)]
pub struct RepairResult {
    pub repaired: Vec<ChecksumDrift>,
    pub migrations_run: usize,
}

impl MigrationService<'_> {
    pub async fn repair_drift(
        &self,
        extension: &dyn Extension,
    ) -> Result<RepairResult, LoaderError> {
        let ext_id = extension.metadata().id;
        let status = self.status(extension).await?;

        if status.drift.is_empty() {
            return Ok(RepairResult::default());
        }

        for drift in &status.drift {
            self.db
                .execute(
                    &"DELETE FROM extension_migrations WHERE extension_id = $1 AND version = $2",
                    &[&drift.extension_id, &drift.version],
                )
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to drop drifted migration record {} ('{}'): {e}",
                        drift.version, drift.name
                    ),
                })?;
        }

        let result = self.run_pending_migrations(extension).await?;

        Ok(RepairResult {
            repaired: status.drift,
            migrations_run: result.migrations_run,
        })
    }
}
