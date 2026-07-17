//! `infra db migrate` command installing schemas and migrations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{Database, MigrationConfig, install_extension_schemas_full};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_models::Config;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandOutput, render_result};

use super::types::DbMigrateOutput;

pub(super) async fn execute_migrate(config: &CliConfig, allow_checksum_drift: bool) -> Result<()> {
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
            &systemprompt_database::PoolConfig::default(),
        )
        .await
        .context("Failed to connect to database")?,
    );

    run_install(
        &ExtensionRegistry::discover()?,
        database.write(),
        config,
        allow_checksum_drift,
    )
    .await
}

pub(super) async fn execute_migrate_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
    allow_checksum_drift: bool,
) -> Result<()> {
    let database = db_ctx.db_pool();
    run_install(
        &ExtensionRegistry::discover()?,
        database.write(),
        config,
        allow_checksum_drift,
    )
    .await
}

async fn run_install(
    registry: &ExtensionRegistry,
    write_provider: &dyn DatabaseProvider,
    config: &CliConfig,
    allow_checksum_drift: bool,
) -> Result<()> {
    let extension_count = registry.schema_extensions().len();

    if config.should_show_verbose() {
        CliService::info(&format!(
            "Installing schemas for {} extensions",
            extension_count
        ));
    }

    let migration_config = MigrationConfig {
        allow_checksum_drift,
    };

    install_extension_schemas_full(registry, write_provider, &[], migration_config)
        .await
        .map_err(|e| anyhow!("Schema installation failed: {}", e))?;

    let installed_extensions: Vec<String> = registry
        .schema_extensions()
        .iter()
        .map(|ext| ext.id().to_owned())
        .collect();

    let output = DbMigrateOutput {
        modules_installed: installed_extensions,
        message: "Database migration completed successfully".to_owned(),
    };

    if config.is_json_output() {
        let result = CommandOutput::card_value("Database Admin", &output);
        render_result(&result, config);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}

pub(super) fn select_extensions(
    registry: &ExtensionRegistry,
    extension: Option<&str>,
) -> Result<Vec<Arc<dyn systemprompt_extension::Extension>>> {
    let all = registry.schema_extensions();
    if let Some(ext_id) = extension {
        let filtered: Vec<_> = all.into_iter().filter(|e| e.id() == ext_id).collect();
        if filtered.is_empty() {
            return Err(anyhow!("Extension '{}' not found or has no schema", ext_id));
        }
        Ok(filtered)
    } else {
        Ok(all)
    }
}
