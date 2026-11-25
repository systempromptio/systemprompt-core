//! Schema installation for compile-time-registered
//! [`SchemaMember`](crate::lifecycle::schema_member::SchemaMember) snapshots of
//! the admitted composition.
//!
//! Installation runs globally in six phases — retirements, structural DDL, then
//! the declarative routines, then migrations, then dependent DDL, then the
//! foreign keys deferred out of the structural `CREATE TABLE`s — so a legacy
//! database reaches its target shape before any `CREATE INDEX`/`VIEW`
//! references a migration-added column, and before any foreign key needs a
//! unique index a migration introduces. Routines go first so a migration can
//! reference a function only the declarative schema defines; a migration
//! that names a declarative-only trigger or view is refused before any
//! statement runs (`migration_refs`), and so is a pending migration the
//! migration ownership rule refuses (`migration_owners`). Retirements run
//! first of all, so a trigger one extension retires is gone before another
//! extension's migration can drop what it writes or fire it (`retire`).
//! A fresh database (no migration history, no owned tables) skips migration
//! execution entirely: the declarative schema is the baseline, and every
//! defined migration is stamped as applied without running — in the same
//! transaction as that extension's structural DDL, so the tables and the
//! baseline claiming them can never be committed apart.
//! A session-scoped advisory lock serialises concurrent boots.
//! Under that lock the install first checks whether the database is *ahead*
//! of this binary (see [`current`]): a newer release owns an ahead extension's
//! schema, so this binary applies nothing to that extension — no declarative
//! DDL, no migrations — while every other extension installs as usual, and
//! the report carries the ahead migrations. Before any of that the install
//! reads the release that last completed it ([`release_order`]): a binary
//! older than that release applies nothing at all.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
pub(crate) mod catalog_digest;
mod cost_warning;
pub(super) mod current;
mod foreign_keys;
pub(crate) mod lock;
mod object_owners;
pub(crate) mod ownership;
mod phase;
mod purge_records;
mod reassignment_targets;
pub(crate) mod release_order;
mod retire;
mod routine_prepass;
mod settled;
mod trigger_reuse;
mod validation;

use crate::lifecycle::schema_member::{SchemaMember, schema_members};
use crate::lifecycle::schema_name::SchemaName;
use systemprompt_extension::{BuiltState, LoaderError};
use systemprompt_identifiers::ExtensionId;
use tracing::{debug, info};

use self::apply::{apply_dependent, apply_structural};
use self::cost_warning::warn_unmeasured_migrations;
use self::lock::BootstrapLockGuard;
use self::ownership::{
    check_pending_migration_owners, check_recorded_owners, record_owners, recorded_table_owners,
};
use self::purge_records::{admit_purges, check_recorded_purges, record_purges};
use self::retire::{apply_retirements, check_retirements};
use self::routine_prepass::apply_routine_prepass;
use self::trigger_reuse::TriggerReuse;
use self::validation::{validate_extension_columns, validate_table_ownership};
use super::migration_owners::MigrationMembers;
use super::migration_refs::check_migration_references;
use super::prepare::{PreparedSchema, prepare_extension_schema};

#[derive(Clone, Copy)]
pub(super) struct PreparedAt<'a> {
    pub(super) prepared: &'a [PreparedSchema],
    pub(super) target: &'a SchemaName,
}
use super::report::SchemaInstallReport;
use super::routine_refs::check_trigger_routines_held;
use super::seeds::apply_seeds;
use super::undeclared::residue_including;
use crate::lifecycle::migrations::{MigrationConfig, MigrationService};
use crate::services::DatabaseProvider;

type ExtensionSet = [std::sync::Arc<dyn SchemaMember>];
type FreshSet = std::collections::HashSet<ExtensionId>;

pub async fn install_admitted_schemas(
    built: &BuiltState,
    db: &dyn DatabaseProvider,
    target: &SchemaName,
    migration_config: MigrationConfig,
) -> Result<SchemaInstallReport, LoaderError> {
    install_schema_members(&schema_members(built), db, target, migration_config).await
}

pub async fn install_schema_members(
    schema_extensions: &ExtensionSet,
    db: &dyn DatabaseProvider,
    target: &SchemaName,
    migration_config: MigrationConfig,
) -> Result<SchemaInstallReport, LoaderError> {
    if schema_extensions.is_empty() {
        info!("No extension schemas to install");
        return Ok(SchemaInstallReport::default());
    }
    admit_purges(schema_extensions)?;
    reassignment_targets::refuse_security_tables(schema_extensions)?;

    info!(
        extensions = schema_extensions.len(),
        "Installing extension schemas"
    );

    let guard = BootstrapLockGuard::acquire(db).await?;
    let result = install_in_release_order(db, schema_extensions, target, migration_config).await;
    guard.release().await;

    let report = result?;

    report.log_outcome();
    Ok(report)
}

