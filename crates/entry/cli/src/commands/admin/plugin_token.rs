//! `admin keys issue-plugin-token` command minting a hook-scoped credential.
//!
//! The kernel declares it as a leaf under the `admin keys` group
//! `identity-http` contributes and runs it itself: it mints over the
//! deployment token authority and a CLI session row, which no member command
//! reaches.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Args, FromArgMatches};
use serde::{Deserialize, Serialize};

use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{AccessTokenId, PluginId};
use systemprompt_oauth::services::plugin_token::{PluginTokenService, PluginTokenSubject};
use systemprompt_security::DeploymentTokenAuthority;
use systemprompt_users::{UserRepository, UserService};

use systemprompt_extension::cli::{
    CliCommandLeaf, CommandDecl, CommandDescriptor, CommandError, CommandHost, CommandOutput,
    CommandParent, DataImpact, DescribeCommand,
};

use crate::kernel::ResolvedCommand;

struct AdminKeys;

impl CommandParent for AdminKeys {
    const PATH: &'static [&'static str] = &["admin", "keys"];
}

#[must_use]
pub(crate) fn decl() -> CommandDecl {
    CommandDecl::leaf_under::<AdminKeys, IssuePluginTokenArgs>()
}

pub(crate) fn is_issue_plugin_token(resolved: &ResolvedCommand) -> bool {
    resolved.owner == crate::kernel::CommandOwner::Kernel && resolved.path == decl().path()
}

pub(crate) async fn run_resolved(
    resolved: &ResolvedCommand,
    host: &dyn KernelHost,
) -> Result<CommandOutput> {
    let args = IssuePluginTokenArgs::from_arg_matches(resolved.parsed.matches())?;
    execute(args, host).await
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("`admin keys issue-plugin-token` runs on the kernel host only")]
struct KernelHostOnly;

#[derive(Debug, Args)]
pub struct IssuePluginTokenArgs {
    #[arg(
        long,
        help = "Admin email to mint the token for. Defaults to the active credentials profile."
    )]
    pub email: Option<String>,

    #[arg(
        long,
        default_value = "cowork-bundle",
        value_parser = systemprompt_extension::cli::parsers::parse_plugin_id,
        help = "Plugin identifier to embed in the token's `plugin_id` claim."
    )]
    pub plugin_id: PluginId,

    #[arg(
        long,
        default_value = "365",
        help = "Token lifetime in days (1..=365)."
    )]
    pub duration_days: u32,

    #[arg(long, help = "Print only the token (for scripting)")]
    pub token_only: bool,
}

impl DescribeCommand for IssuePluginTokenArgs {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(DataImpact::Preserving)
    }
}

impl CliCommandLeaf for IssuePluginTokenArgs {
    const NAME: &'static str = "issue-plugin-token";
    const ABOUT: &'static str =
        "Mint a long-lived RS256 JWT with `aud=plugin` for hook/governance authentication";

    fn run(
        self,
        _host: &dyn CommandHost,
    ) -> impl Future<Output = Result<CommandOutput, CommandError>> + Send {
        std::future::ready(Err(CommandError::new(KernelHostOnly)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct IssuePluginTokenOutput {
    pub plugin_id: PluginId,
    pub email: String,
    pub expires_in_days: u32,
    pub jti: AccessTokenId,
    pub token: String,
}

pub async fn execute(args: IssuePluginTokenArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let profile = host.profile()?;
    let boot = crate::commands::admin::state::boot(host)?;
    let secrets = boot.secrets().get();

    if args.duration_days == 0 || args.duration_days > 365 {
        anyhow::bail!(
            "--duration-days must be between 1 and 365 (got {})",
            args.duration_days
        );
    }

    let email = match args.email.clone() {
        Some(e) => e,
        None => resolve_email().await?,
    };

    let database_url = secrets.effective_database_url(profile.database.external_db_access);
    let db = Database::new_postgres(database_url)
        .await
        .context("Failed to connect to database")?;
    let db_pool = DbPool::from(Arc::new(db));

    let user_service = UserService::new(Arc::new(UserRepository::new(&db_pool)));
    let user = user_service
        .find_by_email(&email)
        .await
        .context("Failed to look up admin user")?
        .with_context(|| format!("User '{email}' not found in database"))?;
    if !user.is_admin() {
        anyhow::bail!("User '{}' is not an admin — refusing to mint", email);
    }

    let subject = PluginTokenSubject {
        id: user.id.clone(),
        username: user.name,
        email: user.email,
    };

    let keys = crate::session::api::load_signing_keys(&boot)?;
    let session_id = crate::session::api::create_local_session_row(
        &db_pool,
        &user.id,
        chrono::Duration::days(i64::from(args.duration_days)),
        keys.clone(),
        &boot,
    )
    .await
    .context("Failed to create the session row backing the plugin token")?;

    let issued = PluginTokenService::issue(
        subject,
        &DeploymentTokenAuthority::new(keys, profile.security.issuer.clone()),
        args.plugin_id.clone(),
        args.duration_days,
        &session_id,
    )
    .context("Failed to mint plugin-scope JWT")?;

    let output = IssuePluginTokenOutput {
        plugin_id: args.plugin_id,
        email,
        expires_in_days: args.duration_days,
        jti: issued.jti,
        token: issued.token.clone(),
    };

    if args.token_only {
        return Ok(CommandOutput::text(&issued.token));
    }

    Ok(CommandOutput::card_value("Plugin-scope JWT", &output))
}

async fn resolve_email() -> Result<String> {
    let creds = CredentialsBootstrap::load()
        .await
        .context("Failed to initialize credentials")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No --email provided and no credentials available. Pass --email or run \
                 `systemprompt cloud auth login` first."
            )
        })?;
    Ok(creds.user_email.as_str().to_owned())
}
