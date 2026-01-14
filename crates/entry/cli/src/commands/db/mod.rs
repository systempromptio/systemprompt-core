mod types;

use crate::cli_settings::{CliConfig, OutputFormat};
use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use std::sync::Arc;
use systemprompt_core_database::{
    DatabaseAdminService, DatabaseCliDisplay, QueryExecutor, QueryResult,
};
use systemprompt_core_logging::CliService;
use systemprompt_core_users::{PromoteResult, UserAdminService, UserService};
use systemprompt_runtime::AppContext;
use tabled::{Table, Tabled};

pub use types::*;

#[derive(Debug, Subcommand)]
pub enum DbCommands {
    #[command(about = "Execute SQL query (read-only)")]
    Query {
        /// SQL query to execute
        sql: String,
        /// Output format: table, json, yaml
        #[arg(long, default_value = "table")]
        format: String,
    },
    #[command(about = "Execute write operation (INSERT, UPDATE, DELETE)")]
    Execute {
        /// SQL statement to execute
        sql: String,
        /// Output format: table, json, yaml
        #[arg(long, default_value = "table")]
        format: String,
    },
    #[command(about = "List all tables with row counts and sizes")]
    Tables,
    #[command(about = "Describe table schema with columns and indexes")]
    Describe {
        /// Table name to describe
        table_name: String,
    },
    #[command(about = "Show database information")]
    Info,
    #[command(about = "Run database migrations")]
    Migrate,
    #[command(about = "Assign admin role to a user")]
    AssignAdmin {
        /// Username or email
        user: String,
    },
    #[command(about = "Show database connection status")]
    Status,
    #[command(about = "Validate database schema against expected tables")]
    Validate,
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

fn get_output_format(format_arg: &str, config: &CliConfig) -> OutputFormat {
    match format_arg {
        "json" => OutputFormat::Json,
        "yaml" => OutputFormat::Yaml,
        _ => config.output_format,
    }
}

fn print_query_result(result: &QueryResult, output_format: OutputFormat) {
    match output_format {
        OutputFormat::Json => CliService::json(result),
        OutputFormat::Yaml => CliService::yaml(result),
        OutputFormat::Table => result.display_with_cli(),
    }
}

pub async fn execute(cmd: DbCommands, config: &CliConfig) -> Result<()> {
    if matches!(cmd, DbCommands::Migrate) {
        return execute_migrate(config).await;
    }

    let db = DatabaseTool::new().await?;

    match cmd {
        DbCommands::Query { sql, format } => execute_query(&db, &sql, &format, config).await,
        DbCommands::Execute { sql, format } => execute_write(&db, &sql, &format, config).await,
        DbCommands::Tables => execute_tables(&db, config).await,
        DbCommands::Describe { table_name } => execute_describe(&db, &table_name, config).await,
        DbCommands::Info => execute_info(&db, config).await,
        DbCommands::Migrate => unreachable!(),
        DbCommands::AssignAdmin { user } => execute_assign_admin(&db, &user, config).await,
        DbCommands::Status => execute_status(&db, config).await,
        DbCommands::Validate => execute_validate(&db, config).await,
    }
}

async fn execute_migrate(config: &CliConfig) -> Result<()> {
    use systemprompt_core_database::Database;
    use systemprompt_loader::ModuleLoader;
    use systemprompt_models::Config;
    use systemprompt_runtime::{install_module_with_db, Modules};

    let sys_config = Config::get()?;

    if config.should_show_verbose() {
        CliService::info(&format!("System path: {}", sys_config.system_path));
        CliService::info(&format!("Database type: {}", sys_config.database_type));
        CliService::info(&format!("Database URL: {}", sys_config.database_url));
    }

    let database = Arc::new(
        Database::from_config(&sys_config.database_type, &sys_config.database_url)
            .await
            .context("Failed to connect to database")?,
    );
    let modules = Modules::from_vec(ModuleLoader::all())?;
    let all_modules = modules.all();

    let mut installed_modules = Vec::new();
    let mut error_count = 0;

    if config.should_show_verbose() {
        CliService::info(&format!("Installing {} modules", all_modules.len()));
    }

    for module in all_modules {
        if config.should_show_verbose() {
            CliService::info(&format!("  Installing: {}", module.name));
        }
        if let Err(e) = install_module_with_db(module, database.as_ref()).await {
            CliService::error(&format!("{} failed: {}", module.name, e));
            error_count += 1;
        } else {
            installed_modules.push(module.name.to_string());
        }
    }

    if error_count > 0 {
        return Err(anyhow!("Some modules failed to install"));
    }

    let output = DbMigrateOutput {
        modules_installed: installed_modules,
        message: "Database migration completed successfully".to_string(),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}

async fn execute_query(
    db: &DatabaseTool,
    sql: &str,
    format: &str,
    config: &CliConfig,
) -> Result<()> {
    let output_format = get_output_format(format, config);

    let result = db
        .query_executor
        .execute_query(sql, true)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("does not exist") {
                anyhow!("Table or column not found: {}", extract_relation_name(&msg))
            } else if msg.contains("syntax error") {
                anyhow!("SQL syntax error: {}", msg)
            } else {
                anyhow!("Query failed: {}", msg)
            }
        })?;

