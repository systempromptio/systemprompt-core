//! Verified bookkeeping transitions for checksums: rows written before core
//! 8374d3210, and rows holding the checksum a migration declares it
//! supersedes (`-- @supersedes-checksum`).
//!
//! Historical writers used `DefaultHasher::new()`, Rust `Hash` for `str`
//! (including its terminator), and unpadded lowercase hexadecimal. Only an
//! exact match for the currently declared SQL, or the exact superseded
//! checksum, is eligible; this is not drift repair and never executes
//! migration SQL. Slot identity is checked first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AppliedMigration, MigrationService};
use std::hash::{Hash, Hasher};
use systemprompt_extension::{LoaderError, Migration};

use crate::error::RepositoryError;

pub(super) fn historical_checksum(sql: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sql.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

pub(super) fn matches_checksum(migration: &Migration, stored: &str) -> bool {
    stored == migration.checksum()
        || stored == historical_checksum(migration.sql)
        || migration.supersedes.contains(&stored)
}

impl MigrationService<'_> {
    pub(super) async fn transition_checksums(
        &self,
        extension: &str,
        migrations: &[Migration],
        applied: &[AppliedMigration],
    ) -> Result<(), LoaderError> {
        let mut transitions = Vec::new();
        for migration in migrations.iter().filter(|migration| !migration.tombstone) {
            let Some(row) = applied.iter().find(|row| row.version == migration.version) else {
                continue;
            };
            self.verify_slot_identity(extension, migration, Some(row))?;
            self.verify_checksum(extension, migration, &row.checksum)?;
            if row.name == migration.name
                && row.checksum != migration.checksum()
                && (row.checksum == historical_checksum(migration.sql)
                    || migration.supersedes.contains(&row.checksum.as_str()))
            {
                transitions.push((migration, row));
            }
        }
        if transitions.is_empty() {
            return Ok(());
        }
        let step = |context: String, source: RepositoryError| LoaderError::MigrationStepFailed {
            extension: extension.to_owned(),
            context,
            source: Box::new(source),
        };
        let mut tx = self
            .db
            .begin_transaction()
            .await
            .map_err(|error| step("Begin verified checksum transition".to_owned(), error))?;
        for (migration, row) in transitions {
            let checksum = migration.checksum();
            let result = tx.execute(
                &"UPDATE extension_migrations SET checksum=$1 WHERE extension_id=$2 AND version=$3 AND name=$4 AND checksum=$5",
                &[&checksum, &extension, &migration.version, &migration.name, &row.checksum],
            ).await;
            match result {
                Ok(1) => {},
                Ok(_) => {
                    tx.rollback()
                        .await
                        .map_err(|error| step("Rollback checksum transition".to_owned(), error))?;
                    return Err(LoaderError::MigrationFailed {
                        extension: extension.to_owned(),
                        message: "Migration history changed concurrently; retry verified checksum transition".to_owned(),
                    });
                },
                Err(error) => {
                    let rollback = tx.rollback().await;
                    return Err(step(
                        format!("Verified checksum transition failed; rollback: {rollback:?}"),
                        error,
                    ));
                },
            }
        }
        tx.commit()
            .await
            .map_err(|error| step("Commit verified checksum transition".to_owned(), error))
    }
}
