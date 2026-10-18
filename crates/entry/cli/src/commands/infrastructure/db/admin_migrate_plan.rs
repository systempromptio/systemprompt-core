//! `infra db migrate plan` command printing pending migrations.
//!
//! The plan is *rolling-safe* when no pending migration is marked
//! `@compat: breaking`: the previous release keeps serving the migrated
//! schema, so a rolling update is safe. Otherwise old pods must stop before
//! the migration runs; `--require-rolling-safe` turns that into a non-zero
//! exit for a pipeline choosing between `RollingUpdate` and `Recreate`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, bail};
use systemprompt_database::MigrationService;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::ExtensionId;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use crate::cli_settings::CliConfig;
use crate::shared::{CommandOutput, render_result};

use super::admin_migrate::select_extensions;
use super::commands::PlanFlags;
use super::types::{MigratePlanOutput, PendingMigrationInfo};

pub(super) async fn execute_migrate_plan(
    ctx: &AppContext,
    extension: Option<&ExtensionId>,
    flags: PlanFlags,
    config: &CliConfig,
) -> Result<()> {
    let db = ctx.db_pool();
    let registry = ctx.extension_registry();
    run_migrate_plan(db.as_ref(), registry, extension, flags, config).await
}

pub(super) async fn execute_migrate_plan_standalone(
    db_ctx: &DatabaseContext,
    extension: Option<&ExtensionId>,
    flags: PlanFlags,
    config: &CliConfig,
) -> Result<()> {
    let db = db_ctx.db_pool();
    let registry = ExtensionRegistry::discover()?;
    run_migrate_plan(db.as_ref(), &registry, extension, flags, config).await
}

async fn run_migrate_plan(
    db: &dyn DatabaseProvider,
    registry: &ExtensionRegistry,
    extension: Option<&ExtensionId>,
    flags: PlanFlags,
    config: &CliConfig,
) -> Result<()> {
    let extensions = select_extensions(registry, extension)?;
    let migration_service = MigrationService::new(db);

    let mut pending_rows: Vec<PendingMigrationInfo> = Vec::new();
    for ext in &extensions {
        let pending = migration_service
            .plan_pending(ext.as_ref())
            .await
            .context("Failed to plan migrations")?;
        for p in pending {
            pending_rows.push(PendingMigrationInfo {
                extension_id: ExtensionId::new(p.extension_id),
                version: p.version,
                name: p.name,
                checksum: p.checksum,
                no_tx: p.no_tx,
                breaking: p.breaks_older,
            });
        }
    }

    let total_pending = pending_rows.len();
    let rolling_safe = pending_rows.iter().all(|p| !p.breaking);
    let output = MigratePlanOutput {
        pending: pending_rows,
        total_pending,
        rolling_safe,
    };

    render_plan(&output, flags, config);

    if flags.require_rolling_safe && !output.rolling_safe {
        let breaking: Vec<String> = output
            .pending
            .iter()
            .filter(|p| p.breaking)
            .map(|p| format!("{}:{:03} {}", p.extension_id, p.version, p.name))
            .collect();
        bail!(
            "migration plan is not rolling-safe: {} is marked `@compat: breaking`",
            breaking.join(", ")
        );
    }

    Ok(())
}

fn render_plan(output: &MigratePlanOutput, flags: PlanFlags, config: &CliConfig) {
    if flags.json || config.is_json_output() {
        let result = CommandOutput::table_of(
            vec![
                "extension_id",
                "version",
                "name",
                "checksum",
                "no_tx",
                "breaking",
            ],
            &output.pending,
        )
        .with_title("Migration Plan");
        render_result(&result, config);
    } else if output.pending.is_empty() {
        CliService::success("No pending migrations");
    } else {
        CliService::info(&format!("{} pending migration(s):", output.total_pending));
        CliService::info("");
        CliService::info(&format!(
            "  {:<24} {:>7} {:<32} {:<10} {:<6} {}",
            "EXTENSION", "VERSION", "NAME", "CHECKSUM", "NO_TX", "BREAKING"
        ));
        for p in &output.pending {
            let checksum_short = if p.checksum.len() >= 8 {
                &p.checksum[..8]
            } else {
                &p.checksum
            };
            CliService::info(&format!(
                "  {:<24} {:>7} {:<32} {:<10} {:<6} {}",
                p.extension_id, p.version, p.name, checksum_short, p.no_tx, p.breaking
            ));
        }
        CliService::info("");
        if output.rolling_safe {
            CliService::info("rolling-safe: yes (the previous release can serve this schema)");
        } else {
            CliService::warning(
                "rolling-safe: no (a `@compat: breaking` migration is pending; stop old pods \
                 first)",
            );
        }
    }
}