    if config.should_show_verbose() {
        CliService::verbose(&format!(
            "Query returned {} rows in {}ms",
            result.row_count, result.execution_time_ms
        ));
    }

    print_query_result(&result, output_format);
    Ok(())
}

async fn execute_write(
    db: &DatabaseTool,
    sql: &str,
    format: &str,
    config: &CliConfig,
) -> Result<()> {
    let output_format = get_output_format(format, config);

    let result = db
        .query_executor
        .execute_query(sql, false)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("does not exist") {
                anyhow!("Table or column not found: {}", extract_relation_name(&msg))
            } else if msg.contains("syntax error") {
                anyhow!("SQL syntax error: {}", msg)
            } else if msg.contains("violates") {
                anyhow!("Constraint violation: {}", msg)
            } else {
                anyhow!("Execution failed: {}", msg)
            }
        })?;

    // For write operations, show rows affected
    let output = DbExecuteOutput {
        rows_affected: result.row_count as u64,
        execution_time_ms: result.execution_time_ms,
        message: format!(
            "Query executed successfully, {} row(s) affected",
            result.row_count
        ),
    };

    if matches!(output_format, OutputFormat::Json) {
        CliService::json(&output);
    } else if matches!(output_format, OutputFormat::Yaml) {
        CliService::yaml(&output);
    } else {
        CliService::success(&output.message);
        if config.should_show_verbose() {
            CliService::verbose(&format!(
                "Execution completed in {}ms",
                result.execution_time_ms
            ));
        }
    }

    Ok(())
}

#[derive(Tabled)]
struct TableRow {
    #[tabled(rename = "Table")]
    name: String,
    #[tabled(rename = "Rows")]
    row_count: i64,
    #[tabled(rename = "Size")]
    size: String,
}

async fn execute_tables(db: &DatabaseTool, config: &CliConfig) -> Result<()> {
    let tables = db
        .admin_service
        .list_tables()
        .await
        .context("Failed to list tables")?;

    let output = DbTablesOutput {
        total: tables.len(),
        tables: tables
            .iter()
            .map(|t| TableInfo {
                name: t.name.clone(),
                schema: "public".to_string(),
                row_count: t.row_count,
                size_bytes: t.size_bytes,
            })
            .collect(),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section("Tables");

        if tables.is_empty() {
            CliService::info("No tables found");
        } else {
            let rows: Vec<TableRow> = tables
                .iter()
                .map(|t| TableRow {
                    name: t.name.clone(),
                    row_count: t.row_count,
                    size: format_bytes(t.size_bytes),
                })
                .collect();

            let table = Table::new(rows).to_string();
            CliService::output(&table);
            CliService::info(&format!("Total: {} table(s)", output.total));
        }
    }

    Ok(())
}

async fn execute_describe(db: &DatabaseTool, table_name: &str, config: &CliConfig) -> Result<()> {
    let (columns, row_count) = db
        .admin_service
        .describe_table(table_name)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not found") || msg.contains("does not exist") {
                anyhow!("Table '{}' not found", table_name)
            } else {
                anyhow!("Failed to describe table: {}", msg)
            }
        })?;

    let indexes = db
        .admin_service
        .get_table_indexes(table_name)
        .await
        .unwrap_or_default();

    let output = DbDescribeOutput {
        table: table_name.to_string(),
        row_count,
        columns: columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
                nullable: c.nullable,
                default: c.default.clone(),
                primary_key: c.primary_key,
            })
            .collect(),
        indexes: indexes
            .iter()
            .map(|i| IndexInfo {
                name: i.name.clone(),
                columns: i.columns.clone(),
                unique: i.unique,
            })
            .collect(),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section(&format!("Table: {} ({} rows)", table_name, row_count));

        // Display columns
        CliService::subsection("Columns");
        (columns, row_count).display_with_cli();

        // Display indexes
        if !indexes.is_empty() {
            CliService::subsection("Indexes");
            for idx in &indexes {
                let unique_marker = if idx.unique { " (unique)" } else { "" };
                CliService::info(&format!(
                    "  {} [{}]{}",
                    idx.name,
                    idx.columns.join(", "),
                    unique_marker
                ));
            }
        }
    }

    Ok(())
}

