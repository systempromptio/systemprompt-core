//! Migration subcommand dispatch for the `db` command group.
//!
//! Each dispatcher consumes and executes the migration variants it owns,
//! returning the command's output once handled or handing any other command
//! back to the caller's match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_extension::BuiltState;
use systemprompt_runtime::DatabaseContext;

use super::admin;
use super::commands::DbCommands;
use crate::kernel::KernelHost;
use systemprompt_extension::cli::CommandOutput;

pub(super) enum Dispatched {
    Done(CommandOutput),
    Unhandled(DbCommands),
}

pub(super) async fn dispatch_profile_migration(
    cmd: DbCommands,
    host: &dyn KernelHost,
) -> Result<Dispatched> {
    let output = match cmd {
        DbCommands::Migrate {
            allow_checksum_drift,
            repair_drift,
            full,
        } => {
            admin::execute_migrate(
                host,
                admin::MigrateOptions {
                    allow_checksum_drift,
                    repair_drift,
                    full,
                },
            )
            .await?
        },
        DbCommands::MigrateDown {
            extension, count, ..
        } => admin::execute_migrate_down(host, &extension, count).await?,
        DbCommands::MigrateRepair {
            extension,
            apply,
            reconcile_only,
        } => {
            admin::execute_migrate_repair(
                host,
                admin::RepairArgs {
                    extension: extension.as_ref(),
                    apply,
                    reconcile_only,
                },
            )
            .await?
        },
        DbCommands::MigrateMarkApplied {
            extension, version, ..
        } => {
            admin::execute_migrate_mark_applied(
                host,
                admin::MarkAppliedArgs {
                    extension: &extension,
                    version,
                },
            )
            .await?
        },
        other => return Ok(Dispatched::Unhandled(other)),
    };
    Ok(Dispatched::Done(output))
}

pub(super) async fn dispatch_standalone_migration(
    cmd: DbCommands,
    db_ctx: &DatabaseContext,
    host: &dyn KernelHost,
) -> Result<Dispatched> {
    let output = match cmd {
        DbCommands::Migrate {
            allow_checksum_drift,
            repair_drift,
            full,
        } => {
            admin::execute_migrate_standalone(
                db_ctx,
                built(host)?,
                host.output(),
                admin::MigrateOptions {
                    allow_checksum_drift,
                    repair_drift,
                    full,
                },
            )
            .await?
        },
        DbCommands::MigrateDown {
            extension, count, ..
        } => {
            admin::execute_migrate_down_standalone(db_ctx, built(host)?, &extension, count).await?
        },
        DbCommands::MigrateRepair {
            extension,
            apply,
            reconcile_only,
        } => {
            admin::execute_migrate_repair_standalone(
                db_ctx,
                built(host)?,
                admin::RepairArgs {
                    extension: extension.as_ref(),
                    apply,
                    reconcile_only,
                },
            )
            .await?
        },
        DbCommands::MigrateMarkApplied {
            extension, version, ..
        } => {
            admin::execute_migrate_mark_applied_standalone(
                db_ctx,
                built(host)?,
                admin::MarkAppliedArgs {
                    extension: &extension,
                    version,
                },
            )
            .await?
        },
        other => return dispatch_standalone_inspection(other, db_ctx, host).await,
    };
    Ok(Dispatched::Done(output))
}

async fn dispatch_standalone_inspection(
    cmd: DbCommands,
    db_ctx: &DatabaseContext,
    host: &dyn KernelHost,
) -> Result<Dispatched> {
    let output = match cmd {
        DbCommands::Migrations { cmd } => {
            admin::execute_migrations_standalone(db_ctx, built(host)?, cmd).await?
        },
        DbCommands::MigratePlan { extension, flags } => {
            admin::execute_migrate_plan_standalone(db_ctx, built(host)?, extension.as_ref(), flags)
                .await?
        },
        DbCommands::MigrateStatus { extension } => {
            admin::execute_migrate_status_standalone(db_ctx, built(host)?, extension.as_ref())
                .await?
        },
        other => return Ok(Dispatched::Unhandled(other)),
    };
    Ok(Dispatched::Done(output))
}

fn built(host: &dyn KernelHost) -> Result<&BuiltState> {
    Ok(host.admitted()?.built())
}
