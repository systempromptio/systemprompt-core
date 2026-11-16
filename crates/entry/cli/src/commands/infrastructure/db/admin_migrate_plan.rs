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

use anyhow::{Context, Result};
use systemprompt_database::MigrationService;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_extension::BuiltState;
use systemprompt_identifiers::ExtensionId;
use systemprompt_extension::cli::CommandError;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::{AppContext, DatabaseContext};

use crate::shared::CommandOutput;

use super::admin_migrate::select_extensions;
use super::commands::PlanFlags;
use super::types::{MigratePlanOutput, PendingMigrationInfo};

pub(super) async fn execute_migrate_plan(
    ctx: &AppContext,
    built: &BuiltState,
    extension: Option<&ExtensionId>,
    flags: PlanFlags,
) -> Result<CommandOutput> {
    run_migrate_plan(ctx.db_pool().as_ref(), built, extension, flags).await
}

pub(super) async fn execute_migrate_plan_standalone(
    db_ctx: &DatabaseContext,
    built: &BuiltState,
    extension: Option<&ExtensionId>,
    flags: PlanFlags,
) -> Result<CommandOutput> {
    run_migrate_plan(db_ctx.db_pool().as_ref(), built, extension, flags).await
}

async fn run_migrate_plan(
    db: &dyn DatabaseProvider,
    built: &BuiltState,
    extension: Option<&ExtensionId>,
    flags: PlanFlags,
) -> Result<CommandOutput> {
    let extensions = select_extensions(built, extension)?;
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

    let result = plan_output(&output);

    if flags.require_rolling_safe && !output.rolling_safe {
        let breaking: Vec<String> = output
            .pending
            .iter()
            .filter(|p| p.breaking)
            .map(|p| format!("{}:{:03} {}", p.extension_id, p.version, p.name))
            .collect();
        return Err(CommandError::new(format!(
            "migration plan is not rolling-safe: {} is marked `@compat: breaking`",
            breaking.join(", ")
        ))
        .with_output(result)
        .into());
    }

    Ok(result)
}

fn plan_output(output: &MigratePlanOutput) -> CommandOutput {
    let table = CommandOutput::table_of(
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
    if output.pending.is_empty() {
        return table.with_note(NoticeLine::new("success", "No pending migrations"));
    }
    let rolling = if output.rolling_safe {
        NoticeLine::new(
            "info",
            "rolling-safe: yes (the previous release can serve this schema)",
        )
    } else {
        NoticeLine::new(
            "warning",
            "rolling-safe: no (a `@compat: breaking` migration is pending; stop old pods first)",
        )
    };
    table
        .with_note(NoticeLine::new(
            "info",
            format!("{} pending migration(s)", output.total_pending),
        ))
        .with_note(rolling)
}