async fn execute_info(db: &DatabaseTool, config: &CliConfig) -> Result<()> {
    let info = db
        .admin_service
        .get_database_info()
        .await
        .context("Failed to get database info")?;

    let table_names: Vec<String> = info.tables.iter().map(|t| t.name.clone()).collect();

    let output = DbInfoOutput {
        version: info.version.clone(),
        database: info.path.clone(),
        size: format_bytes(info.size as i64),
        table_count: info.tables.len(),
        tables: table_names,
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section("Database Info");
        CliService::key_value("Database", &output.database);
        CliService::key_value("Version", &output.version);
        CliService::key_value("Size", &output.size);
        CliService::key_value("Tables", &output.table_count.to_string());
    }

    Ok(())
}

async fn execute_assign_admin(db: &DatabaseTool, user: &str, config: &CliConfig) -> Result<()> {
    let user_service = UserService::new(db.ctx.db_pool())?;
    let user_admin = UserAdminService::new(user_service);

    if !config.is_json_output() {
        CliService::info(&format!("Looking up user: {}", user));
    }

    match user_admin.promote_to_admin(user).await? {
        PromoteResult::Promoted(u, new_roles) => {
            let output = DbAssignAdminOutput {
                user_id: u.id.to_string(),
                name: u.name.clone(),
                email: u.email.clone(),
                roles: new_roles.clone(),
                already_admin: false,
                message: format!("Admin role assigned to user '{}' ({})", u.name, u.email),
            };

            if config.is_json_output() {
                CliService::json(&output);
            } else {
                CliService::success(&output.message);
                CliService::info(&format!("   Roles: {:?}", new_roles));
            }
        }
        PromoteResult::AlreadyAdmin(u) => {
            let output = DbAssignAdminOutput {
                user_id: u.id.to_string(),
                name: u.name.clone(),
                email: u.email.clone(),
                roles: u.roles.clone(),
                already_admin: true,
                message: format!("User '{}' already has admin role", u.name),
            };

            if config.is_json_output() {
                CliService::json(&output);
            } else {
                CliService::warning(&output.message);
            }
        }
        PromoteResult::UserNotFound => {
            return Err(anyhow!("User '{}' not found", user));
        }
    }

    Ok(())
}

async fn execute_status(db: &DatabaseTool, config: &CliConfig) -> Result<()> {
    let info = db
        .admin_service
        .get_database_info()
        .await
        .context("Failed to connect to database")?;

    let output = DbStatusOutput {
        status: "connected".to_string(),
        version: info.version.clone(),
        tables: info.tables.len(),
        size: format_bytes(info.size as i64),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success("Database connection: OK");
        CliService::key_value("  Version", &output.version);
        CliService::key_value("  Tables", &output.tables.to_string());
        CliService::key_value("  Size", &output.size);
    }

    Ok(())
}

async fn execute_validate(db: &DatabaseTool, config: &CliConfig) -> Result<()> {
    let info = db
        .admin_service
        .get_database_info()
        .await
        .context("Failed to get database info")?;

    let expected_tables: Vec<&str> = DatabaseAdminService::get_expected_tables();
    let table_names: Vec<String> = info.tables.iter().map(|t| t.name.clone()).collect();
    let actual_tables: std::collections::HashSet<&str> =
        table_names.iter().map(|s| s.as_str()).collect();

    let missing: Vec<String> = expected_tables
        .iter()
        .filter(|t| !actual_tables.contains(*t))
        .map(|t| t.to_string())
        .collect();

    let extra: Vec<String> = table_names
        .iter()
        .filter(|t| {
            !expected_tables.contains(&t.as_str())
                && !t.starts_with("_sqlx")
                && !t.starts_with("v_")
        })
        .cloned()
        .collect();

    let valid = missing.is_empty();

    let output = DbValidateOutput {
        valid,
        expected_tables: expected_tables.len(),
        actual_tables: table_names.len(),
        missing_tables: missing.clone(),
        extra_tables: extra.clone(),
        message: if valid {
            "Database schema is valid".to_string()
        } else {
            format!("Database schema has {} missing table(s)", missing.len())
        },
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section("Schema Validation");

        if valid {
            CliService::success(&output.message);
        } else {
            CliService::error(&output.message);
            CliService::info("Missing tables:");
            for table in &missing {
                CliService::info(&format!("  - {}", table));
            }
        }

        if !extra.is_empty() && config.should_show_verbose() {
            CliService::info("Extra tables (not in expected list):");
            for table in &extra {
                CliService::info(&format!("  - {}", table));
            }
        }

        CliService::info(&format!(
            "Expected: {}, Actual: {}",
            output.expected_tables, output.actual_tables
        ));
    }

    Ok(())
}

/// Format bytes into human-readable string
fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

/// Extract relation name from database error message
fn extract_relation_name(msg: &str) -> String {
    // Try to extract the relation name from messages like:
    // 'relation "foo" does not exist'
    if let Some(start) = msg.find('"') {
        if let Some(end) = msg[start + 1..].find('"') {
            return msg[start + 1..start + 1 + end].to_string();
        }
    }
    "unknown".to_string()
}
