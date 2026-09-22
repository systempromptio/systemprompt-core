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
//! One class of migration is stamped **and** executed: a retirement, whose
//! every statement is a `DROP … IF EXISTS` or a `DELETE FROM
//! extension_migrations`. Such a migration retires relations that another,
//! since-deleted extension left behind, and an extension whose own tables are
//! all absent says nothing about theirs — a production database kept nineteen
//! `eval_*` tables and three orphaned ledger rows because the migration that
//! dropped them belonged to an extension the database was meeting for the
//! first time. Every statement of a retirement is idempotent, so running it
//! on a truly fresh database is a no-op.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::MigrationService;
use super::exec::execute_statements_transactional;
use crate::services::SqlExecutor;
use pg_query::NodeEnum;
use systemprompt_extension::{Extension, LoaderError, Migration};
use tracing::{info, warn};

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

    pub async fn run_stamped_retirements(
        &self,
        extension: &dyn Extension,
    ) -> Result<usize, LoaderError> {
        let ext_id = extension.metadata().id;
        let mut ran = 0usize;
        for migration in extension
            .migrations()
            .iter()
            .filter(|migration| !migration.tombstone && is_retirement(migration))
        {
            let statements = SqlExecutor::parse_sql_statements(migration.sql).map_err(|e| {
                LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to parse retirement migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                }
            })?;
            info!(
                extension = %ext_id,
                version = migration.version,
                name = %migration.name,
                "Fresh install: executing stamped retirement migration"
            );
            execute_statements_transactional(self.db, &statements, ext_id, migration, None).await?;
            ran += 1;
        }
        Ok(ran)
    }

    #[must_use]
    pub fn baseline_stamp_rows(extension: &dyn Extension) -> Vec<BaselineStamp> {
        let ext_id = extension.metadata().id;
        extension
            .migrations()
            .iter()
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

#[must_use]
pub fn is_retirement(migration: &Migration) -> bool {
    let Ok(parsed) = pg_query::parse(migration.sql) else {
        return false;
    };
    let mut statements = 0usize;
    for raw in parsed.protobuf.stmts {
        let Some(node) = raw.stmt.and_then(|s| s.node) else {
            continue;
        };
        statements += 1;
        let retires = match &node {
            NodeEnum::DropStmt(drop) => drop.missing_ok,
            NodeEnum::DeleteStmt(delete) => delete
                .relation
                .as_ref()
                .is_some_and(|relation| relation.relname == "extension_migrations"),
            _ => false,
        };
        if !retires {
            return false;
        }
    }
    statements > 0
}
