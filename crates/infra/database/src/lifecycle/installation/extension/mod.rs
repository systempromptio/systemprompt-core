//! Schema installation for compile-time-registered
//! [`systemprompt_extension::Extension`] instances.
//!
//! Installation runs globally in five phases — structural DDL, then the
//! declarative routines, then migrations, then dependent DDL, then the
//! foreign keys deferred out of the structural `CREATE TABLE`s — so a legacy
//! database reaches its target shape before any `CREATE INDEX`/`VIEW`
//! references a migration-added column, and before any foreign key needs a
//! unique index a migration introduces. Routines go first so a migration can
//! reference a function only the declarative schema defines; a migration
//! that names a declarative-only trigger or view is refused before any
//! statement runs (`migration_refs`).
//! A fresh database (no migration history, no owned tables) skips migration
//! execution entirely: the declarative schema is the baseline, and every
//! defined migration is stamped as applied without running — in the same
//! transaction as that extension's structural DDL, so the tables and the
//! baseline claiming them can never be committed apart.
//! A session-scoped advisory lock serialises concurrent boots. See
//! `internal/guides/migrations.md`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cost_warning;
mod foreign_keys;
pub(crate) mod lock;
mod phase;
mod routine_prepass;
mod validation;

use systemprompt_extension::{Extension, ExtensionRegistry, LoaderError};
use tracing::{debug, info, warn};

use self::cost_warning::warn_unmeasured_migrations;
use self::foreign_keys::apply_foreign_keys;
use self::lock::BootstrapLockGuard;
use self::phase::execute_phase;
use self::routine_prepass::apply_routine_prepass;
use self::validation::{validate_extension_columns, validate_table_ownership};
use super::migration_refs::check_migration_references;
use super::prepare::{PreparedSchema, prepare_extension_schema};
use super::report::SchemaInstallReport;
use super::seeds::apply_seeds;
use super::undeclared::audit_schema_residue;
use crate::lifecycle::migrations::{MigrationConfig, MigrationService};
use crate::services::DatabaseProvider;

pub async fn install_extension_schemas(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
) -> Result<SchemaInstallReport, LoaderError> {
    install_extension_schemas_with_config(registry, db, &[]).await
}

pub async fn install_extension_schemas_with_config(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
    disabled_extensions: &[String],
) -> Result<SchemaInstallReport, LoaderError> {
    install_extension_schemas_full(
        registry,
        db,
        disabled_extensions,
        MigrationConfig::default(),
    )
    .await
}

pub async fn install_extension_schemas_full(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
    disabled_extensions: &[String],
    migration_config: MigrationConfig,
) -> Result<SchemaInstallReport, LoaderError> {
    let schema_extensions = registry.enabled_schema_extensions(disabled_extensions)?;

    if schema_extensions.is_empty() {
        info!("No extension schemas to install");
        return Ok(SchemaInstallReport::default());
    }

    info!(
        extensions = schema_extensions.len(),
        "Installing extension schemas"
    );

    let guard = BootstrapLockGuard::acquire(db).await?;

    let result = run_install(db, &schema_extensions, migration_config).await;

    guard.release().await;

    let report = result?;

    info!(
        foreign_key_drift = report.foreign_key_drift.len(),
        undeclared_tables = report.residue.undeclared_tables.len(),
        orphan_migration_ledgers = report.residue.orphan_migration_ledgers.len(),
        "Extension schema installation complete"
    );
    for table in &report.residue.undeclared_tables {
        warn!(
            schema = table.schema,
            table = table.table,
            live_rows = table.live_rows,
            "live table declared by no registered extension — add a DROP TABLE migration"
        );
    }
    for ledger in &report.residue.orphan_migration_ledgers {
        warn!(
            extension = ledger.extension_id,
            rows = ledger.rows,
            "extension_migrations ledger for an extension that no longer exists"
        );
    }
    Ok(report)
}

async fn run_install(
    db: &dyn DatabaseProvider,
    schema_extensions: &[std::sync::Arc<dyn Extension>],
    migration_config: MigrationConfig,
) -> Result<SchemaInstallReport, LoaderError> {
    let migration_service = MigrationService::new(db).with_config(migration_config);

    let mut prepared: Vec<PreparedSchema> = Vec::with_capacity(schema_extensions.len());
    for ext in schema_extensions {
        prepared.push(prepare_extension_schema(ext.as_ref())?);
    }

    validate_table_ownership(&prepared, schema_extensions)?;
    check_migration_references(schema_extensions)?;
    warn_unmeasured_migrations(db, &migration_service, schema_extensions).await;

    let mut fresh_extensions: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (ext, p) in schema_extensions.iter().zip(&prepared) {
        if ext.has_migrations()
            && migration_service
                .assess_freshness(&p.extension_id, &p.owned_tables)
                .await?
                .is_fresh()
        {
            fresh_extensions.insert(p.extension_id.clone());
        }
    }

    for (ext, p) in schema_extensions.iter().zip(&prepared) {
        let stamp = if fresh_extensions.contains(&p.extension_id) {
            MigrationService::baseline_stamp_rows(ext.as_ref())
        } else {
            Vec::new()
        };
        if !stamp.is_empty() {
            info!(
                extension = %p.extension_id,
                migrations_stamped = stamp.len(),
                "Fresh install: stamping migrations as baseline without executing them"
            );
        }
        execute_phase(db, &p.structural, &stamp, &p.extension_id).await?;
    }

    for ext in schema_extensions {
        if fresh_extensions.contains(ext.id()) {
            migration_service
                .run_stamped_retirements(ext.as_ref())
                .await?;
        }
    }

    apply_routine_prepass(db, &prepared).await?;

    for ext in schema_extensions {
        if ext.has_migrations() && !fresh_extensions.contains(ext.id()) {
            debug!(extension = %ext.id(), "Running pending migrations");
            migration_service
                .run_pending_migrations(ext.as_ref())
                .await?;
        }
    }

    for p in &prepared {
        execute_phase(db, &p.dependent, &[], &p.extension_id).await?;
        for cols in &p.columns_to_validate {
            validate_extension_columns(db, cols, &p.extension_id).await?;
        }
    }

    // Why: after every extension's dependent phase, not inside it — a key may
    // reference a unique index another extension's dependent phase creates.
    let mut report = SchemaInstallReport::default();
    for (ext, p) in schema_extensions.iter().zip(&prepared) {
        // Why: only an established extension with a migration chain can carry
        // pre-existing drift; anywhere else a key that cannot be created is a
        // schema bug and must fail the install.
        let established = ext.has_migrations() && !fresh_extensions.contains(&p.extension_id);
        let drift = apply_foreign_keys(db, &p.foreign_keys, &p.extension_id, !established).await?;
        report.foreign_key_drift.extend(drift);
    }

    for ext in schema_extensions {
        apply_seeds(ext.as_ref(), db).await?;
    }

    let owned: Vec<String> = prepared
        .iter()
        .flat_map(|p| p.owned_tables.clone())
        .collect();
    let ids: Vec<String> = prepared.iter().map(|p| p.extension_id.clone()).collect();
    report.residue = audit_schema_residue(db, &owned, &ids).await?;

    Ok(report)
}
