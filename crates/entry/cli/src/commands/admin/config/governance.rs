//! `admin config governance` — set the authorization hook mode.
//!
//! Enforces the mode's invariants (webhook needs a URL; unrestricted needs the
//! exact acknowledgement sentence) at edit time so a misconfigured governance
//! block cannot reach the fail-closed bootstrap check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;
use systemprompt_models::profile::{
    AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig, UNRESTRICTED_ACKNOWLEDGEMENT,
};

use super::profile_io::{load_profile, save_profile};
use super::types::ConfigMutationOutput;
use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Subcommand)]
pub enum GovernanceCommands {
    #[command(about = "Show governance configuration")]
    Show,

    #[command(about = "Set the authorization hook")]
    Set(SetArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(
        long,
        help = "Authz mode: webhook | extension | disabled | unrestricted"
    )]
    pub mode: String,

    #[arg(long, help = "Webhook URL (required for mode=webhook)")]
    pub url: Option<String>,

    #[arg(long, help = "Webhook timeout in milliseconds")]
    pub timeout_ms: Option<u64>,

    #[arg(
        long,
        help = "Acknowledgement sentence (required for mode=unrestricted)"
    )]
    pub acknowledgement: Option<String>,
}

pub fn execute(command: &GovernanceCommands, config: &CliConfig) -> Result<()> {
    match command {
        GovernanceCommands::Show => execute_show(config),
        GovernanceCommands::Set(args) => execute_set(args, config),
    }
}

fn parse_mode(raw: &str) -> Result<AuthzMode> {
    match raw.to_lowercase().as_str() {
        "webhook" => Ok(AuthzMode::Webhook),
        "extension" => Ok(AuthzMode::Extension),
        "disabled" => Ok(AuthzMode::Disabled),
        "unrestricted" => Ok(AuthzMode::Unrestricted),
        other => bail!("unknown authz mode '{other}' (webhook|extension|disabled|unrestricted)"),
    }
}

fn execute_set(args: &SetArgs, config: &CliConfig) -> Result<()> {
    let mode = parse_mode(&args.mode)?;

    if matches!(mode, AuthzMode::Webhook) && args.url.is_none() {
        bail!("mode=webhook requires --url");
    }
    if matches!(mode, AuthzMode::Unrestricted)
        && args.acknowledgement.as_deref() != Some(UNRESTRICTED_ACKNOWLEDGEMENT)
    {
        bail!(
            "mode=unrestricted requires --acknowledgement \"{}\"",
            UNRESTRICTED_ACKNOWLEDGEMENT
        );
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    profile.governance = Some(GovernanceConfig {
        authz: Some(AuthzConfig {
            hook: AuthzHookConfig {
                mode,
                url: args.url.clone(),
                timeout_ms: args.timeout_ms.unwrap_or(500),
                acknowledgement: args.acknowledgement.clone(),
            },
        }),
    });

    save_profile(&profile, profile_path)?;

    render_result(
        &CommandOutput::card_value(
            "Governance Updated",
            &ConfigMutationOutput {
                field: "governance.authz".to_owned(),
                message: format!("Authz mode set to {}", args.mode.to_lowercase()),
            },
        ),
        config,
    );
    Ok(())
}

fn execute_show(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let summary = profile
        .governance
        .as_ref()
        .and_then(|g| g.authz.as_ref())
        .map_or_else(
            || "authz: none (fail-closed deny-all)".to_owned(),
            |authz| {
                authz.hook.url.as_deref().map_or_else(
                    || "authz mode set".to_owned(),
                    |url| format!("authz mode set, url={url}"),
                )
            },
        );
    render_result(
        &CommandOutput::text_titled("Governance Configuration", summary),
        config,
    );
    Ok(())
}
