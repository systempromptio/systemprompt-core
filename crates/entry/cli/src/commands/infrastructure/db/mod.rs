//! `db` CLI command group: schema inspection, queries, and migration tooling.
//!
//! [`execute`] runs commands against a freshly opened [`AppContext`], while
//! [`execute_with_db`] reuses a caller-supplied [`DatabaseContext`] for the
//! standalone (profile-less) path. Subcommands cover ad-hoc queries, schema
//! introspection, migration apply/down/repair/squash, and the schema doctor.

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
use systemprompt_database::{DatabaseAdminService, QueryExecutor};
use systemprompt_runtime::{AppContext, DatabaseContext};

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use dispatch::{dispatch_profile_migration, dispatch_standalone_migration};

pub use commands::{DbCommands, MigrationsCommands};
pub use types::*;

struct DatabaseTool {
    ctx: AppContext,
    admin_service: DatabaseAdminService,
    query_executor: QueryExecutor,
}

impl DatabaseTool {
    async fn new() -> Result<Self> {
        let ctx = AppContext::new()
            .await
            .context("Failed to connect to database. Check your profile configuration.")?;
        let pool = ctx.db_pool().write_pool_arc()?;
        let admin_service = DatabaseAdminService::new(Arc::clone(&pool));
        let query_executor = QueryExecutor::new(pool);
        Ok(Self {
            ctx,
            admin_service,
            query_executor,
        })
    }
}

pub async fn execute(cmd: DbCommands, config: &CliConfig) -> Result<()> {
    let Some(cmd) = dispatch_profile_migration(cmd, config).await? else {
        return Ok(());
    };

    let db = DatabaseTool::new().await?;

    match cmd {
        DbCommands::Query {
            sql,
            limit,
            offset,
            format: _,
        } => run_query(&db.query_executor, &sql, limit, offset, config).await,
        DbCommands::Execute { sql, format: _ } => run_write(&db.query_executor, &sql, config).await,
        DbCommands::Tables { filter } => {
            schema::execute_tables(&db.admin_service, filter, config).await
        },
        DbCommands::Describe { table_name } => {
            schema::execute_describe(&db.admin_service, &table_name, config).await
        },
        DbCommands::Info => schema::execute_info(&db.admin_service, config).await,
        DbCommands::Migrate { .. }
        | DbCommands::MigrateDown { .. }
        | DbCommands::MigrateSquash { .. }
        | DbCommands::MigrateRepair { .. }
        | DbCommands::MigrateMarkApplied { .. } => unreachable!(),
        DbCommands::Migrations { cmd } => admin::execute_migrations(&db.ctx, cmd, config).await,
        DbCommands::MigratePlan { extension, json } => {
            admin::execute_migrate_plan(&db.ctx, extension.as_deref(), json, config).await
        },
        DbCommands::MigrateStatus { extension, json } => {
            admin::execute_migrate_status(&db.ctx, extension.as_deref(), json, config).await
        },
        DbCommands::AssignAdmin { user } => {
            admin::execute_assign_admin(&db.ctx, &user, config).await
        },
        DbCommands::Status => admin::execute_status(&db.admin_service, config).await,
        DbCommands::Validate => schema::execute_validate(&db.admin_service, config).await,
        DbCommands::Count { table_name } => {
            schema::execute_count(&db.admin_service, &table_name, config).await
        },
        DbCommands::Indexes { table } => {
            introspect::execute_indexes(&db.admin_service, table, config).await
        },
        DbCommands::Size => introspect::execute_size(&db.admin_service, config).await,
        DbCommands::Doctor => doctor::execute_doctor(db.ctx.db_pool(), config).await,
    }
}

pub async fn execute_with_db(
    cmd: DbCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx
        .db_pool()
        .write_pool_arc()
        .context("Database must be PostgreSQL")?;
    let admin_service = DatabaseAdminService::new(Arc::clone(&pool));
    let query_executor = QueryExecutor::new(pool);

    let Some(cmd) = dispatch_standalone_migration(cmd, db_ctx, config).await? else {
        return Ok(());
    };

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
        | DbCommands::MigrateMarkApplied { .. }
        | DbCommands::Migrations { .. }
        | DbCommands::MigratePlan { .. }
        | DbCommands::MigrateStatus { .. } => unreachable!(),
        DbCommands::AssignAdmin { .. } => {
            bail!("assign-admin requires full profile context")
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
        DbCommands::Doctor => doctor::execute_doctor(db_ctx.db_pool(), config).await,
    }
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
    render_result(&result);
    Ok(())
}

async fn run_write(executor: &QueryExecutor, sql: &str, config: &CliConfig) -> Result<()> {
    let result = query::execute_write(executor, sql, config).await?;
    render_result(&result);
    Ok(())
}
