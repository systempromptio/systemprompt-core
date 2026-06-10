//! `admin config security` — show and edit the profile's security section.
//!
//! Parses the operator's arguments and delegates the mutation to
//! [`SecurityConfigService`], then revalidates the whole profile before
//! writing it back.

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::{
    ProfileBootstrap, SecurityChange, SecurityConfigService, SecurityUpdate,
};
use systemprompt_logging::CliService;
use systemprompt_models::profile::TrustedIssuer;

use super::profile_io::{load_profile, save_profile};
use super::types::{SecurityConfigOutput, SecuritySetOutput};
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandOutput, render_result};

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
    #[arg(long, help = "JWT issuer")]
    pub jwt_issuer: Option<String>,

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

pub fn execute(command: &SecurityCommands, config: &CliConfig) -> Result<()> {
    match command {
        SecurityCommands::Show => execute_show(config),
        SecurityCommands::Set(args) => execute_set(args, config),
        SecurityCommands::TrustedIssuer(cmd) => execute_trusted_issuer(cmd, config),
    }
}

pub(super) fn execute_show(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;

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

    render_result(
        &CommandOutput::card_value("Security Configuration", &output),
        config,
    );

    Ok(())
}

pub(super) fn execute_set(args: &SetArgs, config: &CliConfig) -> Result<()> {
    if args.jwt_issuer.is_none()
        && args.access_expiry.is_none()
        && args.refresh_expiry.is_none()
        && args.resource_audiences.is_empty()
    {
        bail!(
            "Must specify at least one option: --jwt-issuer, --access-expiry, --refresh-expiry, --resource-audience"
        );
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let update = SecurityUpdate {
        jwt_issuer: args.jwt_issuer.clone(),
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
    render_changes(&changes, config);
    Ok(())
}

fn execute_trusted_issuer(command: &TrustedIssuerCommands, config: &CliConfig) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
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
                },
            )
        },
        TrustedIssuerCommands::Remove { issuer } => {
            SecurityConfigService::remove_trusted_issuer(&mut profile.security, issuer)?
        },
    };

    save_profile(&profile, profile_path)?;
    render_changes(&[to_output(change)], config);
    Ok(())
}

fn to_output(change: SecurityChange) -> SecuritySetOutput {
    SecuritySetOutput {
        field: change.field,
        old_value: change.old_value,
        new_value: change.new_value,
        message: change.message,
    }
}

fn render_changes(changes: &[SecuritySetOutput], config: &CliConfig) {
    for change in changes {
        render_result(
            &CommandOutput::card_value("Security Updated", change),
            config,
        );
    }
    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }
}
