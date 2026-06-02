//! `db migrate-repair` subcommand.
//!
//! Detects and, with `--apply`, repairs checksum drift between stored migration
//! rows and the current migration sources for one or all extensions. Provides
//! both the full-context and standalone (`DatabaseContext`-only) entry points.

use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{Database, MigrationService};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_models::Config;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandOutput, render_result};

use super::admin_migrate::select_extensions;
use super::types::{MigrateRepairOutput, MigrationDriftInfo};

#[derive(Clone, Copy)]
pub(super) struct RepairArgs<'a> {
    pub extension: Option<&'a str>,
    pub apply: bool,
    pub json: bool,
}

pub(super) async fn execute_migrate_repair(config: &CliConfig, args: RepairArgs<'_>) -> Result<()> {
    let sys_config = Config::get()?;
    let database = Arc::new(
        Database::from_config_with_write(
            &sys_config.database_type,
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
        )
        .await
        .context("Failed to connect to database")?,
    );

    run_migrate_repair(
        database.write(),
        &ExtensionRegistry::discover()?,
        args,
        config,
    )
    .await
}

pub(super) async fn execute_migrate_repair_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
    args: RepairArgs<'_>,
) -> Result<()> {
    run_migrate_repair(
        db_ctx.db_pool().write(),
        &ExtensionRegistry::discover()?,
        args,
        config,
    )
    .await
}

async fn run_migrate_repair(
    write_provider: &dyn DatabaseProvider,
    registry: &ExtensionRegistry,
    args: RepairArgs<'_>,
    config: &CliConfig,
) -> Result<()> {
    let RepairArgs {
        extension,
        apply,
        json,
    } = args;
    let extensions = select_extensions(registry, extension)?;
    let migration_service = MigrationService::new(write_provider);

    let mut drift_rows: Vec<MigrationDriftInfo> = Vec::new();
    let mut migrations_run = 0usize;

    for ext in &extensions {
        if apply {
            let result = migration_service
                .repair_drift(ext.as_ref())
                .await
                .map_err(|e| anyhow!("Failed to repair migrations: {}", e))?;
            migrations_run += result.migrations_run;
            for d in result.repaired {
                drift_rows.push(MigrationDriftInfo {
                    extension_id: d.extension_id,
                    version: d.version,
                    name: d.name,
                    stored_checksum: d.stored_checksum,
                    current_checksum: d.current_checksum,
                });
            }
        } else {
            let status = migration_service
                .status(ext.as_ref())
                .await
                .map_err(|e| anyhow!("Failed to get migration status: {}", e))?;
            for d in status.drift {
                drift_rows.push(MigrationDriftInfo {
                    extension_id: d.extension_id,
                    version: d.version,
                    name: d.name,
                    stored_checksum: d.stored_checksum,
                    current_checksum: d.current_checksum,
                });
            }
        }
    }

    drift_rows.sort_by(|a, b| {
        a.extension_id
            .cmp(&b.extension_id)
            .then(a.version.cmp(&b.version))
    });

    let output = MigrateRepairOutput {
        applied: apply,
        drift: drift_rows,
        migrations_run,
    };

    if json || config.is_json_output() {
        let result = CommandOutput::card_value("Migration Repair", &output);
        render_result(&result);
    } else {
        render_repair_text(&output);
    }

    Ok(())
}

fn render_repair_text(output: &MigrateRepairOutput) {
    if output.drift.is_empty() {
        CliService::success("No checksum drift — nothing to repair.");
        return;
    }

    let header = if output.applied {
        "Repaired migration(s):"
    } else {
        "Drifted migration(s):"
    };
    CliService::info(header);
    for d in &output.drift {
        CliService::info(&format!(
            "  {} v{:03} {}: stored={} current={}",
            d.extension_id,
            d.version,
            d.name,
            &d.stored_checksum[..d.stored_checksum.len().min(8)],
            &d.current_checksum[..d.current_checksum.len().min(8)]
        ));
    }
    CliService::info("");

    if output.applied {
        CliService::success(&format!(
            "Repaired {} drifted migration(s); {} migration(s) re-applied. Drift: 0",
            output.drift.len(),
            output.migrations_run
        ));
    } else {
        CliService::warning(&format!(
            "{} migration(s) drifted. Re-run with --apply to repair.",
            output.drift.len()
        ));
    }
}
