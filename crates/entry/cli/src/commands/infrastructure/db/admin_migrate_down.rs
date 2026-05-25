use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{Database, MigrationService};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_models::Config;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, render_result};

use super::types::DbMigrateDownOutput;

pub(super) async fn execute_migrate_down(
    config: &CliConfig,
    extension: &str,
    count: u32,
) -> Result<()> {
    let sys_config = Config::get()?;

    let database = Arc::new(
        Database::from_config_with_write(
            &sys_config.database_type,
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
        )
        .await
        .context("Failed to connect to database")?,
    );

    run_down(
        &ExtensionRegistry::discover()?,
        database.write_provider(),
        config,
        extension,
        count,
    )
    .await
}

pub(super) async fn execute_migrate_down_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
    extension: &str,
    count: u32,
) -> Result<()> {
    let database = db_ctx.db_pool();
    run_down(
        &ExtensionRegistry::discover()?,
        database.write_provider(),
        config,
        extension,
        count,
    )
    .await
}

async fn run_down(
    registry: &ExtensionRegistry,
    write_provider: &dyn DatabaseProvider,
    config: &CliConfig,
    extension_id: &str,
    count: u32,
) -> Result<()> {
    let ext = registry
        .get(extension_id)
        .ok_or_else(|| anyhow!("Extension '{}' not found", extension_id))?;

    let migration_service = MigrationService::new(write_provider);
    let result = migration_service
        .run_down_migrations(ext.as_ref(), count)
        .await
        .map_err(|e| anyhow!("Down migration failed: {}", e))?;

    let output = DbMigrateDownOutput {
        extension: extension_id.to_owned(),
        migrations_reverted: result.migrations_run,
        message: format!(
            "Reverted {} migration(s) for '{}'",
            result.migrations_run, extension_id
        ),
    };

    if config.is_json_output() {
        let result = CommandResult::text(output).with_title("Database Admin");
        render_result(&result);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
