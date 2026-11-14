//! Settled-schema detection: an install that has nothing to apply takes no
//! DDL locks.
//!
//! Every phase of a full install re-runs its idempotent DDL, and re-creating
//! a trigger takes an exclusive lock on the table it fires on. Against a
//! serving database that lock is taken while writers hold row locks on that
//! table and their triggers lock others, so a no-op migrate can deadlock with
//! ordinary traffic. A complete install therefore records a fingerprint of
//! everything it applied — each extension's declarative schema, migrations,
//! retirements and cross-extension tables, the disabled set and this crate's
//! version — together with a read-only digest of the live catalog
//! ([`super::catalog_digest`]). The next install under the bootstrap lock
//! skips every DDL phase only when both are unchanged, the ledger is current
//! and every owned table exists, so an object an older binary, an operator or
//! a restore changed is re-applied. `MigrationConfig::force_full_install`
//! always takes the full path. On the settled path the read-only checks
//! (required columns, trigger routines) and the seeds still run. When the
//! full path does run, an unchanged trigger is still left in place
//! ([`super::trigger_reuse`]); a forced full install re-creates every one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::hash::Hasher;

use systemprompt_extension::{Extension, LoaderError};
use systemprompt_identifiers::ExtensionId;
use tracing::info;
use xxhash_rust::xxh64::Xxh64;

use super::super::prepare::PreparedSchema;
use super::super::report::SchemaInstallReport;
use super::ExtensionSet;
use super::catalog_digest::catalog_digest;
use crate::lifecycle::migrations::MigrationConfig;
use crate::services::DatabaseProvider;

const FIELD_END: &[u8] = &[0x1f];

pub(super) struct InstallSets<'a> {
    pub(super) active: &'a ExtensionSet,
    pub(super) held_back: &'a ExtensionSet,
}

