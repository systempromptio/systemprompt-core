use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use std::fs;
use systemprompt_logging::CliService;
use systemprompt_models::{Profile, ProfileBootstrap};

use super::types::{SecurityConfigOutput, SecuritySetOutput};
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum SecurityCommands {
    #[command(about = "Show security configuration")]
    Show,

    #[command(about = "Set security configuration value")]
    Set(SetArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(long, help = "JWT issuer")]
    pub jwt_issuer: Option<String>,

    #[arg(long, help = "Access token expiry in seconds")]
    pub access_expiry: Option<i64>,

    #[arg(long, help = "Refresh token expiry in seconds")]
    pub refresh_expiry: Option<i64>,
}

pub fn execute(command: &SecurityCommands, config: &CliConfig) -> Result<()> {
    match command {
        SecurityCommands::Show => execute_show(),
        SecurityCommands::Set(args) => execute_set(args, config),
    }
}

fn execute_show() -> Result<()> {
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

    render_result(&CommandResult::card(output).with_title("Security Configuration"));

    Ok(())
}

fn execute_set(args: &SetArgs, config: &CliConfig) -> Result<()> {
    if args.jwt_issuer.is_none() && args.access_expiry.is_none() && args.refresh_expiry.is_none() {
        bail!("Must specify at least one option: --jwt-issuer, --access-expiry, --refresh-expiry");
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let mut changes: Vec<SecuritySetOutput> = Vec::new();

    if let Some(ref issuer) = args.jwt_issuer {
        let old = profile.security.issuer.clone();
        profile.security.issuer.clone_from(issuer);
        changes.push(SecuritySetOutput {
            field: "jwt_issuer".to_string(),
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
            field: "access_token_expiration".to_string(),
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
            field: "refresh_token_expiration".to_string(),
            old_value: old.to_string(),
            new_value: expiry.to_string(),
            message: format!("Updated refresh token expiry to {} seconds", expiry),
        });
    }

    save_profile(&profile, profile_path)?;

    for change in &changes {
        render_result(&CommandResult::text(change.clone()).with_title("Security Updated"));
    }

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

fn load_profile(path: &str) -> Result<Profile> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read profile: {}", path))?;
    let profile: Profile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse profile: {}", path))?;
    Ok(profile)
}

fn save_profile(profile: &Profile, path: &str) -> Result<()> {
    let content = serde_yaml::to_string(profile).context("Failed to serialize profile")?;
    fs::write(path, content).with_context(|| format!("Failed to write profile: {}", path))?;
    Ok(())
}
