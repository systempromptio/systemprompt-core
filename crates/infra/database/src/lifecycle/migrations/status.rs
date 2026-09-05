//! Migration status and plan queries, plus the value types they return.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::MigrationService;
use std::collections::HashSet;
use systemprompt_extension::{Extension, LoaderError, Migration};

#[derive(Debug, Clone)]
pub struct AppliedMigration {
    pub extension_id: String,
    pub version: u32,
    pub name: String,
    pub checksum: String,
    pub applied_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingMigration {
    pub extension_id: String,
    pub version: u32,
    pub name: String,
    pub sql: &'static str,
    pub checksum: String,
    pub no_tx: bool,
}

/// An applied migration whose slot the extension no longer declares at all.
///
/// The file was deleted without leaving a `.tombstone`, so the number reads as
/// free in the tree while every established database has spent it.
#[derive(Debug, Clone)]
pub struct OrphanedMigration {
    pub extension_id: String,
    pub version: u32,
    pub name: String,
}

/// A slot declared spent by a `.tombstone` file.
#[derive(Debug, Clone)]
pub struct TombstonedSlot {
    pub extension_id: String,
    pub version: u32,
    pub name: String,
    pub tracked: bool,
}

#[derive(Debug, Clone)]
pub struct ChecksumDrift {
    pub extension_id: String,
    pub version: u32,
    pub name: String,
    pub stored_checksum: String,
    pub current_checksum: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExtensionMigrationStatus {
    pub extension_id: String,
    pub applied: Vec<AppliedMigration>,
    pub pending: Vec<PendingMigration>,
    pub drift: Vec<ChecksumDrift>,
    pub orphaned: Vec<OrphanedMigration>,
    pub tombstoned: Vec<TombstonedSlot>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MigrationResult {
    pub migrations_run: usize,
    pub migrations_skipped: usize,
}

#[derive(Debug)]
pub struct MigrationStatus {
    pub extension_id: String,
    pub total_defined: usize,
    pub total_applied: usize,
    pub pending_count: usize,
    pub pending: Vec<Migration>,
    pub applied: Vec<AppliedMigration>,
}

impl MigrationService<'_> {
    pub async fn plan_pending(
        &self,
        extension: &dyn Extension,
    ) -> Result<Vec<PendingMigration>, LoaderError> {
        let ext_id = extension.metadata().id;
        let defined = extension.migrations();

        if defined.is_empty() {
            return Ok(Vec::new());
        }

        self.ensure_migrations_table_exists().await?;
        let applied_versions: HashSet<u32> = self
            .get_applied_migrations(ext_id)
            .await?
            .into_iter()
            .map(|m| m.version)
            .collect();

        Ok(defined
            .into_iter()
            .filter(|m| !m.tombstone && !applied_versions.contains(&m.version))
            .map(|m| PendingMigration {
                extension_id: ext_id.to_owned(),
                version: m.version,
                name: m.name.clone(),
                sql: m.sql,
                checksum: m.checksum(),
                no_tx: m.no_transaction,
            })
            .collect())
    }

    pub async fn status(
        &self,
        extension: &dyn Extension,
    ) -> Result<ExtensionMigrationStatus, LoaderError> {
        let ext_id = extension.metadata().id;
        let defined = extension.migrations();

        self.ensure_migrations_table_exists().await?;
        let applied = self.get_applied_migrations(ext_id).await?;

        let applied_versions: HashSet<u32> = applied.iter().map(|m| m.version).collect();
        let applied_checksums: std::collections::HashMap<u32, &str> = applied
            .iter()
            .map(|m| (m.version, m.checksum.as_str()))
            .collect();

        let mut pending = Vec::new();
        let mut drift = Vec::new();
        let mut tombstoned = Vec::new();

        for m in &defined {
            if m.tombstone {
                tombstoned.push(TombstonedSlot {
                    extension_id: ext_id.to_owned(),
                    version: m.version,
                    name: m.name.clone(),
                    tracked: applied_versions.contains(&m.version),
                });
                continue;
            }
            let current_checksum = m.checksum();
            if applied_versions.contains(&m.version) {
                if let Some(&stored_checksum) = applied_checksums.get(&m.version)
                    && stored_checksum != current_checksum
                {
                    drift.push(ChecksumDrift {
                        extension_id: ext_id.to_owned(),
                        version: m.version,
                        name: m.name.clone(),
                        stored_checksum: stored_checksum.to_owned(),
                        current_checksum,
                    });
                }
            } else {
                pending.push(PendingMigration {
                    extension_id: ext_id.to_owned(),
                    version: m.version,
                    name: m.name.clone(),
                    sql: m.sql,
                    checksum: current_checksum,
                    no_tx: m.no_transaction,
                });
            }
        }

        let orphaned = super::orphaned_versions(&applied, &defined)
            .into_iter()
            .map(|version| OrphanedMigration {
                extension_id: ext_id.to_owned(),
                version,
                name: applied
                    .iter()
                    .find(|m| m.version == version)
                    .map_or_else(String::new, |m| m.name.clone()),
            })
            .collect();

        Ok(ExtensionMigrationStatus {
            extension_id: ext_id.to_owned(),
            applied,
            pending,
            drift,
            orphaned,
            tombstoned,
        })
    }

    pub async fn get_migration_status(
        &self,
        extension: &dyn Extension,
    ) -> Result<MigrationStatus, LoaderError> {
        self.ensure_migrations_table_exists().await?;

        let ext_id = extension.metadata().id;
        let defined_migrations = extension.migrations();
        let applied = self.get_applied_migrations(ext_id).await?;

        let applied_versions: HashSet<u32> = applied.iter().map(|m| m.version).collect();

        let pending: Vec<_> = defined_migrations
            .iter()
            .filter(|m| !m.tombstone && !applied_versions.contains(&m.version))
            .cloned()
            .collect();

        Ok(MigrationStatus {
            extension_id: ext_id.to_owned(),
            total_defined: defined_migrations.len(),
            total_applied: applied.len(),
            pending_count: pending.len(),
            pending,
            applied,
        })
    }
}
