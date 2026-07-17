//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{Database, MigrationService};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_models::Config;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandOutput, render_result};

use super::admin_migrate::select_extensions;
use super::types::MigrateMarkAppliedOutput;

#[derive(Clone, Copy)]
pub(super) struct MarkAppliedArgs<'a> {
    pub extension: &'a str,
    pub version: u32,
    pub json: bool,
}

pub(super) async fn execute_migrate_mark_applied(
    config: &CliConfig,
    args: MarkAppliedArgs<'_>,
) -> Result<()> {
    let sys_config = Config::get()?;
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

    run_mark_applied(
        database.write(),
        &ExtensionRegistry::discover()?,
        args,
        config,
    )
    .await
}

pub(super) async fn execute_migrate_mark_applied_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
    args: MarkAppliedArgs<'_>,
) -> Result<()> {
    run_mark_applied(
        db_ctx.db_pool().write(),
        &ExtensionRegistry::discover()?,
        args,
        config,
    )
    .await
}

async fn run_mark_applied(
    write_provider: &dyn DatabaseProvider,
    registry: &ExtensionRegistry,
    args: MarkAppliedArgs<'_>,
    config: &CliConfig,
) -> Result<()> {
    let MarkAppliedArgs {
        extension,
        version,
        json,
    } = args;

    let extensions = select_extensions(registry, Some(extension))?;
    let ext = extensions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Extension '{}' not found", extension))?;

    let migration_service = MigrationService::new(write_provider);
    let outcome = migration_service
        .mark_applied(ext.as_ref(), version)
        .await
        .map_err(|e| anyhow!("Failed to mark migration as applied: {}", e))?;

    let message = format!(
        "Recorded {} v{:03} '{}' as applied (checksum {})",
        outcome.extension_id,
        outcome.version,
        outcome.name,
        &outcome.checksum[..outcome.checksum.len().min(8)]
    );

    let output = MigrateMarkAppliedOutput {
        extension_id: outcome.extension_id,
        version: outcome.version,
        name: outcome.name,
        checksum: outcome.checksum,
        message: message.clone(),
    };

    if json || config.is_json_output() {
        let result = CommandOutput::card_value("Migration Mark Applied", &output);
        render_result(&result, config);
    } else {
        CliService::success(&message);
    }

    Ok(())
}
