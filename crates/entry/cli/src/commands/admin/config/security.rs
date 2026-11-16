//! `admin config security` — show and edit the profile's security section.
//!
//! Parses the operator's arguments and delegates the mutation to
//! [`SecurityConfigService`], then revalidates the whole profile before
//! writing it back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use super::changes_output;
use crate::commands::admin::config::restart_note;
use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::{SecurityChange, SecurityConfigService, SecurityUpdate};
use systemprompt_manifest::profile::TrustedIssuer;

use systemprompt_extension::cli::CommandOutput;
use super::profile_io::{load_profile, profile_path, save_profile};
use super::types::{SecurityConfigOutput, SecuritySetOutput};

#[derive(Debug, Subcommand)]
pub enum SecurityCommands {
    #[command(about = "Show security configuration", alias = "list")]
    Show,

    #[command(about = "Set security configuration value")]
    Set(SetArgs),

    #[command(subcommand, about = "Manage federated trusted JWT issuers")]
    TrustedIssuer(TrustedIssuerCommands),
}

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(long, help = "Access token expiry in seconds")]
    pub access_expiry: Option<i64>,

    #[arg(long, help = "Refresh token expiry in seconds")]
    pub refresh_expiry: Option<i64>,

    #[arg(
        long = "resource-audience",
        help = "Resource audience to allow (repeatable). Gateway-required audiences are always kept."
    )]
    pub resource_audiences: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum TrustedIssuerCommands {
    #[command(about = "Add or replace a trusted issuer")]
    Add(TrustedIssuerAddArgs),

    #[command(about = "Remove a trusted issuer by its issuer URL")]
    Remove {
        #[arg(long, help = "Issuer URL to remove")]
        issuer: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct TrustedIssuerAddArgs {
    #[arg(long, help = "Issuer URL (iss claim)")]
    pub issuer: String,

    #[arg(long, help = "JWKS URI for signature verification")]
    pub jwks_uri: String,

    #[arg(long, help = "Expected audience claim")]
    pub audience: String,
}

pub fn execute(command: &SecurityCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match command {
        SecurityCommands::Show => execute_show(host),
        SecurityCommands::Set(args) => execute_set(args, host),
        SecurityCommands::TrustedIssuer(cmd) => execute_trusted_issuer(cmd, host),
    }
}

pub(super) fn execute_show(host: &dyn KernelHost) -> Result<CommandOutput> {
    let profile = host.profile()?;

    let output = SecurityConfigOutput {
        jwt_issuer: profile.security.issuer.clone(),
        access_token_expiry_seconds: profile.security.access_token_expiration,
        refresh_token_expiry_seconds: profile.security.refresh_token_expiration,
        audiences: profile
            .security
            .audiences
            .iter()
            .map(ToString::to_string)
            .collect(),
    };

    Ok(CommandOutput::card_value("Security Configuration", &output))
}

pub(super) fn execute_set(args: &SetArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    if args.access_expiry.is_none()
        && args.refresh_expiry.is_none()
        && args.resource_audiences.is_empty()
    {
        bail!(
            "Must specify at least one option: --access-expiry, --refresh-expiry, --resource-audience"
        );
    }

    let profile_path = profile_path(host)?;
    let mut profile = load_profile(profile_path)?;

    let update = SecurityUpdate {
        access_token_expiration: args.access_expiry,
        refresh_token_expiration: args.refresh_expiry,
        resource_audiences: args.resource_audiences.clone(),
    };
    let changes: Vec<SecuritySetOutput> =
        SecurityConfigService::apply_update(&mut profile.security, &update)?
            .into_iter()
            .map(to_output)
            .collect();

    save_profile(&profile, profile_path)?;

    Ok(render_changes(&changes))
}

fn execute_trusted_issuer(command: &TrustedIssuerCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    let profile_path = profile_path(host)?;
    let mut profile = load_profile(profile_path)?;

    let change = match command {
        TrustedIssuerCommands::Add(args) => {
            if args.issuer.is_empty() || args.jwks_uri.is_empty() || args.audience.is_empty() {
                bail!("--issuer, --jwks-uri, and --audience are all required");
            }
            SecurityConfigService::upsert_trusted_issuer(
                &mut profile.security,
                TrustedIssuer {
                    issuer: args.issuer.clone(),
                    jwks_uri: args.jwks_uri.clone(),
                    audience: args.audience.clone(),
                    typ_allowlist: Vec::new(),
                    allowed_client_ids: Vec::new(),
                    can_issue_id_jag: false,
                    link: systemprompt_traits::LinkPolicy::default(),
                },
            )
        },
        TrustedIssuerCommands::Remove { issuer } => {
            SecurityConfigService::remove_trusted_issuer(&mut profile.security, issuer)?
        },
    };

    save_profile(&profile, profile_path)?;
    Ok(render_changes(&[to_output(change)]))
}

fn to_output(change: SecurityChange) -> SecuritySetOutput {
    SecuritySetOutput {
        field: change.field,
        old_value: change.old_value,
        new_value: change.new_value,
        message: change.message,
    }
}

fn render_changes(changes: &[SecuritySetOutput]) -> CommandOutput {
    changes_output("Security Updated", changes).with_note(restart_note())
}
