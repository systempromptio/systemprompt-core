use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use std::fs;
use systemprompt_core_logging::CliService;
use systemprompt_models::{Profile, ProfileBootstrap};

use super::types::{CorsListOutput, CorsModifyOutput, ServerConfigOutput, ServerSetOutput};
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum ServerCommands {
    #[command(about = "Show server configuration")]
    Show,

    #[command(about = "Set server configuration value")]
    Set(SetArgs),

    #[command(subcommand, about = "Manage CORS allowed origins")]
    Cors(CorsCommands),
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

#[derive(Debug, Subcommand)]
pub enum CorsCommands {
    #[command(about = "List CORS allowed origins")]
    List,

    #[command(about = "Add a CORS origin")]
    Add(CorsAddArgs),

    #[command(about = "Remove a CORS origin")]
    Remove(CorsRemoveArgs),
}

#[derive(Debug, Clone, Args)]
pub struct CorsAddArgs {
    #[arg(help = "Origin URL to add (e.g., https://example.com)")]
    pub origin: String,
}

#[derive(Debug, Clone, Args)]
pub struct CorsRemoveArgs {
    #[arg(help = "Origin URL to remove")]
    pub origin: String,
}

pub fn execute(command: ServerCommands, config: &CliConfig) -> Result<()> {
    match command {
        ServerCommands::Show => execute_show(config),
        ServerCommands::Set(args) => execute_set(args, config),
        ServerCommands::Cors(cmd) => execute_cors(cmd, config),
    }
}

fn execute_show(_config: &CliConfig) -> Result<()> {
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

fn execute_set(args: SetArgs, config: &CliConfig) -> Result<()> {
    if args.host.is_none()
        && args.port.is_none()
        && args.use_https.is_none()
        && args.api_server_url.is_none()
        && args.api_internal_url.is_none()
        && args.api_external_url.is_none()
    {
        bail!("Must specify at least one option: --host, --port, --use-https, --api-server-url, --api-internal-url, --api-external-url");
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let mut changes: Vec<ServerSetOutput> = Vec::new();

    if let Some(host) = args.host {
        let old = profile.server.host.clone();
        profile.server.host = host.clone();
        changes.push(ServerSetOutput {
            field: "host".to_string(),
            old_value: old,
            new_value: host.clone(),
            message: format!("Updated host to {}", host),
        });
    }

    if let Some(port) = args.port {
        let old = profile.server.port;
        profile.server.port = port;
        changes.push(ServerSetOutput {
            field: "port".to_string(),
            old_value: old.to_string(),
            new_value: port.to_string(),
            message: format!("Updated port to {}", port),
        });
    }

    if let Some(use_https) = args.use_https {
        let old = profile.server.use_https;
        profile.server.use_https = use_https;
        changes.push(ServerSetOutput {
            field: "use_https".to_string(),
            old_value: old.to_string(),
            new_value: use_https.to_string(),
            message: format!("Updated use_https to {}", use_https),
        });
    }

    if let Some(url) = args.api_server_url {
        let old = profile.server.api_server_url.clone();
        profile.server.api_server_url = url.clone();
        changes.push(ServerSetOutput {
            field: "api_server_url".to_string(),
            old_value: old,
            new_value: url.clone(),
            message: format!("Updated api_server_url to {}", url),
        });
    }

    if let Some(url) = args.api_internal_url {
        let old = profile.server.api_internal_url.clone();
        profile.server.api_internal_url = url.clone();
        changes.push(ServerSetOutput {
            field: "api_internal_url".to_string(),
            old_value: old,
            new_value: url.clone(),
            message: format!("Updated api_internal_url to {}", url),
        });
    }

    if let Some(url) = args.api_external_url {
        let old = profile.server.api_external_url.clone();
        profile.server.api_external_url = url.clone();
        changes.push(ServerSetOutput {
            field: "api_external_url".to_string(),
            old_value: old,
            new_value: url.clone(),
            message: format!("Updated api_external_url to {}", url),
        });
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

fn execute_cors(command: CorsCommands, config: &CliConfig) -> Result<()> {
    match command {
        CorsCommands::List => execute_cors_list(),
        CorsCommands::Add(args) => execute_cors_add(args, config),
        CorsCommands::Remove(args) => execute_cors_remove(args, config),
    }
}

fn execute_cors_list() -> Result<()> {
    let profile = ProfileBootstrap::get()?;

    let output = CorsListOutput {
        origins: profile.server.cors_allowed_origins.clone(),
        count: profile.server.cors_allowed_origins.len(),
    };

    render_result(&CommandResult::list(output).with_title("CORS Allowed Origins"));

    Ok(())
}

fn execute_cors_add(args: CorsAddArgs, config: &CliConfig) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    if profile.server.cors_allowed_origins.contains(&args.origin) {
        let output = CorsModifyOutput {
            action: "skipped".to_string(),
            origin: args.origin.clone(),
            message: format!("Origin {} already exists", args.origin),
        };
        render_result(&CommandResult::text(output).with_title("CORS Origin"));
        return Ok(());
    }

    profile.server.cors_allowed_origins.push(args.origin.clone());
    save_profile(&profile, profile_path)?;

    let output = CorsModifyOutput {
        action: "added".to_string(),
        origin: args.origin.clone(),
        message: format!("Added CORS origin: {}", args.origin),
    };
    render_result(&CommandResult::text(output).with_title("CORS Origin Added"));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

fn execute_cors_remove(args: CorsRemoveArgs, config: &CliConfig) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let original_len = profile.server.cors_allowed_origins.len();
    profile
        .server
        .cors_allowed_origins
        .retain(|o| o != &args.origin);

    if profile.server.cors_allowed_origins.len() == original_len {
        let output = CorsModifyOutput {
            action: "skipped".to_string(),
            origin: args.origin.clone(),
            message: format!("Origin {} not found", args.origin),
        };
        render_result(&CommandResult::text(output).with_title("CORS Origin"));
        return Ok(());
    }

    save_profile(&profile, profile_path)?;

    let output = CorsModifyOutput {
        action: "removed".to_string(),
        origin: args.origin.clone(),
        message: format!("Removed CORS origin: {}", args.origin),
    };
    render_result(&CommandResult::text(output).with_title("CORS Origin Removed"));

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
