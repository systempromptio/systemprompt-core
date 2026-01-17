use crate::shared::CommandResult;
use anyhow::{Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use std::path::{Path, PathBuf};
use systemprompt_core_logging::CliService;

use super::postgres::PostgresConfig;
use super::types::{DatabaseSetupInfo, SecretsConfiguredInfo, SetupOutput};
use super::{postgres, profile, secrets, SetupArgs};
use crate::CliConfig;

pub async fn execute(args: SetupArgs, config: &CliConfig) -> Result<CommandResult<SetupOutput>> {
    if !config.is_json_output() {
        CliService::section("SystemPrompt Setup Wizard");
    }

    let project_root = detect_project_root()?;
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("systemprompt");

    if !config.is_json_output() {
        CliService::success(&format!(
            "Project: {} ({})",
            project_name,
            project_root.display()
        ));
    }

    let systemprompt_dir = project_root.join(".systemprompt");

    let env_name = get_environment_name(&args, config)?;

    if !config.is_json_output() {
        CliService::info(&format!("Configuring environment: {}", env_name));
    }

    if !args.dry_run && !args.yes && config.is_interactive() {
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "This will create/update configuration for '{}' environment. Continue?",
                env_name
            ))
            .default(true)
            .interact()?;

        if !confirmed {
            let output = SetupOutput {
                environment: env_name.clone(),
                profile_path: String::new(),
                database: DatabaseSetupInfo {
                    host: args.db_host.clone(),
                    port: args.db_port,
                    name: args.effective_db_name(&env_name),
                    user: args.effective_db_user(&env_name),
                    connection_status: "cancelled".to_string(),
                    docker: args.docker,
                },
                secrets_configured: SecretsConfiguredInfo {
                    anthropic: args.anthropic_key.is_some(),
                    openai: args.openai_key.is_some(),
                    gemini: args.gemini_key.is_some(),
                    github: args.github_token.is_some(),
                },
                migrations_run: false,
                message: "Setup cancelled by user".to_string(),
            };

            if !config.is_json_output() {
                CliService::info("Setup cancelled");
            }

            let result = CommandResult::text(output).with_title("Setup Cancelled");
            if config.is_json_output() {
                return Ok(result);
            }
            return Ok(result.with_skip_render());
        }
    }

    if args.dry_run {
        return Ok(execute_dry_run(&args, &env_name, &systemprompt_dir, config));
    }

    if !config.is_json_output() {
        CliService::section(&format!("Setting up '{}' environment", env_name));
    }

    let pg_config = setup_postgres(&args, config, &env_name).await?;

    let connection_status = if postgres::test_connection(&pg_config).await {
        "connected"
    } else {
        "unreachable"
    };

    let mut secrets_data = collect_secrets(&args, config, &env_name)?;
    secrets_data.database_url = Some(pg_config.database_url());

    let secrets_path = secrets::default_path(&systemprompt_dir, &env_name);
    secrets::save(&secrets_data, &secrets_path)?;

    let relative_secrets_path = format!("../secrets/{}.secrets.json", env_name);
    let profile_data = profile::build(&env_name, &relative_secrets_path, &project_root)?;
    let profile_path = profile::default_path(&systemprompt_dir, &env_name);
    profile::save(&profile_data, &profile_path)?;

    match profile_data.validate() {
        Ok(()) => {
            if !config.is_json_output() {
                CliService::success("Profile validated successfully");
            }
        },
        Err(e) => {
            if !config.is_json_output() {
                CliService::warning(&format!("Profile validation warnings: {}", e));
            }
        },
    }

    let run_migrations = should_run_migrations(&args, config)?;
    if run_migrations {
        profile::run_migrations(&profile_path)?;
    }

    let output = SetupOutput {
        environment: env_name.clone(),
        profile_path: profile_path.to_string_lossy().to_string(),
        database: DatabaseSetupInfo {
            host: pg_config.host.clone(),
            port: pg_config.port,
            name: pg_config.database.clone(),
            user: pg_config.user.clone(),
            connection_status: connection_status.to_string(),
            docker: args.docker,
        },
        secrets_configured: SecretsConfiguredInfo {
            anthropic: secrets_data.anthropic.is_some(),
            openai: secrets_data.openai.is_some(),
            gemini: secrets_data.gemini.is_some(),
            github: secrets_data.github.is_some(),
        },
        migrations_run: run_migrations,
        message: format!("Environment '{}' setup completed successfully", env_name),
    };

    if !config.is_json_output() {
        print_summary(&env_name, &profile_path);
    }

    let result = CommandResult::text(output).with_title("Setup Complete");
    if config.is_json_output() {
        Ok(result)
    } else {
        Ok(result.with_skip_render())
    }
}