pub(super) async fn install_unless_settled(
    db: &dyn DatabaseProvider,
    InstallSets { active, held_back }: InstallSets<'_>,
    currency: &super::current::SchemaCurrency,
    disabled_extensions: &[ExtensionId],
    migration_config: MigrationConfig,
) -> Result<SchemaInstallReport, LoaderError> {
    let fingerprint = fingerprint(active, disabled_extensions);
    let candidate = held_back.is_empty()
        && currency.is_current()
        && !active.is_empty()
        && !migration_config.force_full_install;
    if candidate {
        let prepared = active
            .iter()
            .map(|ext| super::prepare_extension_schema(ext.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        if is_settled(db, &prepared, &fingerprint).await? {
            info!("Schema is settled at this release; applying no DDL");
            return run_settled(db, active, held_back, &prepared).await;
        }
    }
    if migration_config.force_full_install {
        info!("Full schema install forced");
    }
    let report = super::run_install(db, active, held_back, migration_config).await?;
    if held_back.is_empty() && report.foreign_key_drift.is_empty() {
        record(db, &fingerprint).await?;
    }
    Ok(report)
}

async fn run_settled(
    db: &dyn DatabaseProvider,
    active: &ExtensionSet,
    held_back: &ExtensionSet,
    prepared: &[PreparedSchema],
) -> Result<SchemaInstallReport, LoaderError> {
    for p in prepared {
        for cols in &p.columns_to_validate {
            super::validate_extension_columns(db, cols, &p.extension_id).await?;
        }
    }
    super::check_trigger_routines(db).await?;
    for ext in active {
        super::apply_seeds(ext.as_ref(), db).await?;
    }
    Ok(SchemaInstallReport {
        residue: super::residue_including(db, prepared, held_back).await?,
        ..SchemaInstallReport::default()
    })
}

fn stamp(fingerprint: &str, catalog: &str) -> String {
    format!("{fingerprint}:{catalog}")
}

pub(super) fn fingerprint(
    extensions: &[std::sync::Arc<dyn Extension>],
    disabled: &[ExtensionId],
) -> String {
    let mut hasher = Xxh64::new(0);
    let mut field = |bytes: &[u8]| {
        hasher.write(bytes);
        hasher.write(FIELD_END);
    };
    field(env!("CARGO_PKG_VERSION").as_bytes());
    let mut disabled_ids: Vec<&str> = disabled.iter().map(ExtensionId::as_str).collect();
    disabled_ids.sort_unstable();
    for id in disabled_ids {
        field(id.as_bytes());
    }
    for ext in extensions {
        field(ext.id().as_bytes());
        field(ext.version().as_bytes());
        for schema in ext.schemas() {
            field(schema.table.as_deref().unwrap_or_default().as_bytes());
            field(schema.schema_name().as_bytes());
            field(schema.sql.as_bytes());
            field(schema.required_columns.join(",").as_bytes());
        }
        for migration in ext.migrations() {
            field(&migration.version.to_le_bytes());
            field(migration.name.as_bytes());
            field(migration.checksum().as_bytes());
            field(&[u8::from(migration.no_transaction)]);
        }
        for retirement in ext.retirements() {
            field(retirement.sql.as_bytes());
        }
        for table in ext.cross_extension_tables() {
            field(table.as_bytes());
        }
    }
    format!("{:016x}", hasher.finish())
}

pub(super) async fn is_settled(
    db: &dyn DatabaseProvider,
    prepared: &[PreparedSchema],
    fingerprint: &str,
) -> Result<bool, LoaderError> {
    if !fingerprint_table_present(db).await? {
        return Ok(false);
    }
    let pool = db.get_postgres_pool();

    let stored =
        sqlx::query_scalar!("SELECT fingerprint FROM schema_install_fingerprint WHERE singleton")
            .fetch_optional(pool.as_ref())
            .await
            .map_err(|e| step_failed("Failed to read the schema fingerprint", e))?;
    let Some(stored) = stored else {
        return Ok(false);
    };
    if stored.split_once(':').map(|(binary, _)| binary) != Some(fingerprint) {
        return Ok(false);
    }

    let owned: Vec<String> = prepared
        .iter()
        .flat_map(|p| p.owned_tables.iter().cloned())
        .collect();
    let missing = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "missing!" FROM unnest($1::text[]) AS t(name)
           WHERE to_regclass(t.name) IS NULL"#,
        &owned
    )
    .fetch_one(pool.as_ref())
    .await
    .map_err(|e| step_failed("Failed to check owned tables", e))?;
    if missing != 0 {
        return Ok(false);
    }
    Ok(stored == stamp(fingerprint, &catalog_digest(db).await?))
}

pub(super) async fn record(
    db: &dyn DatabaseProvider,
    fingerprint: &str,
) -> Result<(), LoaderError> {
    if !fingerprint_table_present(db).await? {
        return Ok(());
    }
    let stamped = stamp(fingerprint, &catalog_digest(db).await?);
    sqlx::query!(
        "INSERT INTO schema_install_fingerprint (singleton, fingerprint, recorded_at)
         VALUES (true, $1, CURRENT_TIMESTAMP)
         ON CONFLICT (singleton) DO UPDATE
         SET fingerprint = EXCLUDED.fingerprint, recorded_at = EXCLUDED.recorded_at",
        stamped
    )
    .execute(db.get_postgres_pool().as_ref())
    .await
    .map_err(|e| step_failed("Failed to record the schema fingerprint", e))?;
    Ok(())
}

async fn fingerprint_table_present(db: &dyn DatabaseProvider) -> Result<bool, LoaderError> {
    sqlx::query_scalar!(
        r#"SELECT to_regclass('schema_install_fingerprint') IS NOT NULL AS "present!""#
    )
    .fetch_one(db.get_postgres_pool().as_ref())
    .await
    .map_err(|e| step_failed("Failed to look up the schema fingerprint table", e))
}

fn step_failed(context: &str, source: sqlx::Error) -> LoaderError {
    LoaderError::SchemaInstallationStepFailed {
        extension: ExtensionId::new("database"),
        context: context.to_owned(),
        source: Box::new(source),
    }
}
