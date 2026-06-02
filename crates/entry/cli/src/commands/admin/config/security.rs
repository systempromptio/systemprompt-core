use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::profile::{TrustedIssuer, default_resource_audiences};

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
        SecurityCommands::Show => execute_show(),
        SecurityCommands::Set(args) => execute_set(args, config),
        SecurityCommands::TrustedIssuer(cmd) => execute_trusted_issuer(cmd, config),
    }
}

pub(super) fn execute_show() -> Result<()> {
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

    render_result(&CommandOutput::card_value(
        "Security Configuration",
        &output,
    ));

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
    let mut changes: Vec<SecuritySetOutput> = Vec::new();

    if let Some(ref issuer) = args.jwt_issuer {
        let old = profile.security.issuer.clone();
        profile.security.issuer.clone_from(issuer);
        changes.push(SecuritySetOutput {
            field: "jwt_issuer".to_owned(),
            old_value: old,
            new_value: issuer.clone(),
            message: format!("Updated JWT issuer to {}", issuer),
        });
    }

    if let Some(expiry) = args.access_expiry {
        if expiry <= 0 {
            bail!("Access token expiry must be positive");
        }
        let old = profile.security.access_token_expiration;
        profile.security.access_token_expiration = expiry;
        changes.push(SecuritySetOutput {
            field: "access_token_expiration".to_owned(),
            old_value: old.to_string(),
            new_value: expiry.to_string(),
            message: format!("Updated access token expiry to {} seconds", expiry),
        });
    }

    if let Some(expiry) = args.refresh_expiry {
        if expiry <= 0 {
            bail!("Refresh token expiry must be positive");
        }
        let old = profile.security.refresh_token_expiration;
        profile.security.refresh_token_expiration = expiry;
        changes.push(SecuritySetOutput {
            field: "refresh_token_expiration".to_owned(),
            old_value: old.to_string(),
            new_value: expiry.to_string(),
            message: format!("Updated refresh token expiry to {} seconds", expiry),
        });
    }

    if !args.resource_audiences.is_empty() {
        let old = profile.security.allowed_resource_audiences.join(",");
        let mut merged = default_resource_audiences();
        for aud in &args.resource_audiences {
            if !merged.contains(aud) {
                merged.push(aud.clone());
            }
        }
        profile
            .security
            .allowed_resource_audiences
            .clone_from(&merged);
        changes.push(SecuritySetOutput {
            field: "allowed_resource_audiences".to_owned(),
            old_value: old,
            new_value: merged.join(","),
            message: "Updated allowed resource audiences".to_owned(),
        });
    }

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
            profile
                .security
                .trusted_issuers
                .retain(|t| t.issuer != args.issuer);
            profile.security.trusted_issuers.push(TrustedIssuer {
                issuer: args.issuer.clone(),
                jwks_uri: args.jwks_uri.clone(),
                audience: args.audience.clone(),
            });
            SecuritySetOutput {
                field: "trusted_issuers".to_owned(),
                old_value: String::new(),
                new_value: args.issuer.clone(),
                message: format!("Added trusted issuer {}", args.issuer),
            }
        },
        TrustedIssuerCommands::Remove { issuer } => {
            let before = profile.security.trusted_issuers.len();
            profile
                .security
                .trusted_issuers
                .retain(|t| &t.issuer != issuer);
            if profile.security.trusted_issuers.len() == before {
                bail!("No trusted issuer found with issuer {}", issuer);
            }
            SecuritySetOutput {
                field: "trusted_issuers".to_owned(),
                old_value: issuer.clone(),
                new_value: String::new(),
                message: format!("Removed trusted issuer {}", issuer),
            }
        },
    };

    save_profile(&profile, profile_path)?;
    render_changes(std::slice::from_ref(&change), config);
    Ok(())
}

fn render_changes(changes: &[SecuritySetOutput], config: &CliConfig) {
    for change in changes {
        render_result(&CommandOutput::card_value("Security Updated", change));
    }
    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }
}
