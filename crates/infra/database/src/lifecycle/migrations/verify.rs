//! Pre-execution verification of an already-applied migration slot.
//!
//! Two distinct failures wear the same shape — a stored row that disagrees
//! with the file now occupying its slot.
//! [`MigrationService::verify_slot_identity`] catches a *reused* slot, where
//! the row describes a different migration entirely; reconciling that would
//! stamp one migration's checksum onto another's row and silence its drift
//! detector for good. [`MigrationService::verify_checksum`] catches ordinary
//! drift, where the same migration has been edited since it ran.
//!
//! `--allow-checksum-drift` downgrades either to a warning, which is why both
//! log the values they compared before returning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{LoaderError, Migration};
use tracing::warn;

use super::{AppliedMigration, MigrationService};

impl MigrationService<'_> {
    pub(super) fn verify_slot_identity(
        &self,
        ext_id: &str,
        migration: &Migration,
        stored: Option<&AppliedMigration>,
    ) -> Result<(), LoaderError> {
        let Some(stored) = stored else {
            return Ok(());
        };
        if stored.name == migration.name {
            return Ok(());
        }
        if self.config.allow_checksum_drift {
            warn!(
                extension = %ext_id,
                version = migration.version,
                stored_name = %stored.name,
                current_name = %migration.name,
                "Migration slot reuse tolerated by --allow-checksum-drift"
            );
            return Ok(());
        }
        Err(LoaderError::MigrationSlotReused {
            extension: ext_id.to_owned(),
            version: migration.version,
            stored_name: stored.name.clone(),
            current_name: migration.name.clone(),
        })
    }

    pub(super) fn verify_checksum(
        &self,
        ext_id: &str,
        migration: &Migration,
        stored: Option<&str>,
    ) -> Result<(), LoaderError> {
        let Some(stored_checksum) = stored else {
            return Ok(());
        };
        let current_checksum = migration.checksum();
        if stored_checksum == current_checksum {
            return Ok(());
        }
        if self.config.allow_checksum_drift {
            warn!(
                extension = %ext_id,
                version = migration.version,
                name = %migration.name,
                stored_checksum = %stored_checksum,
                current_checksum = %current_checksum,
                "Migration checksum mismatch tolerated by --allow-checksum-drift"
            );
            return Ok(());
        }
        Err(LoaderError::MigrationFailed {
            extension: ext_id.to_owned(),
            message: format!(
                "Migration {ver} ('{name}') has been edited since it was applied (stored checksum \
                 {stored_checksum}, current {current_checksum}). Refusing to proceed. If the \
                 database schema already matches the edited file, run `systemprompt infra db \
                 migrate-repair --reconcile-only --apply` to rewrite the stored checksum without \
                 executing any SQL. To re-execute the edited migration, run `systemprompt infra \
                 db migrate-repair --apply`. Passing --allow-checksum-drift bypasses the check \
                 without fixing it.",
                ver = migration.version,
                name = migration.name,
            ),
        })
    }
}
