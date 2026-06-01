//! Top-level orchestration of the setup wizard.
//!
//! [`execute`] runs the end-to-end flow: detect the project root, resolve the
//! environment, provision `PostgreSQL`, collect secrets, write the profile, and
//! optionally run migrations, returning a [`SetupOutput`]. The dry-run and
//! cancellation paths short-circuit without touching the filesystem.

use crate::shared::CommandResult;
use anyhow::Result;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use systemprompt_logging::CliService;

use super::types::{DatabaseSetupInfo, SecretsConfiguredInfo, SetupOutput};
use super::wizard_dry_run::execute_dry_run;
use super::wizard_prompts::{
    collect_secrets, detect_project_root, get_environment_name, print_summary, setup_postgres,
    should_run_migrations,
};
use super::{SetupArgs, catalog, common, profile, secrets};
use crate::CliConfig;

fn should_write(path: &std::path::Path, force: bool, config: &CliConfig) -> bool {
    if force || !path.exists() {
        return true;
    }
    if !config.is_json_output() {
        CliService::info(&format!(
            "Preserving existing {} (pass --force to overwrite)",
            path.display()
        ));
    }
    false
}

pub(super) async fn execute(
    args: SetupArgs,
    config: &CliConfig,
) -> Result<CommandResult<SetupOutput>> {
    if !config.is_json_output() {
        CliService::section("systemprompt.io Setup Wizard");
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
            return Ok(build_cancelled(&args, &env_name, config));
        }
    }

    if args.dry_run {
        return Ok(execute_dry_run(&args, &env_name, &systemprompt_dir, config));
    }

    if !config.is_json_output() {
        CliService::section(&format!("Setting up '{}' environment", env_name));
    }

    let pg_config = setup_postgres(&args, config, &env_name).await?;

    let connection_status = if common::test_connection(&pg_config).await {
        "connected"
    } else {
        "unreachable"
    };

    let mut secrets_data = collect_secrets(&args, config, &env_name)?;
    secrets_data.database_url = Some(pg_config.database_url());

    let secrets_path = profile::profile_dir(&systemprompt_dir, &env_name).join("secrets.json");
    if should_write(&secrets_path, args.force, config) {
        secrets::save(&secrets_data, &secrets_path)?;
    }

    let profile_data = profile::build(
        &env_name,
        "secrets.json",
        &project_root,
        None,
        &secrets_data,
    )?;
    let profile_path = profile::default_path(&systemprompt_dir, &env_name);
    if should_write(&profile_path, args.force, config) {
        profile::save(&profile_data, &profile_path)?;
    }

    let catalog_path = profile::catalog_path(&systemprompt_dir, &env_name);
    if should_write(&catalog_path, args.force, config) {
        catalog::save_catalog(&catalog::build_catalog(&secrets_data), &catalog_path)?;
        if !config.is_json_output() {
            CliService::success(&format!(
                "Saved gateway catalog to {}",
                catalog_path.display()
            ));
        }
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
            connection_status: connection_status.to_owned(),
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

fn build_cancelled(
    args: &SetupArgs,
    env_name: &str,
    config: &CliConfig,
) -> CommandResult<SetupOutput> {
    let output = SetupOutput {
        environment: env_name.to_owned(),
        profile_path: String::new(),
        database: DatabaseSetupInfo {
            host: args.db_host.clone(),
            port: args.db_port,
            name: args.effective_db_name(env_name),
            user: args.effective_db_user(env_name),
            connection_status: "cancelled".to_owned(),
            docker: args.docker,
        },
        secrets_configured: SecretsConfiguredInfo {
            anthropic: args.anthropic_key.is_some(),
            openai: args.openai_key.is_some(),
            gemini: args.gemini_key.is_some(),
            github: args.github_token.is_some(),
        },
        migrations_run: false,
        message: "Setup cancelled by user".to_owned(),
    };

    if !config.is_json_output() {
        CliService::info("Setup cancelled");
    }

    let result = CommandResult::text(output).with_title("Setup Cancelled");
    if config.is_json_output() {
        result
    } else {
        result.with_skip_render()
    }
}
