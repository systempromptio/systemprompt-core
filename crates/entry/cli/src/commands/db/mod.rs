use crate::cli_settings::{get_global_config, CliConfig, OutputFormat};
use anyhow::{anyhow, Result};
use clap::Subcommand;
use std::sync::Arc;
use systemprompt_core_database::{
    DatabaseAdminService, DatabaseCliDisplay, QueryExecutor, QueryResult,
};
use systemprompt_core_logging::CliService;
use systemprompt_core_users::{PromoteResult, UserAdminService, UserService};
use systemprompt_runtime::AppContext;

#[derive(Debug, Subcommand)]
pub enum DbCommands {
    #[command(about = "Execute SQL query")]
    Query {
        sql: String,
        #[arg(long, default_value = "table")]
        format: String,
    },
    #[command(about = "Execute write operation")]
    Execute {
        sql: String,
        #[arg(long, default_value = "table")]
        format: String,
    },
    #[command(about = "List all tables")]
    Tables,
    #[command(about = "Describe table schema")]
    Describe { table_name: String },
    #[command(about = "Database information")]
    Info,
    #[command(about = "Run database migrations")]
    Migrate,
    #[command(about = "Assign admin role to a user")]
    AssignAdmin { user: String },
    #[command(about = "Show database connection status")]
    Status,
    #[command(about = "Reset database (drop all tables and recreate)")]
    Reset {
        #[arg(long, help = "Skip confirmation prompt")]
        yes: bool,
    },
}

struct DatabaseTool {
    ctx: AppContext,
    admin_service: DatabaseAdminService,
    query_executor: QueryExecutor,
}

