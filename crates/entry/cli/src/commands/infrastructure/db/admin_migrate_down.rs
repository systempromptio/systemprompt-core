//! `infra db migrate down` command reverting a migration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use crate::kernel::KernelHost;
use std::sync::Arc;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{Database, MigrationService};
use systemprompt_extension::BuiltState;
use systemprompt_identifiers::ExtensionId;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandOutput, render_result};

use super::types::DbMigrateDownOutput;

pub(super) async fn execute_migrate_down(
    host: &dyn KernelHost,
    extension: &ExtensionId,
    count: u32,
) -> Result<()> {
    let config = host.output();
    let sys_config = host.config()?;

    let database = Arc::new(
        Database::connect(
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
            &systemprompt_database::PoolConfig::default(),
        )
        .await
        .context("Failed to connect to database")?
        .with_session_url(
            sys_config.database_session_url.as_deref(),
            &systemprompt_database::PoolConfig::default().for_session(None),
        )
        .await
        .context("Failed to connect to the session database")?,
    );

    run_down(
        host.admitted()?.built(),
        database.session(),
        config,
        extension,
        count,
    )
    .await
}

pub(super) async fn execute_migrate_down_standalone(
    db_ctx: &DatabaseContext,
    built: &BuiltState,
    config: &CliConfig,
    extension: &ExtensionId,
    count: u32,
) -> Result<()> {
    let database = db_ctx.db_pool();
    run_down(
        built,
        database.session(),
        config,
        extension,
        count,
    )
    .await
}

async fn run_down(
    built: &BuiltState,
    write_provider: &dyn DatabaseProvider,
    config: &CliConfig,
    extension_id: &ExtensionId,
    count: u32,
) -> Result<()> {
    let ext = super::admin_migrate::find_member(built, extension_id)?;

    let migration_service = MigrationService::new(write_provider);
    let result = migration_service
        .run_down_migrations(ext.as_ref(), count)
        .await
        .context("Down migration failed")?;

    let output = DbMigrateDownOutput {
        extension: extension_id.clone(),
        migrations_reverted: result.migrations_run,
        message: format!(
            "Reverted {} migration(s) for '{}'",
            result.migrations_run, extension_id
        ),
    };

    if config.is_json_output() {
        let result = CommandOutput::card_value("Database Admin", &output);
        render_result(&result, config);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
