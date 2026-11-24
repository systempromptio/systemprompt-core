//! `admin keys issue-plugin-token`: a long-lived hook credential for an
//! admin.
//!
//! The command reaches the mint only through the `DynPluginTokenMinter` the
//! oauth member confers on `identity-http` (`identity-plugin-tokens`). That
//! minter mints one token shape (`aud=hook` plus the `plugin` resource
//! audience, `HookGovern` + `HookTrack`, the `plugin_id` claim) for one to
//! 365 days, for an active admin only, over a fresh CLI session row. With no
//! `--email` the token is minted for the caller's own CLI session user.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_identifiers::{AccessTokenId, PluginId};
use systemprompt_traits::{DynPluginTokenMinter, PluginTokenHolder, PluginTokenRequest};

#[derive(Debug, Args)]
pub struct IssuePluginTokenArgs {
    #[arg(
        long,
        help = "Admin email to mint the token for. Defaults to the user of the active CLI session."
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
struct IssuePluginTokenOutput {
    plugin_id: PluginId,
    email: String,
    expires_in_days: u32,
    jti: AccessTokenId,
    token: String,
}

pub(super) async fn execute(
    args: IssuePluginTokenArgs,
    host: &dyn CommandHost,
) -> Result<CommandOutput> {
    let holder = match args.email {
        Some(email) => PluginTokenHolder::Email(email),
        None => PluginTokenHolder::User(host.caller_session().await?.user_id().clone()),
    };
    let tokens = host
        .resolved()
        .await?
        .get::<DynPluginTokenMinter>()
        .context("the plugin-token minter is not available on this host")?;
    let minted = tokens
        .mint(PluginTokenRequest {
            holder,
            plugin_id: args.plugin_id.clone(),
            duration_days: args.duration_days,
        })
        .await
        .context("Failed to mint plugin-scope JWT")?;

    if args.token_only {
        return Ok(CommandOutput::text(&minted.token));
    }
    Ok(CommandOutput::card_value(
        "Plugin-scope JWT",
        &IssuePluginTokenOutput {
            plugin_id: args.plugin_id,
            email: minted.email,
            expires_in_days: args.duration_days,
            jti: minted.jti,
            token: minted.token,
        },
    ))
}