impl DatabaseTool {
    async fn new() -> Result<Self> {
        let ctx = AppContext::new().await?;
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

fn print_query_result(result: &QueryResult, format: &str) {
    let config = get_global_config();
    let output_format = match format {
        "json" => OutputFormat::Json,
        "yaml" => OutputFormat::Yaml,
        _ => config.output_format,
    };

    match output_format {
        OutputFormat::Json => CliService::json(result),
        OutputFormat::Yaml => CliService::yaml(result),
        OutputFormat::Table => result.display_with_cli(),
    }
}

pub async fn execute(cmd: DbCommands, config: &CliConfig) -> Result<()> {
    if matches!(cmd, DbCommands::Migrate) {
        return execute_migrate().await;
    }

    let db = DatabaseTool::new().await?;

    match cmd {
        DbCommands::Query { sql, format } => execute_query(&db, &sql, &format).await,
        DbCommands::Execute { sql, format } => execute_write(&db, &sql, &format).await,
        DbCommands::Tables => execute_tables(&db).await,
        DbCommands::Describe { table_name } => execute_describe(&db, &table_name).await,
        DbCommands::Info => execute_info(&db).await,
        DbCommands::Migrate => unreachable!(),
        DbCommands::AssignAdmin { user } => execute_assign_admin(&db, &user).await,
        DbCommands::Status => execute_status(&db).await,
        DbCommands::Reset { yes } => execute_reset(yes, config).await,
    }
}

async fn execute_migrate() -> Result<()> {
    use systemprompt_core_database::Database;
    use systemprompt_loader::ModuleLoader;
    use systemprompt_models::config::VerbosityLevel;
    use systemprompt_models::Config;
    use systemprompt_runtime::{install_module_with_db, Modules};

    let verbosity = VerbosityLevel::resolve();
    let config = Config::get()?;

    if verbosity.should_show_verbose() {
        CliService::info(&format!("System path: {}", config.system_path));
        CliService::info(&format!("Database type: {}", config.database_type));
        CliService::info(&format!("Database URL: {}", config.database_url));
    }

    let database =
        Arc::new(Database::from_config(&config.database_type, &config.database_url).await?);
    let modules = Modules::from_vec(ModuleLoader::all())?;
    let all_modules = modules.all();

    if verbosity.should_show_verbose() {
        CliService::info(&format!("Installing {} modules", all_modules.len()));
        for module in all_modules {
            CliService::info(&format!("  {}", module.name));
        }
    }

    let mut error_count = 0;
    for module in all_modules {
        if let Err(e) = install_module_with_db(module, database.as_ref()).await {
            CliService::error(&format!("{} failed: {}", module.name, e));
            error_count += 1;
        }
    }

    if error_count > 0 {
        return Err(anyhow!("Some modules failed to install"));
    }

    CliService::success("Database migration completed");
    Ok(())
}

async fn execute_query(db: &DatabaseTool, sql: &str, format: &str) -> Result<()> {
    let config = get_global_config();
    let result = db
        .query_executor
        .execute_query(sql, true)
        .await
        .map_err(|e| anyhow!("Query failed: {}", e))?;

    if config.should_show_verbose() {
        CliService::verbose(&format!(
            "Query returned {} rows in {}ms",
            result.row_count, result.execution_time_ms
        ));
    }
    print_query_result(&result, format);
    Ok(())
}

async fn execute_write(db: &DatabaseTool, sql: &str, format: &str) -> Result<()> {
    let config = get_global_config();
    let result = db
        .query_executor
        .execute_query(sql, false)
        .await
        .map_err(|e| anyhow!("Execution failed: {}", e))?;

    if config.should_show_verbose() {
        CliService::verbose(&format!(
            "Execution completed in {}ms",
            result.execution_time_ms
        ));
    }
    print_query_result(&result, format);
    Ok(())
}

async fn execute_tables(db: &DatabaseTool) -> Result<()> {
    let config = get_global_config();
    let tables = db.admin_service.list_tables().await?;

    if config.is_json_output() {
        CliService::json(&tables);
    } else {
        tables.display_with_cli();
    }
    Ok(())
}

async fn execute_describe(db: &DatabaseTool, table_name: &str) -> Result<()> {
    let config = get_global_config();
    let (columns, row_count) = db.admin_service.describe_table(table_name).await?;

    if config.is_json_output() {
        CliService::json(&serde_json::json!({
            "table": table_name,
            "row_count": row_count,
            "columns": columns
        }));
    } else {
        CliService::info(&format!("Table: {} ({} rows)", table_name, row_count));
        (columns, row_count).display_with_cli();
    }
    Ok(())
}

async fn execute_info(db: &DatabaseTool) -> Result<()> {
    let config = get_global_config();
    let info = db.admin_service.get_database_info().await?;

    if config.is_json_output() {
        CliService::json(&info);
    } else {
        info.display_with_cli();
    }
    Ok(())
}

async fn execute_assign_admin(db: &DatabaseTool, user: &str) -> Result<()> {
    let user_service = UserService::new(db.ctx.db_pool())?;
    let user_admin = UserAdminService::new(user_service);

    CliService::info(&format!("Looking up user: {}", user));

    match user_admin.promote_to_admin(user).await? {
        PromoteResult::Promoted(u, new_roles) => {
            CliService::success(&format!(
                "Admin role assigned to user '{}' ({})",
                u.name, u.email
            ));
            CliService::info(&format!("   Roles: {:?}", new_roles));
        },
        PromoteResult::AlreadyAdmin(u) => {
            CliService::warning(&format!("User '{}' already has admin role", u.name));
        },
        PromoteResult::UserNotFound => {
            return Err(anyhow!("User '{}' not found", user));
        },
    }
    Ok(())
}

async fn execute_status(db: &DatabaseTool) -> Result<()> {
    let config = get_global_config();
    let info = db.admin_service.get_database_info().await?;

    CliService::success("Database connection: OK");
    if config.is_json_output() {
        CliService::json(&serde_json::json!({
            "status": "connected",
            "version": info.version,
            "tables": info.tables.len(),
            "size": info.size
        }));
    } else {
        CliService::info(&format!("  Version: {}", info.version));
        CliService::info(&format!("  Tables: {}", info.tables.len()));
        CliService::info(&format!("  Size: {}", info.size));
    }
    Ok(())
}

async fn execute_reset(yes: bool, config: &CliConfig) -> Result<()> {
    if !yes && !config.interactive {
        CliService::warning("This will drop ALL tables and recreate the schema!");
        CliService::warning("Use --yes to confirm.");
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    CliService::warning("Resetting database...");
    execute_migrate().await?;
    CliService::success("Database reset completed");
    Ok(())
}
