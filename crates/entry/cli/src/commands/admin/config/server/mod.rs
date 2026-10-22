//! `admin config server` command: show and edit profile server settings.
//!
//! [`ServerCommands`] reports and updates host, port and `external_url` (every
//! other server URL derives from those), and delegates the extra CORS origins
//! to the `cors` submodule. Changes persist to the active profile's overlay.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cors;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;

pub(super) use super::profile_io::{load_profile, save_profile};
use super::types::{ServerConfigOutput, ServerSetOutput};
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandOutput, render_result};

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

    #[arg(
        long,
        help = "Public URL; the API URLs, JWT issuer, CORS origin and HTTPS flag derive from it"
    )]
    pub external_url: Option<String>,
}

pub fn execute(command: &ServerCommands, config: &CliConfig) -> Result<()> {
    match command {
        ServerCommands::Show => execute_show(config),
        ServerCommands::Set(args) => execute_set(args, config),
        ServerCommands::Cors(cmd) => cors::execute(cmd, config),
    }
}

pub(super) fn execute_show(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;

    let output = ServerConfigOutput {
        host: profile.server.host.clone(),
        port: profile.server.port,
        external_url: profile.server.external_url.clone(),
        api_server_url: profile.server.api_server_url.clone(),
        api_internal_url: profile.server.api_internal_url.clone(),
        api_external_url: profile.server.api_external_url.clone(),
        use_https: profile.server.use_https,
        cors_allowed_origins: profile.server.cors_allowed_origins.clone(),
    };

    render_result(
        &CommandOutput::card_value("Server Configuration", &output),
        config,
    );

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
    if args.host.is_none() && args.port.is_none() && args.external_url.is_none() {
        bail!("Must specify at least one option: --host, --port, --external-url");
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

    if let Some(ref url) = args.external_url {
        let old = profile.server.external_url.clone();
        profile.server.external_url.clone_from(url);
        changes.push(change("external_url", old, url.clone()));
    }

    save_profile(&profile, profile_path)?;

    if args.external_url.is_some() {
        crate::session::clear_session(&crate::paths::ResolvedPaths::discover())?;
        CliService::info(
            "Cleared the stored CLI session: its token was minted under the previous issuer.",
        );
    }

    for change in &changes {
        render_result(&CommandOutput::card_value("Server Updated", change), config);
    }

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}
