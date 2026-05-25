use crate::CliConfig;
use anyhow::{Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use std::path::{Path, PathBuf};
use systemprompt_logging::CliService;

use super::common::PostgresConfig;
use super::{SetupArgs, postgres, secrets};

pub(super) fn get_environment_name(args: &SetupArgs, config: &CliConfig) -> Result<String> {
    if let Some(ref env) = args.environment {
        return Ok(env.clone());
    }

    if !config.is_interactive() {
        return Ok("dev".to_owned());
    }

    CliService::info("Enter environment name (e.g., 'dev', 'staging', 'prod')");
    CliService::info("Press Enter for default: dev");

    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Environment name")
        .default("dev".to_owned())
        .interact_text()?;

    Ok(input.trim().to_lowercase())
}

pub(super) async fn setup_postgres(
    args: &SetupArgs,
    config: &CliConfig,
    env_name: &str,
) -> Result<PostgresConfig> {
    if !config.is_interactive() {
        return postgres::setup_non_interactive(args, env_name, config).await;
    }
    postgres::setup_interactive(args, env_name, config).await
}

pub(super) fn collect_secrets(
    args: &SetupArgs,
    config: &CliConfig,
    env_name: &str,
) -> Result<secrets::SecretsData> {
    if !config.is_interactive() {
        return secrets::collect_non_interactive(args, config);
    }
    secrets::collect_interactive(args, env_name, config)
}

pub(super) fn should_run_migrations(args: &SetupArgs, config: &CliConfig) -> Result<bool> {
    if args.migrate {
        return Ok(true);
    }
    if args.no_migrate {
        return Ok(false);
    }
    if !config.is_interactive() {
        return Ok(false);
    }

    let run = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Run database migrations now?")
        .default(true)
        .interact()?;

    Ok(run)
}

pub(super) fn detect_project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    let indicators = ["Cargo.toml", "services", ".systemprompt", "core"];

    for indicator in indicators {
        if cwd.join(indicator).exists() {
            return Ok(cwd);
        }
    }

    let mut current = cwd.clone();
    for _ in 0..5 {
        if let Some(parent) = current.parent() {
            for indicator in indicators {
                if parent.join(indicator).exists() {
                    return Ok(parent.to_path_buf());
                }
            }
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    Ok(cwd)
}

pub(super) fn print_summary(env_name: &str, profile_path: &Path) {
    CliService::section("Setup Complete!");

    CliService::info(&format!(
        "Created profile: {} -> {}",
        env_name,
        profile_path.display()
    ));

    CliService::section("Next Steps");

    CliService::info(&format!(
        "1. Set your profile environment variable for '{}':",
        env_name
    ));
    CliService::info(&format!(
        "   export SYSTEMPROMPT_PROFILE={}",
        profile_path.display()
    ));
    CliService::info("");
    CliService::info("2. Start services:");
    CliService::info("   just start");
    CliService::info("");
    CliService::info("3. (Optional) Configure cloud deployment:");
    CliService::info("   systemprompt cloud login");
    CliService::info("   systemprompt cloud config");
}
