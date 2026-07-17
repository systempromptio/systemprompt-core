//! Migration subcommand dispatch for the `db` command group.
//!
//! Each dispatcher consumes and executes the migration variants it owns,
//! returning `Ok(None)` once handled or `Ok(Some(cmd))` to hand any other
//! command back to the caller's match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_runtime::DatabaseContext;

use super::commands::DbCommands;
use super::{admin, admin_squash};
use crate::cli_settings::CliConfig;

pub(super) async fn dispatch_profile_migration(
    cmd: DbCommands,
    config: &CliConfig,
) -> Result<Option<DbCommands>> {
    match cmd {
        DbCommands::Migrate {
            allow_checksum_drift,
        } => admin::execute_migrate(config, allow_checksum_drift)
            .await
            .map(|()| None),
        DbCommands::MigrateDown { extension, count } => {
            admin::execute_migrate_down(config, &extension, count)
                .await
                .map(|()| None)
        },
        DbCommands::MigrateRepair {
            extension,
            apply,
            json,
        } => admin::execute_migrate_repair(
            config,
            admin::RepairArgs {
                extension: extension.as_deref(),
                apply,
                json,
            },
        )
        .await
        .map(|()| None),
        DbCommands::MigrateMarkApplied {
            extension,
            version,
            json,
        } => admin::execute_migrate_mark_applied(
            config,
            admin::MarkAppliedArgs {
                extension: &extension,
                version,
                json,
            },
        )
        .await
        .map(|()| None),
        DbCommands::MigrateSquash {
            extension,
            through,
            apply,
        } => admin_squash::execute_squash(
            config,
            admin_squash::SquashArgs {
                extension: &extension,
                through,
                apply,
            },
        )
        .await
        .map(|()| None),
        other => Ok(Some(other)),
    }
}

pub(super) async fn dispatch_standalone_migration(
    cmd: DbCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<Option<DbCommands>> {
    match cmd {
        DbCommands::Migrate {
            allow_checksum_drift,
        } => admin::execute_migrate_standalone(db_ctx, config, allow_checksum_drift)
            .await
            .map(|()| None),
        DbCommands::MigrateDown { extension, count } => {
            admin::execute_migrate_down_standalone(db_ctx, config, &extension, count)
                .await
                .map(|()| None)
        },
        DbCommands::MigrateSquash {
            extension,
            through,
            apply,
        } => admin_squash::execute_squash_standalone(
            db_ctx,
            config,
            admin_squash::SquashArgs {
                extension: &extension,
                through,
                apply,
            },
        )
        .await
        .map(|()| None),
        DbCommands::MigrateRepair {
            extension,
            apply,
            json,
        } => admin::execute_migrate_repair_standalone(
            db_ctx,
            config,
            admin::RepairArgs {
                extension: extension.as_deref(),
                apply,
                json,
            },
        )
        .await
        .map(|()| None),
        DbCommands::MigrateMarkApplied {
            extension,
            version,
            json,
        } => admin::execute_migrate_mark_applied_standalone(
            db_ctx,
            config,
            admin::MarkAppliedArgs {
                extension: &extension,
                version,
                json,
            },
        )
        .await
        .map(|()| None),
        other => dispatch_standalone_inspection(other, db_ctx, config).await,
    }
}

async fn dispatch_standalone_inspection(
    cmd: DbCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<Option<DbCommands>> {
    match cmd {
        DbCommands::Migrations { cmd } => admin::execute_migrations_standalone(db_ctx, cmd, config)
            .await
            .map(|()| None),
        DbCommands::MigratePlan { extension, json } => {
            admin::execute_migrate_plan_standalone(db_ctx, extension.as_deref(), json, config)
                .await
                .map(|()| None)
        },
        DbCommands::MigrateStatus { extension, json } => {
            admin::execute_migrate_status_standalone(db_ctx, extension.as_deref(), json, config)
                .await
                .map(|()| None)
        },
        other => Ok(Some(other)),
    }
}
