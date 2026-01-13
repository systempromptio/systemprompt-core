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

#[derive(Subcommand)]
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

    fn print_result(result: &QueryResult, format: &str) {
        let config = get_global_config();
        let output_format = if format == "json" {
            OutputFormat::Json
        } else if format == "yaml" {
            OutputFormat::Yaml
        } else {
            config.output_format
        };

        match output_format {
            OutputFormat::Json => CliService::json(result),
            OutputFormat::Yaml => CliService::yaml(result),
            OutputFormat::Table => result.display_with_cli(),
        }
    }
}

async fn migrate_standalone() -> Result<()> {
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
        match install_module_with_db(module, database.as_ref()).await {
            Ok(()) => {},
            Err(e) => {
                CliService::error(&format!("{} failed: {}", module.name, e));
                error_count += 1;
            },
        }
    }

    if error_count > 0 {
        return Err(anyhow!("Some modules failed to install"));
    }

    Ok(())
}

pub async fn execute(cmd: DbCommands, _config: &CliConfig) -> Result<()> {
    if matches!(cmd, DbCommands::Migrate) {
        return match migrate_standalone().await {
            Ok(()) => {
                CliService::success("Database migration completed");
                Ok(())
            },
            Err(e) => {
                CliService::error(&format!("Migration failed: {}", e));
                Err(e)
            },
        };
    }

    let db = DatabaseTool::new().await?;
    let config = get_global_config();

    match cmd {
        DbCommands::Query { sql, format } => {
            match db.query_executor.execute_query(&sql, true).await {
                Ok(result) => {
                    if config.should_show_verbose() {
                        CliService::verbose(&format!(
                            "Query returned {} rows in {}ms",
                            result.row_count, result.execution_time_ms
                        ));
                    }
                    DatabaseTool::print_result(&result, &format);
                },
                Err(e) => {
                    CliService::error(&format!("Query failed: {}", e));
                    return Err(anyhow!("{}", e));
                },
            }
        },
        DbCommands::Execute { sql, format } => {
            match db.query_executor.execute_query(&sql, false).await {
                Ok(result) => {
                    if config.should_show_verbose() {
                        CliService::verbose(&format!(
                            "Execution completed in {}ms",
                            result.execution_time_ms
                        ));
                    }
                    DatabaseTool::print_result(&result, &format);
                },
                Err(e) => {
                    CliService::error(&format!("Execution failed: {}", e));
                    return Err(anyhow!("{}", e));
                },
            }
        },
        DbCommands::Tables => match db.admin_service.list_tables().await {
            Ok(tables) => {
                if config.is_json_output() {
                    CliService::json(&tables);
                } else {
                    tables.display_with_cli();
                }
            },
            Err(e) => {
                CliService::error(&format!("Failed to list tables: {}", e));
                return Err(e);
            },
        },
        DbCommands::Describe { table_name } => {
            match db.admin_service.describe_table(&table_name).await {
                Ok((columns, row_count)) => {
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
                },
                Err(e) => {
                    CliService::error(&format!("Failed to describe table: {}", e));
                    return Err(e);
                },
            }
        },
        DbCommands::Info => match db.admin_service.get_database_info().await {
            Ok(info) => {
                if config.is_json_output() {
                    CliService::json(&info);
                } else {
                    info.display_with_cli();
                }
            },
            Err(e) => {
                CliService::error(&format!("Failed to get database info: {}", e));
                return Err(e);
            },
        },
        DbCommands::Migrate => unreachable!("Migrate is handled earlier"),
        DbCommands::AssignAdmin { user } => {
            let user_service = UserService::new(db.ctx.db_pool())?;
            let user_admin = UserAdminService::new(user_service);

            CliService::info(&format!("Looking up user: {}", user));

            match user_admin.promote_to_admin(&user).await? {
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
                    CliService::error(&format!("User '{}' not found", user));
                    return Err(anyhow!("User not found"));
                },
            }
        },
    }

    Ok(())
}
