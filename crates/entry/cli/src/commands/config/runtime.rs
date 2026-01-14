use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use std::fs;
use systemprompt_core_logging::CliService;
use systemprompt_models::profile::{Environment, LogLevel, OutputFormat as ProfileOutputFormat};
use systemprompt_models::{Profile, ProfileBootstrap};

use super::types::{RuntimeConfigOutput, RuntimeSetOutput};
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum RuntimeCommands {
    #[command(about = "Show runtime configuration")]
    Show,

    #[command(about = "Set runtime configuration value")]
    Set(SetArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(long, help = "Environment: development, test, staging, production")]
    pub environment: Option<String>,

    #[arg(long, help = "Log level: quiet, normal, verbose, debug")]
    pub log_level: Option<String>,

    #[arg(long, help = "Output format: text, json, yaml")]
    pub output_format: Option<String>,

    #[arg(long, help = "Disable colored output")]
    pub no_color: Option<bool>,
}

pub fn execute(command: RuntimeCommands, config: &CliConfig) -> Result<()> {
    match command {
        RuntimeCommands::Show => execute_show(),
        RuntimeCommands::Set(args) => execute_set(args, config),
    }
}

fn execute_show() -> Result<()> {
    let profile = ProfileBootstrap::get()?;

    let output = RuntimeConfigOutput {
        environment: profile.runtime.environment.to_string(),
        log_level: profile.runtime.log_level.to_string(),
        output_format: profile.runtime.output_format.to_string(),
        no_color: profile.runtime.no_color,
        non_interactive: profile.runtime.non_interactive,
    };

    render_result(&CommandResult::card(output).with_title("Runtime Configuration"));

    Ok(())
}

fn execute_set(args: SetArgs, config: &CliConfig) -> Result<()> {
    if args.environment.is_none()
        && args.log_level.is_none()
        && args.output_format.is_none()
        && args.no_color.is_none()
    {
        bail!(
            "Must specify at least one option: --environment, --log-level, --output-format, --no-color"
        );
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let mut changes: Vec<RuntimeSetOutput> = Vec::new();

    if let Some(env_str) = args.environment {
        let env: Environment = env_str
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?;
        let old = profile.runtime.environment.to_string();
        profile.runtime.environment = env;
        changes.push(RuntimeSetOutput {
            field: "environment".to_string(),
            old_value: old,
            new_value: env.to_string(),
            message: format!("Updated environment to {}", env),
        });
    }

    if let Some(level_str) = args.log_level {
        let level: LogLevel = level_str
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?;
        let old = profile.runtime.log_level.to_string();
        profile.runtime.log_level = level;
        changes.push(RuntimeSetOutput {
            field: "log_level".to_string(),
            old_value: old,
            new_value: level.to_string(),
            message: format!("Updated log_level to {}", level),
        });
    }

    if let Some(format_str) = args.output_format {
        let format: ProfileOutputFormat = format_str
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?;
        let old = profile.runtime.output_format.to_string();
        profile.runtime.output_format = format;
        changes.push(RuntimeSetOutput {
            field: "output_format".to_string(),
            old_value: old,
            new_value: format.to_string(),
            message: format!("Updated output_format to {}", format),
        });
    }

    if let Some(no_color) = args.no_color {
        let old = profile.runtime.no_color;
        profile.runtime.no_color = no_color;
        changes.push(RuntimeSetOutput {
            field: "no_color".to_string(),
            old_value: old.to_string(),
            new_value: no_color.to_string(),
            message: format!("Updated no_color to {}", no_color),
        });
    }

    save_profile(&profile, profile_path)?;

    for change in &changes {
        render_result(&CommandResult::text(change.clone()).with_title("Runtime Updated"));
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
