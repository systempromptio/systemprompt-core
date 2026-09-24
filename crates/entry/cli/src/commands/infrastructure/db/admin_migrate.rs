//! `infra db migrate` command installing schemas and migrations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{
    Database, MigrationConfig, MigrationService, install_extension_schemas_full,
};
use systemprompt_extension::{ExtensionRegistry, LoaderError};
use systemprompt_logging::CliService;
use systemprompt_models::Config;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandOutput, render_result};

use super::types::DbMigrateOutput;

pub(super) async fn execute_migrate(
    config: &CliConfig,
    allow_checksum_drift: bool,
    repair_drift: bool,
) -> Result<()> {
    let sys_config = Config::get()?;

    if config.should_show_verbose() {
        CliService::info(&format!("System path: {}", sys_config.system_path));
        CliService::info(&format!("Database type: {}", sys_config.database_type));
        CliService::info(&format!("Database URL: {}", sys_config.database_url));
    }

    let database = Arc::new(
        Database::connect(
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
            &systemprompt_database::PoolConfig::default(),
        )
        .await
        .context("Failed to connect to database")?,
    );

    run_install(
        &ExtensionRegistry::discover()?,
        database.write(),
        config,
        allow_checksum_drift,
        repair_drift,
    )
    .await
}

pub(super) async fn execute_migrate_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
    allow_checksum_drift: bool,
    repair_drift: bool,
) -> Result<()> {
    let database = db_ctx.db_pool();
    run_install(
        &ExtensionRegistry::discover()?,
        database.write(),
        config,
        allow_checksum_drift,
        repair_drift,
    )
    .await
}

async fn run_install(
    registry: &ExtensionRegistry,
    write_provider: &dyn DatabaseProvider,
    config: &CliConfig,
    allow_checksum_drift: bool,
    repair_drift: bool,
) -> Result<()> {
    let extension_count = registry.schema_extensions().len();

    if config.should_show_verbose() {
        CliService::info(&format!(
            "Installing schemas for {} extensions",
            extension_count
        ));
    }

    let migration_config = MigrationConfig {
        allow_checksum_drift,
    };

    let report =
        match install_extension_schemas_full(registry, write_provider, &[], migration_config).await
        {
            Ok(report) => report,
            Err(LoaderError::MigrationChecksumDrift { .. }) if repair_drift => {
                CliService::warning(
                    "Migration checksum drift found; re-applying drifted migrations",
                );
                let service = MigrationService::new(write_provider);
                for ext in registry.schema_extensions() {
                    service
                        .repair_drift(ext.as_ref())
                        .await
                        .map_err(|e| anyhow!("Failed to repair migrations: {e}"))?;
                }
                install_extension_schemas_full(registry, write_provider, &[], migration_config)
                    .await
                    .map_err(|e| failure(&e))?
            },
            Err(e) => return Err(failure(&e)),
        };
    if !report.is_clean() {
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
        return Err(anyhow!(
            "Schema installation committed but {} declared foreign key(s) could not be created \
             on this established database; add the referenced unique index with a migration:\n{}",
            drift.len(),
            drift.join("\n")
        ));
    }

    let installed_extensions: Vec<String> = registry
        .schema_extensions()
        .iter()
        .map(|ext| ext.id().to_owned())
        .collect();

    let output = DbMigrateOutput {
        modules_installed: installed_extensions,
        message: "Database migration completed successfully".to_owned(),
    };

    if config.is_json_output() {
        let result = CommandOutput::card_value("Database Admin", &output);
        render_result(&result, config);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}

// Why: the error alone says which statement failed; the hint says what kind
// of failure it is and what fixes it, so an operator does not reach for
// checksum repair (the old entrypoint retry) on a failure it cannot fix.
fn failure(error: &LoaderError) -> anyhow::Error {
    let text = error.to_string();
    let hint = match error {
        LoaderError::MigrationChecksumDrift { .. } => Some(
            "checksum drift: `infra db migrate --repair-drift` re-applies the edited migrations",
        ),
        LoaderError::DanglingTriggerRoutine { .. } => Some(
            "dangling trigger routine: retire the trigger in its extension's retirements, which \
             run before any migration",
        ),
        _ if text.contains("canceling statement due to statement timeout") => Some(
            "statement timeout: a migration statement ran past its bound. Row triggers on the \
             tables it writes are already suspended; if the table is far larger than the \
             migration's @cost measurement, rerun attended with \
             SYSTEMPROMPT_MIGRATION_STATEMENT_TIMEOUT_SECS=0",
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

pub(super) fn select_extensions(
    registry: &ExtensionRegistry,
    extension: Option<&str>,
) -> Result<Vec<Arc<dyn systemprompt_extension::Extension>>> {
    let all = registry.schema_extensions();
    if let Some(ext_id) = extension {
        let filtered: Vec<_> = all.into_iter().filter(|e| e.id() == ext_id).collect();
        if filtered.is_empty() {
            return Err(anyhow!("Extension '{}' not found or has no schema", ext_id));
        }
        Ok(filtered)
    } else {
        Ok(all)
    }
}
