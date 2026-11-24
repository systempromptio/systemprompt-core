//! `infra db migrate` command installing schemas and migrations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use systemprompt_database::{
    Database, MigrationConfig, MigrationService, SchemaInstallReport, SchemaMember,
    install_admitted_schemas, schema_members,
};
use systemprompt_extension::{BuiltState, LoaderError};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::Profile;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::DatabaseContext;

use systemprompt_extension::cli::{CliConfig, CommandOutput};

use super::types::DbMigrateOutput;

#[derive(Debug, Clone, Copy)]
pub(super) struct MigrateOptions {
    pub(super) allow_checksum_drift: bool,
    pub(super) repair_drift: bool,
    pub(super) full: bool,
}

pub(super) async fn execute_migrate(
    host: &dyn KernelHost,
    options: MigrateOptions,
) -> Result<CommandOutput> {
    let config = host.output();
    let sys_config = host.config()?;

    let mut notes = Vec::new();
    if config.should_show_verbose() {
        notes.push(info(format!("System path: {}", sys_config.system_path)));
        notes.push(info(format!("Database type: {}", sys_config.database_type)));
        notes.push(info(format!(
            "Database URL: {}",
            Profile::mask_database_url(&sys_config.database_url)
        )));
    }

    let database = Arc::new(
        Database::connect(
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
            &systemprompt_database::PoolConfig::default(),
        )
        .await
        .context("Failed to connect to database")?
        .with_session_url(
            sys_config.database_session_url.as_deref(),
            &systemprompt_database::PoolConfig::default().for_session(None),
        )
        .await
        .context("Failed to connect to the session database")?,
    );

    let output = run_install(host.admitted()?.built(), &database, config, options).await?;
    Ok(notes.into_iter().fold(output, CommandOutput::with_note))
}

pub(super) async fn execute_migrate_standalone(
    db_ctx: &DatabaseContext,
    built: &BuiltState,
    config: &CliConfig,
    options: MigrateOptions,
) -> Result<CommandOutput> {
    let database = db_ctx.db_pool();
    run_install(built, database, config, options).await
}

fn refuse_foreign_key_drift(report: &SchemaInstallReport) -> Result<()> {
    if report.is_clean() {
        return Ok(());
    }
    let drift: Vec<String> = report
        .foreign_key_drift
        .iter()
        .map(|d| {
            format!(
                "{}.{} ({}): {}",
                d.extension, d.table, d.constraint, d.cause
            )
        })
        .collect();
    Err(anyhow!(
        "Schema installation committed but {} declared foreign key(s) could not be created \
         on this established database; add the referenced unique index with a migration:\n{}",
        drift.len(),
        drift.join("\n")
    ))
}

async fn run_install(
    built: &BuiltState,
    database: &Database,
    config: &CliConfig,
    MigrateOptions {
        allow_checksum_drift,
        repair_drift,
        full,
    }: MigrateOptions,
) -> Result<CommandOutput> {
    let write_provider = database.session();
    let mut notes = Vec::new();
    if config.should_show_verbose() {
        notes.push(info(format!(
            "Installing schemas for {} extensions",
            install_set(built).len()
        )));
    }

    let migration_config = MigrationConfig {
        allow_checksum_drift,
        force_full_install: full,
    };

    let report = match install_admitted_schemas(built, write_provider, migration_config).await {
        Ok(report) => report,
        Err(LoaderError::MigrationChecksumDrift { .. }) if repair_drift => {
            notes.push(NoticeLine::new(
                "warning",
                "Migration checksum drift found; re-applied drifted migrations",
            ));
            let service = MigrationService::new(write_provider);
            for ext in install_set(built) {
                service
                    .repair_drift(ext.as_ref())
                    .await
                    .map_err(|e| anyhow!("Failed to repair migrations: {e}"))?;
            }
            install_admitted_schemas(built, write_provider, migration_config)
                .await
                .map_err(|e| failure(&e))?
        },
        Err(e) => return Err(failure(&e)),
    };
    if let Some(superseded) = &report.superseded {
        return Err(anyhow!(
            "Schema install refused: release {} is older than release {}, which installed this \
             database; run the migration with the newer release",
            superseded.release,
            superseded.recorded
        ));
    }
    database.mark_schema_changed();
    refuse_foreign_key_drift(&report)?;

    let installed_extensions: Vec<String> = install_set(built)
        .iter()
        .map(|ext| ext.id().to_owned())
        .collect();

    let output = DbMigrateOutput {
        modules_installed: installed_extensions,
        message: "Database migration completed successfully".to_owned(),
    };

    notes.push(NoticeLine::new("success", output.message.clone()));
    Ok(notes.into_iter().fold(
        CommandOutput::card_value("Database Admin", &output),
        CommandOutput::with_note,
    ))
}

fn info(text: String) -> NoticeLine {
    NoticeLine::new("info", text)
}

// Why: the error alone says which statement failed; the hint says what kind
// of failure it is and what fixes it, so an operator does not reach for
// checksum repair (the old entrypoint retry) on a failure it cannot fix.
fn failure(error: &LoaderError) -> anyhow::Error {
    let text = std::iter::successors(Some::<&dyn std::error::Error>(error), |e| e.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    let hint = match error {
        LoaderError::MigrationChecksumDrift { .. } => Some(
            "checksum drift: `infra db migrate --repair-drift` re-applies the edited migrations",
        ),
        LoaderError::DanglingTriggerRoutine { .. } => Some(
            "dangling trigger routine: retire the trigger in its extension's retirements, which \
             run before any migration",
        ),
        _ if text.contains("canceling statement due to statement timeout") => Some(
            "statement timeout: a migration statement ran past its bound (its @cost measurement \
             projected onto the live hot-table size, capped at 30 minutes). The transaction rolled back, so nothing is half \
             applied. Stop every other instance against this database, then rerun the one-off \
             migrate attended with SYSTEMPROMPT_MIGRATION_STATEMENT_TIMEOUT_SECS=0",
        ),
        _ if text.contains("lock timeout") => Some(
            "lock timeout: another session holds a lock on a table this migration alters; stop \
             other instances against this database and retry",
        ),
        _ => None,
    };
    hint.map_or_else(
        || anyhow!("Schema installation failed: {text}"),
        |hint| anyhow!("Schema installation failed: {text}\nHint: {hint}"),
    )
}

pub(super) fn install_set(built: &BuiltState) -> Vec<Arc<dyn SchemaMember>> {
    schema_members(built)
}

pub(super) fn find_member(
    built: &BuiltState,
    extension: &ExtensionId,
) -> Result<Arc<dyn SchemaMember>> {
    install_set(built)
        .into_iter()
        .find(|member| member.id() == extension.as_str())
        .ok_or_else(|| anyhow!("Extension '{}' not found or has no schema", extension))
}

pub(super) fn select_extensions(
    built: &BuiltState,
    extension: Option<&ExtensionId>,
) -> Result<Vec<Arc<dyn SchemaMember>>> {
    let all = install_set(built);
    if let Some(ext_id) = extension {
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| e.id() == ext_id.as_str())
            .collect();
        if filtered.is_empty() {
            return Err(anyhow!("Extension '{}' not found or has no schema", ext_id));
        }
        Ok(filtered)
    } else {
        Ok(all)
    }
}