async fn install_in_release_order(
    db: &dyn DatabaseProvider,
    schema_extensions: &ExtensionSet,
    target: &SchemaName,
    migration_config: MigrationConfig,
) -> Result<SchemaInstallReport, LoaderError> {
    if let Some(superseded) = release_order::newer_recorded(db).await? {
        let currency = current::members_currency(db, schema_extensions, target).await?;
        release_order::judge_superseded(&superseded, &currency)?;
        return Ok(SchemaInstallReport {
            ahead: super::report::SchemaAhead::from_currency(&currency),
            superseded: Some(superseded),
            ..SchemaInstallReport::default()
        });
    }
    let currency = current::members_currency(db, schema_extensions, target).await?;
    let (held_back, active): (Vec<_>, Vec<_>) =
        schema_extensions.iter().cloned().partition(|ext| {
            currency
                .ahead
                .iter()
                .any(|m| m.extension_id.as_str() == ext.id())
        });
    let report = settled::install_unless_settled(
        db,
        settled::InstallSets::new(schema_extensions, &active, &held_back),
        &currency,
        target,
        migration_config,
    )
    .await?;
    if held_back.is_empty() {
        release_order::record(db).await?;
    }
    let ahead = super::report::SchemaAhead::from_currency(&currency);
    let held_back = super::report::HeldBackMember::of(&ahead, |extension| {
        currency
            .pending
            .iter()
            .any(|m| &m.extension_id == extension)
    });
    Ok(SchemaInstallReport {
        ahead,
        held_back,
        ..report
    })
}

async fn fresh_extension_ids(
    migration_service: &MigrationService<'_>,
    schema_extensions: &ExtensionSet,
    prepared: &[PreparedSchema],
    target: &SchemaName,
) -> Result<FreshSet, LoaderError> {
    let mut fresh = FreshSet::new();
    for (ext, p) in schema_extensions.iter().zip(prepared) {
        if ext.has_migrations()
            && migration_service
                .assess_freshness(&p.extension_id, &p.owned_tables, target)
                .await?
                .is_fresh()
        {
            fresh.insert(p.extension_id.clone());
        }
    }
    Ok(fresh)
}

async fn run_install(
    db: &dyn DatabaseProvider,
    members: MigrationMembers<'_>,
    schema_extensions: &ExtensionSet,
    target: &SchemaName,
    migration_config: MigrationConfig,
) -> Result<SchemaInstallReport, LoaderError> {
    if schema_extensions.is_empty() {
        return Ok(SchemaInstallReport::default());
    }
    let migration_service = MigrationService::new(db).with_config(migration_config);
    let prepared =
        prepare_and_check(db, &migration_service, members, schema_extensions, target).await?;

    apply_retirements(db, schema_extensions).await?;
    let fresh =
        fresh_extension_ids(&migration_service, schema_extensions, &prepared, target).await?;
    apply_structural(db, &migration_service, schema_extensions, &prepared, &fresh).await?;
    apply_routine_prepass(db, &prepared).await?;

    for ext in schema_extensions {
        if ext.has_migrations() && !fresh.contains(ext.id()) {
            debug!(extension = %ext.id(), "Running pending migrations");
            migration_service
                .run_pending_migrations(ext.as_ref())
                .await?;
        }
    }

    let reuse = if migration_config.force_full_install {
        TriggerReuse::Reapply
    } else {
        TriggerReuse::Reuse
    };
    let mut report = apply_dependent(
        db,
        schema_extensions,
        PreparedAt {
            prepared: &prepared,
            target,
        },
        &fresh,
        reuse,
    )
    .await?;

    // Why: after every phase, so the schema checked is the one the server
    // will write to; a trigger left writing a dropped table fails every
    // request that touches its table.
    check_trigger_routines_held(db).await?;

    for ext in schema_extensions {
        apply_seeds(ext.as_ref(), db).await?;
    }

    record_owners(db, &prepared, members).await?;
    reassignment_targets::check_catalog_targets(db, schema_extensions).await?;
    record_purges(db, schema_extensions).await?;
    report.residue = residue_including(db, &prepared, members.held_back(), target).await?;

    Ok(report)
}

async fn prepare_and_check(
    db: &dyn DatabaseProvider,
    migration_service: &MigrationService<'_>,
    members: MigrationMembers<'_>,
    schema_extensions: &ExtensionSet,
    target: &SchemaName,
) -> Result<Vec<PreparedSchema>, LoaderError> {
    let mut prepared: Vec<PreparedSchema> = Vec::with_capacity(schema_extensions.len());
    for ext in schema_extensions {
        prepared.push(prepare_extension_schema(ext.as_ref())?);
    }
    validate_table_ownership(&prepared, members.held_back(), schema_extensions)?;
    let owners = recorded_table_owners(db).await?;
    check_recorded_owners(&owners, &prepared)?;
    check_pending_migration_owners(
        db,
        &owners,
        migration_service,
        members,
        PreparedAt {
            prepared: &prepared,
            target,
        },
    )
    .await?;
    check_recorded_purges(db, schema_extensions).await?;
    check_migration_references(schema_extensions)?;
    check_retirements(schema_extensions)?;
    warn_unmeasured_migrations(db, migration_service, schema_extensions).await;
    Ok(prepared)
}
