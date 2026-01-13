use anyhow::{Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use std::path::PathBuf;
use systemprompt_core_logging::CliService;

use super::{postgres, profile, secrets};

pub async fn execute(env_arg: Option<String>) -> Result<()> {
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

    let selected_envs = if let Some(env) = env_arg {
        vec![env]
    } else {
        select_environments()?
    };

    if selected_envs.is_empty() {
        CliService::warning("No environments selected. Exiting.");
        return Ok(());
    }

    CliService::info(&format!(
        "Configuring {} environment(s): {}",
        selected_envs.len(),
        selected_envs.join(", ")
    ));

    let mut created_profiles = Vec::new();

    for env_name in &selected_envs {
        CliService::section(&format!("Setting up '{}' environment", env_name));

        let pg_config = postgres::setup_interactive(env_name).await?;

        let mut secrets_data = secrets::collect_interactive(env_name)?;
        secrets_data.database_url = Some(pg_config.database_url());

        let secrets_path = secrets::default_path(&systemprompt_dir, env_name);
        secrets::save(&secrets_data, &secrets_path)?;

        let relative_secrets_path = format!("../secrets/{}.secrets.json", env_name);

        let profile_data = profile::build(env_name, &relative_secrets_path, &project_root)?;
        let profile_path = profile::default_path(&systemprompt_dir, env_name);
        profile::save(&profile_data, &profile_path)?;

        match profile_data.validate() {
            Ok(()) => CliService::success("Profile validated successfully"),
            Err(e) => CliService::warning(&format!("Profile validation warnings: {}", e)),
        }

        created_profiles.push((env_name.clone(), profile_path.clone()));

        let run_migrations = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Run database migrations now?")
            .default(true)
            .interact()?;

        if run_migrations {
            profile::run_migrations(&profile_path)?;
        }
    }

    print_summary(&created_profiles);

    Ok(())
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

fn select_environments() -> Result<Vec<String>> {
    CliService::info("Enter environment names (comma-separated, e.g., 'dev, staging, prod')");
    CliService::info("Press Enter for default: dev");

    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Environment names")
        .default("dev".to_string())
        .interact_text()?;

    let names: Vec<String> = input
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(names)
}

fn print_summary(profiles: &[(String, PathBuf)]) {
    CliService::section("Setup Complete!");

    CliService::info("Created profiles:");
    for (env_name, path) in profiles {
        CliService::info(&format!("  - {} → {}", env_name, path.display()));
    }

    CliService::section("Next Steps");

    if let Some((env_name, profile_path)) = profiles.first() {
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

    if profiles.len() > 1 {
        CliService::info("");
        CliService::info("To switch environments, update SYSTEMPROMPT_PROFILE:");
        for (env_name, profile_path) in profiles.iter().skip(1) {
            CliService::info(&format!(
                "  # For {}:\n  export SYSTEMPROMPT_PROFILE={}",
                env_name,
                profile_path.display()
            ));
        }
    }
}
