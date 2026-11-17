//! `db migrations` subcommand group: aggregate status and per-extension
//! history.
//!
//! Dispatches the [`MigrationsCommands`] variants, summarising defined vs.
//! applied vs. pending migrations across registered extensions. Provides both
//! the full-context and standalone (`DatabaseContext`-only) entry points.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_database::MigrationService;
use systemprompt_extension::BuiltState;
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::DatabaseContext;

use systemprompt_extension::cli::CommandOutput;

use super::MigrationsCommands;
use super::types::{
    AppliedMigrationInfo, ExtensionMigrationStatus, MigrationHistoryOutput, MigrationStatusOutput,
};

pub(super) async fn execute_migrations(
    ctx: &systemprompt_runtime::AppContext,
    built: &BuiltState,
    cmd: MigrationsCommands,
) -> Result<CommandOutput> {
    run_migrations(ctx.db_pool().as_ref(), built, cmd).await
}

pub(super) async fn execute_migrations_standalone(
    db_ctx: &DatabaseContext,
    built: &BuiltState,
    cmd: MigrationsCommands,
) -> Result<CommandOutput> {
    run_migrations(db_ctx.db_pool().as_ref(), built, cmd).await
}

async fn run_migrations(
    db: &dyn systemprompt_database::services::DatabaseProvider,
    built: &BuiltState,
    cmd: MigrationsCommands,
) -> Result<CommandOutput> {
    match cmd {
        MigrationsCommands::Status => execute_migrations_status(db, built).await,
        MigrationsCommands::History { extension } => {
            execute_migrations_history(db, built, &extension).await
        },
    }
}

async fn execute_migrations_status(
    db: &dyn systemprompt_database::services::DatabaseProvider,
    built: &BuiltState,
) -> Result<CommandOutput> {
    let migration_service = MigrationService::new(db);
    let mut extensions = Vec::new();
    let mut total_pending = 0;
    let mut total_applied = 0;

    for ext in super::admin_migrate::install_set(built) {
        let status: systemprompt_database::MigrationStatus = migration_service
            .get_migration_status(ext.as_ref())
            .await
            .context("Failed to get migration status")?;

        total_pending += status.pending_count;
        total_applied += status.total_applied;

        extensions.push(ExtensionMigrationStatus {
            extension_id: ExtensionId::new(status.extension_id),
            total_defined: status.total_defined,
            total_applied: status.total_applied,
            pending_count: status.pending_count,
            pending_versions: status.pending.iter().map(|m| m.version).collect(),
        });
    }

    let output = MigrationStatusOutput {
        extensions,
        total_pending,
        total_applied,
    };

    let summary = if total_pending == 0 {
        NoticeLine::new("success", "All migrations are up to date")
    } else {
        NoticeLine::new("warning", format!("{total_pending} pending migration(s)"))
    };
    Ok(CommandOutput::table_of(
        vec![
            "extension_id",
            "total_defined",
            "total_applied",
            "pending_count",
        ],
        &output.extensions,
    )
    .with_title("Migration Status")
    .with_note(summary))
}

async fn execute_migrations_history(
    db: &dyn systemprompt_database::services::DatabaseProvider,
    built: &BuiltState,
    extension_id: &ExtensionId,
) -> Result<CommandOutput> {
    let ext = super::admin_migrate::find_member(built, extension_id)?;

    let migration_service = MigrationService::new(db);
    let applied: Vec<systemprompt_database::AppliedMigration> = migration_service
        .get_applied_migrations(extension_id)
        .await
        .context("Failed to get migration history")?;

    let migrations: Vec<AppliedMigrationInfo> = applied
        .into_iter()
        .map(|m| AppliedMigrationInfo {
            version: m.version,
            name: m.name,
            checksum: m.checksum,
            applied_at: None,
        })
        .collect();

    let output = MigrationHistoryOutput {
        extension_id: extension_id.clone(),
        migrations,
    };

    let note = if output.migrations.is_empty() {
        NoticeLine::new("info", "No migrations applied yet")
    } else {
        NoticeLine::new("info", format!("Extension version: {}", ext.version()))
    };
    Ok(
        CommandOutput::table_of(vec!["version", "name", "checksum"], &output.migrations)
            .with_title(format!("Migration history for '{extension_id}'"))
            .with_note(note),
    )
}
