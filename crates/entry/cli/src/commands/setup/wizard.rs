use anyhow::{Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use std::path::PathBuf;
use systemprompt_core_logging::CliService;

use super::postgres::PostgresConfig;
use super::{postgres, profile, secrets, SetupArgs};
use crate::CliConfig;

pub async fn execute(args: SetupArgs, config: &CliConfig) -> Result<()> {
    CliService::section("SystemPrompt Setup Wizard");

    let project_root = detect_project_root()?;
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("systemprompt");

    CliService::success(&format!(
        "Project: {} ({})",
        project_name,
        project_root.display()
    ));

    let systemprompt_dir = project_root.join(".systemprompt");

    let env_name = get_environment_name(&args, config)?;

    CliService::info(&format!("Configuring environment: {}", env_name));
    CliService::section(&format!("Setting up '{}' environment", env_name));

    let pg_config = setup_postgres(&args, config, &env_name).await?;

    let mut secrets_data = collect_secrets(&args, config, &env_name)?;
    secrets_data.database_url = Some(pg_config.database_url());

    let secrets_path = secrets::default_path(&systemprompt_dir, &env_name);
    secrets::save(&secrets_data, &secrets_path)?;

    let relative_secrets_path = format!("../secrets/{}.secrets.json", env_name);
    let profile_data = profile::build(&env_name, &relative_secrets_path, &project_root)?;
    let profile_path = profile::default_path(&systemprompt_dir, &env_name);
    profile::save(&profile_data, &profile_path)?;

    match profile_data.validate() {
        Ok(()) => CliService::success("Profile validated successfully"),
        Err(e) => CliService::warning(&format!("Profile validation warnings: {}", e)),
    }

    let run_migrations = should_run_migrations(&args, config)?;
    if run_migrations {
        profile::run_migrations(&profile_path)?;
    }

    print_summary(&env_name, &profile_path);

    Ok(())
}

fn get_environment_name(args: &SetupArgs, config: &CliConfig) -> Result<String> {
    if let Some(ref env) = args.environment {
        return Ok(env.clone());
    }

    if !config.is_interactive() {
        return Ok("dev".to_string());
    }

    CliService::info("Enter environment name (e.g., 'dev', 'staging', 'prod')");
    CliService::info("Press Enter for default: dev");

    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Environment name")
        .default("dev".to_string())
        .interact_text()?;

    Ok(input.trim().to_lowercase())
}

async fn setup_postgres(
    args: &SetupArgs,
    config: &CliConfig,
    env_name: &str,
) -> Result<PostgresConfig> {
    if !config.is_interactive() {
        return postgres::setup_non_interactive(args, env_name).await;
    }
    postgres::setup_interactive(args, env_name).await
}

fn collect_secrets(
    args: &SetupArgs,
    config: &CliConfig,
    env_name: &str,
) -> Result<secrets::SecretsData> {
    if !config.is_interactive() {
        return secrets::collect_non_interactive(args);
    }
    secrets::collect_interactive(args, env_name)
}

fn should_run_migrations(args: &SetupArgs, config: &CliConfig) -> Result<bool> {
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

fn detect_project_root() -> Result<PathBuf> {
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

fn print_summary(env_name: &str, profile_path: &PathBuf) {
    CliService::section("Setup Complete!");

    CliService::info(&format!(
        "Created profile: {} → {}",
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
