//! `db` CLI command group: schema inspection, queries, and migration tooling.
//!
//! [`execute`] runs commands against the invocation's
//! [`CommandContext`](crate::context::CommandContext): migration variants are
//! routed to the profile or standalone dispatcher depending on whether the
//! invocation is database-scoped, and the remaining subcommands share the
//! context's pool. Subcommands cover ad-hoc queries, schema introspection,
//! migration apply/down/repair/squash, and the schema doctor.

mod admin;
mod admin_migrate;
mod admin_migrate_down;
mod admin_migrate_mark_applied;
mod admin_migrate_plan;
mod admin_migrate_repair;
mod admin_migrate_status;
mod admin_migrations;
mod admin_squash;
mod commands;
mod dispatch;
mod doctor;
mod helpers;
mod introspect;
mod query;
mod schema;
mod types;

use anyhow::{Context, Result, bail};
use std::sync::Arc;
use systemprompt_database::{DatabaseAdminService, DbPool, QueryExecutor};

use crate::cli_settings::CliConfig;
use crate::context::CommandContext;
use crate::shared::render_result;
use dispatch::{dispatch_profile_migration, dispatch_standalone_migration};

pub use commands::{DbCommands, MigrationsCommands};
pub use types::*;

pub async fn execute(cmd: DbCommands, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    let Some(cmd) = (match ctx.database_context() {
        Some(db_ctx) => dispatch_standalone_migration(cmd, db_ctx, config).await?,
        None => dispatch_profile_migration(cmd, config).await?,
    }) else {
        return Ok(());
    };

    let (pool, admin_service, query_executor) = connect_services(ctx).await?;

    match cmd {
        DbCommands::Query {
            sql,
            limit,
            offset,
            format: _,
        } => run_query(&query_executor, &sql, limit, offset, config).await,
        DbCommands::Execute { sql, format: _ } => run_write(&query_executor, &sql, config).await,
        DbCommands::Tables { filter } => {
            schema::execute_tables(&admin_service, filter, config).await
        },
        DbCommands::Describe { table_name } => {
            schema::execute_describe(&admin_service, &table_name, config).await
        },
        DbCommands::Info => schema::execute_info(&admin_service, config).await,
        DbCommands::Migrate { .. }
        | DbCommands::MigrateDown { .. }
        | DbCommands::MigrateSquash { .. }
        | DbCommands::MigrateRepair { .. }
        | DbCommands::MigrateMarkApplied { .. } => unreachable!(),
        DbCommands::Migrations { cmd } => {
            admin::execute_migrations(ctx.app_context().await?, cmd, config).await
        },
        DbCommands::MigratePlan { extension, json } => {
            admin::execute_migrate_plan(
                ctx.app_context().await?,
                extension.as_deref(),
                json,
                config,
            )
            .await
        },
        DbCommands::MigrateStatus { extension, json } => {
            admin::execute_migrate_status(
                ctx.app_context().await?,
                extension.as_deref(),
                json,
                config,
            )
            .await
        },
        DbCommands::AssignAdmin { user } => {
            if ctx.is_database_scoped() {
                bail!("assign-admin requires full profile context");
            }
            admin::execute_assign_admin(ctx.app_context().await?, &user, config).await
        },
        DbCommands::Status => admin::execute_status(&admin_service, config).await,
        DbCommands::Validate => schema::execute_validate(&admin_service, config).await,
        DbCommands::Count { table_name } => {
            schema::execute_count(&admin_service, &table_name, config).await
        },
        DbCommands::Indexes { table } => {
            introspect::execute_indexes(&admin_service, table, config).await
        },
        DbCommands::Size => introspect::execute_size(&admin_service, config).await,
        DbCommands::Doctor => doctor::execute_doctor(&pool, config).await,
    }
}

async fn connect_services(
    ctx: &CommandContext,
) -> Result<(DbPool, DatabaseAdminService, QueryExecutor)> {
    let pool = ctx
        .db_pool()
        .await
        .context("Failed to connect to database. Check your profile configuration.")?;
    let write_pool = pool
        .write_pool_arc()
        .context("Database must be PostgreSQL")?;
    let admin_service = DatabaseAdminService::new(Arc::clone(&write_pool));
    let query_executor = QueryExecutor::new(write_pool);
    Ok((pool, admin_service, query_executor))
}

async fn run_query(
    executor: &QueryExecutor,
    sql: &str,
    limit: Option<u32>,
    offset: Option<u32>,
    config: &CliConfig,
) -> Result<()> {
    let params = query::QueryParams { sql, limit, offset };
    let result = query::execute_query(executor, &params, config).await?;
    render_result(&result, config);
    Ok(())
}

async fn run_write(executor: &QueryExecutor, sql: &str, config: &CliConfig) -> Result<()> {
    let result = query::execute_write(executor, sql, config).await?;
    render_result(&result, config);
    Ok(())
}
