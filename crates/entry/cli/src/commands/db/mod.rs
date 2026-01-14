mod admin;
mod helpers;
mod query;
mod schema;
mod types;

use anyhow::{Context, Result};
use clap::Subcommand;
use std::sync::Arc;
use systemprompt_core_database::{DatabaseAdminService, QueryExecutor};
use systemprompt_runtime::AppContext;

use crate::cli_settings::CliConfig;

pub use types::*;

#[derive(Debug, Subcommand)]
pub enum DbCommands {
    #[command(about = "Execute SQL query (read-only)")]
    Query {
        sql: String,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        offset: Option<u32>,
        #[arg(long)]
        format: Option<String>,
    },
    #[command(about = "Execute write operation (INSERT, UPDATE, DELETE)")]
    Execute {
        sql: String,
        #[arg(long)]
        format: Option<String>,
    },
    #[command(about = "List all tables with row counts and sizes")]
    Tables {
        #[arg(long, help = "Filter tables by pattern")]
        filter: Option<String>,
    },
    #[command(about = "Describe table schema with columns and indexes")]
    Describe { table_name: String },
    #[command(about = "Show database information")]
    Info,
    #[command(about = "Run database migrations")]
    Migrate,
    #[command(about = "Assign admin role to a user")]
    AssignAdmin { user: String },
    #[command(about = "Show database connection status")]
    Status,
    #[command(about = "Validate database schema against expected tables")]
    Validate,
    #[command(about = "Get row count for a table")]
    Count { table_name: String },
    #[command(about = "List all indexes")]
    Indexes {
        #[arg(long, help = "Filter by table name")]
        table: Option<String>,
    },
    #[command(about = "Show database and table sizes")]
    Size,
}

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
        let pool = ctx.db_pool().pool_arc()?;
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
    if matches!(cmd, DbCommands::Migrate) {
        return admin::execute_migrate(config).await;
    }

    let db = DatabaseTool::new().await?;

    match cmd {
        DbCommands::Query { sql, limit, offset, format } => {
            query::execute_query(&db.query_executor, &sql, limit, offset, &format, config).await
        }
        DbCommands::Execute { sql, format } => {
            query::execute_write(&db.query_executor, &sql, &format, config).await
        }
        DbCommands::Tables { filter } => {
            schema::execute_tables(&db.admin_service, filter, config).await
        }
        DbCommands::Describe { table_name } => {
            schema::execute_describe(&db.admin_service, &table_name, config).await
        }
        DbCommands::Info => schema::execute_info(&db.admin_service, config).await,
        DbCommands::Migrate => unreachable!(),
        DbCommands::AssignAdmin { user } => {
            admin::execute_assign_admin(&db.ctx, &user, config).await
        }
        DbCommands::Status => admin::execute_status(&db.admin_service, config).await,
        DbCommands::Validate => schema::execute_validate(&db.admin_service, config).await,
        DbCommands::Count { table_name } => {
            schema::execute_count(&db.admin_service, &table_name, config).await
        }
        DbCommands::Indexes { table } => {
            schema::execute_indexes(&db.admin_service, table, config).await
        }
        DbCommands::Size => schema::execute_size(&db.admin_service, config).await,
    }
}
