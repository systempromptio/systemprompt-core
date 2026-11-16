//! `db` CLI command group: schema inspection, queries, and migration tooling.
//!
//! [`execute`] runs commands against the command host: migration variants
//! are routed to the profile or standalone dispatcher depending on whether the
//! invocation is database-scoped, and the remaining subcommands share the
//! host's pool. Subcommands cover ad-hoc queries, schema introspection,
//! migration apply/down/repair, and the schema doctor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admin;
mod admin_migrate;
mod admin_migrate_down;
mod admin_migrate_mark_applied;
mod admin_migrate_plan;
mod admin_migrate_repair;
mod admin_migrate_status;
mod admin_migrations;
mod commands;
mod dispatch;
mod doctor;
mod introspect;
mod query;
mod schema;
pub mod text_format;
mod types;

use anyhow::{Context, Result, bail};
use std::sync::Arc;
use systemprompt_database::{DatabaseAdminService, DbPool, QueryExecutor, pool_from_handle};
use systemprompt_runtime::DatabaseContext;

use crate::commands::shared::app_context;
use crate::interactive::require_confirmation;
use crate::kernel::KernelHost;
use crate::shared::CommandOutput;
use dispatch::{Dispatched, dispatch_profile_migration, dispatch_standalone_migration};

pub use commands::{DbCommands, MigrationsCommands};
pub use types::{
    AppliedMigrationInfo, ColumnInfo, DbAssignAdminOutput, DbCountOutput, DbDescribeOutput,
    DbExecuteOutput, DbIndexesOutput, DbInfoOutput, DbMigrateDownOutput, DbMigrateOutput,
    DbSizeOutput, DbStatusOutput, DbTablesOutput, ExtensionMigrationStatus, IndexInfo,
    MigrateMarkAppliedOutput, MigratePlanOutput, MigrateRepairOutput, MigrateStatusOutput,
    MigrateStatusRow, MigrationCollisionInfo, MigrationDriftInfo, MigrationHistoryOutput,
    MigrationStatusOutput, PendingMigrationInfo, TableIndexInfo, TableInfo, TableSizeInfo,
};

pub async fn execute(cmd: DbCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    let config = host.output();
    let dispatched = if host.is_database_scoped() {
        let db_ctx = DatabaseContext::from_pool(pool_from_handle(host.db().await?)?);
        dispatch_standalone_migration(cmd, &db_ctx, host).await?
    } else {
        dispatch_profile_migration(cmd, host).await?
    };
    let cmd = match dispatched {
        Dispatched::Done(output) => return Ok(output),
        Dispatched::Unhandled(cmd) => cmd,
    };

    let (pool, admin_service, query_executor) = connect_services(host).await?;

    match cmd {
        DbCommands::Query { sql, limit, offset } => {
            let params = query::QueryParams::new(&sql, limit, offset);
            query::execute_query(&query_executor, &admin_service, &params).await
        },
        DbCommands::Execute { sql, yes } => {
            require_confirmation(
                host.prompter(),
                &format!("Execute this write against the database?\n  {sql}"),
                yes,
                config,
            )?;
            query::execute_write(&query_executor, &sql).await
        },
        DbCommands::Tables { filter, exact } => {
            schema::execute_tables(&admin_service, filter, exact).await
        },
        DbCommands::Describe { table_name } => {
            schema::execute_describe(&admin_service, &table_name).await
        },
        DbCommands::Info => schema::execute_info(&admin_service).await,
        DbCommands::Migrate { .. }
        | DbCommands::MigrateDown { .. }
        | DbCommands::MigrateRepair { .. }
        | DbCommands::MigrateMarkApplied { .. } => {
            bail!("migration command was not consumed by the migration dispatcher")
        },
        DbCommands::Migrations { cmd } => {
            admin::execute_migrations(&app_context(host).await?, host.admitted()?.built(), cmd)
                .await
        },
        DbCommands::MigratePlan { extension, flags } => {
            admin::execute_migrate_plan(
                &app_context(host).await?,
                host.admitted()?.built(),
                extension.as_ref(),
                flags,
            )
            .await
        },
        DbCommands::MigrateStatus { extension } => {
            admin::execute_migrate_status(
                &app_context(host).await?,
                host.admitted()?.built(),
                extension.as_ref(),
            )
            .await
        },
        DbCommands::AssignAdmin { user, .. } => {
            if host.is_database_scoped() {
                bail!("assign-admin requires full profile context");
            }
            admin::execute_assign_admin(&app_context(host).await?, &user).await
        },
        DbCommands::Status => admin::execute_status(&admin_service).await,
        DbCommands::Count { table_name } => {
            schema::execute_count(&admin_service, &table_name).await
        },
        DbCommands::Indexes { table } => introspect::execute_indexes(&admin_service, table).await,
        DbCommands::Size => introspect::execute_size(&admin_service).await,
        DbCommands::Doctor => doctor::execute_doctor(&pool, host.admitted()?.built()).await,
    }
}

async fn connect_services(
    host: &dyn KernelHost,
) -> Result<(DbPool, DatabaseAdminService, QueryExecutor)> {
    let pool = pool_from_handle(host.db().await?)
        .context("Failed to connect to database. Check your profile configuration.")?;
    let write_pool = pool.write_pool();
    let admin_service = DatabaseAdminService::new(Arc::clone(&write_pool));
    let query_executor = QueryExecutor::new(write_pool);
    Ok((pool, admin_service, query_executor))
}
