use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use systemprompt_database::{Database, install_extension_schemas};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_models::Config;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, render_result};

use super::types::DbMigrateOutput;

pub async fn execute_migrate(config: &CliConfig) -> Result<()> {
    let sys_config = Config::get()?;

    if config.should_show_verbose() {
        CliService::info(&format!("System path: {}", sys_config.system_path));
        CliService::info(&format!("Database type: {}", sys_config.database_type));
        CliService::info(&format!("Database URL: {}", sys_config.database_url));
    }

    let database = Arc::new(
        Database::from_config_with_write(
            &sys_config.database_type,
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
        )
        .await
        .context("Failed to connect to database")?,
    );

    run_install(
        &ExtensionRegistry::discover(),
        database.write_provider(),
        config,
    )
    .await
}

pub async fn execute_migrate_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let database = db_ctx.db_pool();
    run_install(
        &ExtensionRegistry::discover(),
        database.write_provider(),
        config,
    )
    .await
}

async fn run_install(
    registry: &ExtensionRegistry,
    write_provider: &dyn systemprompt_database::services::DatabaseProvider,
    config: &CliConfig,
) -> Result<()> {
    let extension_count = registry.schema_extensions().len();

    if config.should_show_verbose() {
        CliService::info(&format!(
            "Installing schemas for {} extensions",
            extension_count
        ));
    }

    install_extension_schemas(registry, write_provider)
        .await
        .map_err(|e| anyhow!("Schema installation failed: {}", e))?;

    let installed_extensions: Vec<String> = registry
        .schema_extensions()
        .iter()
        .map(|ext| ext.id().to_string())
        .collect();

    let output = DbMigrateOutput {
        modules_installed: installed_extensions,
        message: "Database migration completed successfully".to_string(),
    };

    if config.is_json_output() {
        let result = CommandResult::text(output).with_title("Database Admin");
        render_result(&result);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
