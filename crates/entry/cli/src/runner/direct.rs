//! The `--database-url` invocation path.
//!
//! A command whose [`DbUrlRouting`] is `Direct` runs against the supplied
//! connection without a profile: [`direct_database_url`] decides whether the
//! flag applies to the selected command, and [`run_with_database_url`] dials
//! the database and dispatches with a database-scoped [`CommandContext`].
//! A cloud profile with external database access takes the same path with
//! its profile loaded: admission is against that profile, and a disabled
//! member's command is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, bail};
use systemprompt_extension::cli::{CliConfig, DbUrlRouting};
use systemprompt_manifest::Profile;
use systemprompt_runtime::DatabaseContext;

use super::RunGuards;
use super::dispatch::{Invocation, Selected, dispatch_command};
use crate::context::CommandContext;
use crate::env_overrides::EnvOverrides;

pub(super) fn direct_database_url(invocation: &Invocation) -> Result<Option<String>> {
    let Some(database_url) = invocation.cli.database.database_url.clone() else {
        return Ok(None);
    };
    match invocation.db_url_routing() {
        Some(DbUrlRouting::Direct) => Ok(Some(database_url)),
        Some(DbUrlRouting::Unsupported) => bail!(
            "This command cannot run with --database-url; it requires full profile \
             initialization. Remove --database-url."
        ),
        Some(DbUrlRouting::ProfileDriven) | None => Ok(None),
    }
}

pub(super) struct DirectDatabase<'a> {
    pub(super) url: &'a str,
    pub(super) profile: Option<&'a Profile>,
}

pub(super) async fn run_with_database_url(
    selected: Selected,
    cli_config: CliConfig,
    env: EnvOverrides,
    database: DirectDatabase<'_>,
    guards: RunGuards,
) -> Result<()> {
    let DirectDatabase {
        url: database_url,
        profile,
    } = database;
    let db_ctx = DatabaseContext::from_url(database_url)
        .await
        .context("Failed to connect to database")?;

    systemprompt_logging::init_logging(&db_ctx.db_pool_arc(), None);

    let ctx = guards.apply(
        CommandContext::with_database(cli_config, env, db_ctx, database_url.to_owned()),
        &selected,
        profile,
    )?;
    Box::pin(dispatch_command(selected, &ctx)).await
}
