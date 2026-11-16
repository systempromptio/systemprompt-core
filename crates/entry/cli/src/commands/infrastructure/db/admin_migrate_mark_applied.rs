//! `infra db migrate mark-applied` command recording a migration without
//! running it.
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
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::DatabaseContext;

use systemprompt_extension::cli::CommandOutput;

use super::admin_migrate::find_member;
use super::types::MigrateMarkAppliedOutput;

#[derive(Clone, Copy)]
pub(super) struct MarkAppliedArgs<'a> {
    pub extension: &'a ExtensionId,
    pub version: u32,
}

pub(super) async fn execute_migrate_mark_applied(
    host: &dyn KernelHost,
    args: MarkAppliedArgs<'_>,
) -> Result<CommandOutput> {
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

    run_mark_applied(database.session(), host.admitted()?.built(), args).await
}

pub(super) async fn execute_migrate_mark_applied_standalone(
    db_ctx: &DatabaseContext,
    built: &BuiltState,
    args: MarkAppliedArgs<'_>,
) -> Result<CommandOutput> {
    run_mark_applied(db_ctx.db_pool().session(), built, args).await
}

async fn run_mark_applied(
    write_provider: &dyn DatabaseProvider,
    built: &BuiltState,
    args: MarkAppliedArgs<'_>,
) -> Result<CommandOutput> {
    let MarkAppliedArgs { extension, version } = args;

    let ext = find_member(built, extension)?;

    let migration_service = MigrationService::new(write_provider);
    let outcome = migration_service
        .mark_applied(ext.as_ref(), version)
        .await
        .context("Failed to mark migration as applied")?;

    let message = format!(
        "Recorded {} v{:03} '{}' as applied (checksum {})",
        outcome.extension_id,
        outcome.version,
        outcome.name,
        &outcome.checksum[..outcome.checksum.len().min(8)]
    );

    let output = MigrateMarkAppliedOutput {
        extension_id: ExtensionId::new(outcome.extension_id),
        version: outcome.version,
        name: outcome.name,
        checksum: outcome.checksum,
        message: message.clone(),
    };

    Ok(CommandOutput::card_value("Migration Mark Applied", &output)
        .with_note(NoticeLine::new("success", message)))
}
