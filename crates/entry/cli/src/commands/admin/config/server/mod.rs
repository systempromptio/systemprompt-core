//! `admin config server` command: show and edit profile server settings.
//!
//! [`ServerCommands`] reports and updates host, port, URLs, and HTTPS settings,
//! and delegates the CORS allowed-origins list to the `cors` submodule. Changes
//! persist to the active profile.

mod cors;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use std::fs;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use super::types::{ServerConfigOutput, ServerSetOutput};
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandResult, render_result};

#[derive(Debug, Subcommand)]
pub enum ServerCommands {
    #[command(about = "Show server configuration")]
    Show,

    #[command(about = "Set server configuration value")]
    Set(SetArgs),

    #[command(subcommand, about = "Manage CORS allowed origins")]
    Cors(cors::CorsCommands),
}

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(long, help = "Server host address")]
    pub host: Option<String>,

    #[arg(long, help = "Server port")]
    pub port: Option<u16>,

    #[arg(long, help = "Enable/disable HTTPS")]
    pub use_https: Option<bool>,

    #[arg(long, help = "API server URL")]
    pub api_server_url: Option<String>,

    #[arg(long, help = "API internal URL")]
    pub api_internal_url: Option<String>,

    #[arg(long, help = "API external URL")]
    pub api_external_url: Option<String>,
}

pub fn execute(command: &ServerCommands, config: &CliConfig) -> Result<()> {
    match command {
        ServerCommands::Show => execute_show(config),
        ServerCommands::Set(args) => execute_set(args, config),
        ServerCommands::Cors(cmd) => cors::execute(cmd, config),
    }
}

pub(super) fn execute_show(_config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;

    let output = ServerConfigOutput {
        host: profile.server.host.clone(),
        port: profile.server.port,
        api_server_url: profile.server.api_server_url.clone(),
        api_internal_url: profile.server.api_internal_url.clone(),
        api_external_url: profile.server.api_external_url.clone(),
        use_https: profile.server.use_https,
        cors_allowed_origins: profile.server.cors_allowed_origins.clone(),
    };

    render_result(&CommandResult::card(output).with_title("Server Configuration"));

    Ok(())
}

fn change(field: &str, old: String, new: String) -> ServerSetOutput {
    ServerSetOutput {
        field: field.to_owned(),
        message: format!("Updated {field} to {new}"),
        old_value: old,
        new_value: new,
    }
}

pub(super) fn execute_set(args: &SetArgs, config: &CliConfig) -> Result<()> {
    if args.host.is_none()
        && args.port.is_none()
        && args.use_https.is_none()
        && args.api_server_url.is_none()
        && args.api_internal_url.is_none()
        && args.api_external_url.is_none()
    {
        bail!(
            "Must specify at least one option: --host, --port, --use-https, --api-server-url, \
             --api-internal-url, --api-external-url"
        );
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let mut changes: Vec<ServerSetOutput> = Vec::new();

    if let Some(ref host) = args.host {
        let old = profile.server.host.clone();
        profile.server.host.clone_from(host);
        changes.push(change("host", old, host.clone()));
    }

    if let Some(port) = args.port {
        let old = profile.server.port;
        profile.server.port = port;
        changes.push(change("port", old.to_string(), port.to_string()));
    }

    if let Some(use_https) = args.use_https {
        let old = profile.server.use_https;
        profile.server.use_https = use_https;
        changes.push(change("use_https", old.to_string(), use_https.to_string()));
    }

    if let Some(ref url) = args.api_server_url {
        let old = profile.server.api_server_url.clone();
        profile.server.api_server_url.clone_from(url);
        changes.push(change("api_server_url", old, url.clone()));
    }

    if let Some(ref url) = args.api_internal_url {
        let old = profile.server.api_internal_url.clone();
        profile.server.api_internal_url.clone_from(url);
        changes.push(change("api_internal_url", old, url.clone()));
    }

    if let Some(ref url) = args.api_external_url {
        let old = profile.server.api_external_url.clone();
        profile.server.api_external_url.clone_from(url);
        changes.push(change("api_external_url", old, url.clone()));
    }

    save_profile(&profile, profile_path)?;

    for change in &changes {
        render_result(&CommandResult::text(change.clone()).with_title("Server Updated"));
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

pub(super) fn save_profile(profile: &Profile, path: &str) -> Result<()> {
    let content = serde_yaml::to_string(profile).context("Failed to serialize profile")?;
    fs::write(path, content).with_context(|| format!("Failed to write profile: {}", path))?;
    Ok(())
}