fn execute_dry_run(
    args: &SetupArgs,
    env_name: &str,
    systemprompt_dir: &Path,
    config: &CliConfig,
) -> CommandResult<SetupOutput> {
    if !config.is_json_output() {
        CliService::section("Dry Run - No changes will be made");
    }

    let profile_path = profile::default_path(systemprompt_dir, env_name);
    let secrets_path = secrets::default_path(systemprompt_dir, env_name);

    let connection_status = if args.docker {
        "docker_pending"
    } else if postgres::detect_postgresql(&args.db_host, args.db_port) {
        "reachable"
    } else {
        "unreachable"
    };

    if !config.is_json_output() {
        CliService::subsection("Configuration Preview");
        CliService::key_value("Environment", env_name);
        CliService::key_value("Profile path", &profile_path.to_string_lossy());
        CliService::key_value("Secrets path", &secrets_path.to_string_lossy());

        CliService::subsection("Database");
        CliService::key_value("Host", &args.db_host);
        CliService::key_value("Port", &args.db_port.to_string());
        CliService::key_value("User", &args.effective_db_user(env_name));
        CliService::key_value("Database", &args.effective_db_name(env_name));
        CliService::key_value("Docker", if args.docker { "yes" } else { "no" });
        CliService::key_value("Connection", connection_status);

        CliService::subsection("API Keys");
        CliService::key_value(
            "Anthropic",
            if args.anthropic_key.is_some() {
                "configured"
            } else {
                "not set"
            },
        );
        CliService::key_value(
            "OpenAI",
            if args.openai_key.is_some() {
                "configured"
            } else {
                "not set"
            },
        );
        CliService::key_value(
            "Gemini",
            if args.gemini_key.is_some() {
                "configured"
            } else {
                "not set"
            },
        );
        CliService::key_value(
            "GitHub",
            if args.github_token.is_some() {
                "configured"
            } else {
                "not set"
            },
        );

        CliService::subsection("Migrations");
        let migration_status = if args.migrate {
            "will run"
        } else if args.no_migrate {
            "skipped"
        } else {
            "will prompt (interactive)"
        };
        CliService::key_value("Status", migration_status);

        CliService::info("");
        CliService::info("Run without --dry-run to execute setup");
    }

    let output = SetupOutput {
        environment: env_name.to_string(),
        profile_path: profile_path.to_string_lossy().to_string(),
        database: DatabaseSetupInfo {
            host: args.db_host.clone(),
            port: args.db_port,
            name: args.effective_db_name(env_name),
            user: args.effective_db_user(env_name),
            connection_status: connection_status.to_string(),
            docker: args.docker,
        },
        secrets_configured: SecretsConfiguredInfo {
            anthropic: args.anthropic_key.is_some(),
            openai: args.openai_key.is_some(),
            gemini: args.gemini_key.is_some(),
            github: args.github_token.is_some(),
        },
        migrations_run: false,
        message: "Dry run completed - no changes made".to_string(),
    };

    let result = CommandResult::text(output).with_title("Setup Dry Run");
    if config.is_json_output() {
        result
    } else {
        result.with_skip_render()
    }
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
        return postgres::setup_non_interactive(args, env_name, config).await;
    }
    postgres::setup_interactive(args, env_name, config).await
}

fn collect_secrets(
    args: &SetupArgs,
    config: &CliConfig,
    env_name: &str,
) -> Result<secrets::SecretsData> {
    if !config.is_interactive() {
        return secrets::collect_non_interactive(args, config);
    }
    secrets::collect_interactive(args, env_name, config)
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

fn print_summary(env_name: &str, profile_path: &Path) {
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
