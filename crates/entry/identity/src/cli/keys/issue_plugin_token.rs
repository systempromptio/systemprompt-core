//! `admin keys issue-plugin-token` command minting a hook-scoped credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};

use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_extension::cli::{CommandHost, CommandOutput, PluginTokenGrant};
use systemprompt_identifiers::{AccessTokenId, PluginId, SessionSource};
use systemprompt_users::admin_bootstrap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct IssuePluginTokenOutput {
    pub plugin_id: PluginId,
    pub email: String,
    pub expires_in_days: u32,
    pub jti: AccessTokenId,
    pub token: String,
}

pub(super) async fn execute(
    args: IssuePluginTokenArgs,
    host: &dyn CommandHost,
) -> Result<CommandOutput> {
    if args.duration_days == 0 || args.duration_days > 365 {
        anyhow::bail!(
            "--duration-days must be between 1 and 365 (got {})",
            args.duration_days
        );
    }
    let signing_keys = host.signing_keys()?;

    let email = match args.email.clone() {
        Some(e) => e,
        None => resolve_email().await?,
    };

    let admins = admin_bootstrap(host.db().await?).context("Failed to open the user store")?;
    let user = admins
        .find_by_email(&email)
        .await
        .context("Failed to look up admin user")?
        .with_context(|| format!("User '{email}' not found in database"))?;
    if !user.is_admin() {
        anyhow::bail!("User '{}' is not an admin — refusing to mint", email);
    }

    let session_id = admins
        .open_session(
            &user.id,
            chrono::Duration::days(i64::from(args.duration_days)),
            SessionSource::Cli,
        )
        .await
        .context("Failed to create the session row backing the plugin token")?;

    let issued = signing_keys
        .mint_plugin_token(PluginTokenGrant {
            user_id: user.id,
            username: user.name,
            email: user.email,
            plugin_id: args.plugin_id.clone(),
            duration_days: args.duration_days,
            session_id,
        })
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
