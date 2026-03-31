use std::path::Path;

use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use systemprompt_logging::CliService;

use super::types::{DatabaseSetupInfo, SecretsConfiguredInfo, SetupOutput};
use super::{SetupArgs, postgres, profile, secrets};
use crate::CliConfig;
use crate::shared::CommandResult;

pub fn execute_dry_run(
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
        render_dry_run_preview(args, env_name, &profile_path, &secrets_path, connection_status);
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

fn render_dry_run_preview(
    args: &SetupArgs,
    env_name: &str,
    profile_path: &Path,
    secrets_path: &Path,
    connection_status: &str,
) {
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

pub fn get_environment_name(args: &SetupArgs, config: &CliConfig) -> Result<String> {
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

pub fn should_run_migrations(args: &SetupArgs, config: &CliConfig) -> Result<bool> {
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

pub fn print_summary(env_name: &str, profile_path: &Path) {
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
