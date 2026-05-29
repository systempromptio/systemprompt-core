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
    if let DbCommands::Migrate {
        allow_checksum_drift,
    } = cmd
    {
        return admin::execute_migrate(config, allow_checksum_drift).await;
    }

    if let DbCommands::MigrateDown { extension, count } = cmd {
        return admin::execute_migrate_down(config, &extension, count).await;
    }

    if let DbCommands::MigrateRepair {
        extension,
        apply,
        json,
    } = cmd
    {
        return admin::execute_migrate_repair(
            config,
            admin::RepairArgs {
                extension: extension.as_deref(),
                apply,
                json,
            },
        )
        .await;
    }

    if let DbCommands::MigrateMarkApplied {
        extension,
        version,
        json,
    } = cmd
    {
        return admin::execute_migrate_mark_applied(
            config,
            admin::MarkAppliedArgs {
                extension: &extension,
                version,
                json,
            },
        )
        .await;
    }

    if let DbCommands::MigrateSquash {
        extension,
        through,
        apply,
    } = cmd
    {
        return admin_squash::execute_squash(
            config,
            admin_squash::SquashArgs {
                extension: &extension,
                through,
                apply,
            },
        )
        .await;
    }

    let db = DatabaseTool::new().await?;

    match cmd {
        DbCommands::Query {
            sql,
            limit,
            offset,
            format: _,
        } => {
            let params = query::QueryParams {
                sql: &sql,
                limit,
                offset,
            };
            let result = query::execute_query(&db.query_executor, &params, config).await?;
            render_result(&result);
            Ok(())
        },
        DbCommands::Execute { sql, format: _ } => {
            let result = query::execute_write(&db.query_executor, &sql, config).await?;
            render_result(&result);
            Ok(())
        },
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

    match cmd {
        DbCommands::Query {
            sql,
            limit,
            offset,
            format: _,
        } => {
            let params = query::QueryParams {
                sql: &sql,
                limit,
                offset,
            };
            let result = query::execute_query(&query_executor, &params, config).await?;
            render_result(&result);
            Ok(())
        },
        DbCommands::Execute { sql, format: _ } => {
            let result = query::execute_write(&query_executor, &sql, config).await?;
            render_result(&result);
            Ok(())
        },
        DbCommands::Tables { filter } => {
            schema::execute_tables(&admin_service, filter, config).await
        },
        DbCommands::Describe { table_name } => {
            schema::execute_describe(&admin_service, &table_name, config).await
        },
        DbCommands::Info => schema::execute_info(&admin_service, config).await,
        DbCommands::Migrate {
            allow_checksum_drift,
        } => admin::execute_migrate_standalone(db_ctx, config, allow_checksum_drift).await,
        DbCommands::MigrateDown { extension, count } => {
            admin::execute_migrate_down_standalone(db_ctx, config, &extension, count).await
        },
        DbCommands::MigrateSquash {
            extension,
            through,
            apply,
        } => {
            admin_squash::execute_squash_standalone(
                db_ctx,
                config,
                admin_squash::SquashArgs {
                    extension: &extension,
                    through,
                    apply,
                },
            )
            .await
        },
        DbCommands::Migrations { cmd } => {
            admin::execute_migrations_standalone(db_ctx, cmd, config).await
        },
        DbCommands::MigratePlan { extension, json } => {
            admin::execute_migrate_plan_standalone(db_ctx, extension.as_deref(), json, config).await
        },
        DbCommands::MigrateStatus { extension, json } => {
            admin::execute_migrate_status_standalone(db_ctx, extension.as_deref(), json, config)
                .await
        },
        DbCommands::MigrateRepair {
            extension,
            apply,
            json,
        } => {
            admin::execute_migrate_repair_standalone(
                db_ctx,
                config,
                admin::RepairArgs {
                    extension: extension.as_deref(),
                    apply,
                    json,
                },
            )
            .await
        },
        DbCommands::MigrateMarkApplied {
            extension,
            version,
            json,
        } => {
            admin::execute_migrate_mark_applied_standalone(
                db_ctx,
                config,
                admin::MarkAppliedArgs {
                    extension: &extension,
                    version,
                    json,
                },
            )
            .await
        },
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
