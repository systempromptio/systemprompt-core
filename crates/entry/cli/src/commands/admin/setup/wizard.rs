//! Top-level orchestration of the setup wizard.
//!
//! `execute` runs the end-to-end flow: detect the project root, resolve the
//! environment, provision `PostgreSQL`, collect secrets, write the profile, and
//! optionally run migrations, returning a [`SetupOutput`]. The dry-run and
//! cancellation paths short-circuit without touching the filesystem.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::interactive::Prompter;
use crate::shared::CommandOutput;
use anyhow::Result;
use systemprompt_logging::CliService;

use super::common::PostgresConfig;
use super::types::{DatabaseSetupInfo, SecretsConfiguredInfo, SetupOutput};
use super::wizard_dry_run::execute_dry_run;
use super::wizard_prompts::{
    collect_secrets, detect_project_root, get_environment_name, print_summary, setup_postgres,
    should_run_migrations,
};
use super::{SetupArgs, ai_config, common, profile, secrets};
use crate::CliConfig;

pub fn should_write(path: &Path, force: bool, config: &CliConfig) -> bool {
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
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    if !config.is_json_output() {
        CliService::section("systemprompt.io Setup Wizard");
    }

    let project_root = detect_project_root()?;
    announce_project(&project_root, config);

    let env_name = get_environment_name(&args, prompter, config)?;

    if !config.is_json_output() {
        CliService::info(&format!("Configuring environment: {}", env_name));
    }

    if !confirm_setup(&args, prompter, &env_name, config)? {
        return Ok(build_cancelled(&args, &env_name, config));
    }

    if args.dry_run {
        let systemprompt_dir = project_root.join(".systemprompt");
        return Ok(execute_dry_run(&args, &env_name, &systemprompt_dir, config));
    }

    if !config.is_json_output() {
        CliService::section(&format!("Setting up '{}' environment", env_name));
    }

    let pg_config = setup_postgres(&args, prompter, config, &env_name).await?;

    let connection_status = if common::test_connection(&pg_config).await {
        "connected"
    } else {
        "unreachable"
    };

    let (secrets_data, profile_path) = write_configuration(
        &args,
        prompter,
        config,
        &env_name,
        &project_root,
        &pg_config,
    )?;

    let run_migrations = should_run_migrations(&args, prompter, config)?;
    if run_migrations {
        profile::run_migrations(&profile_path)?;
    }

    let output = SetupOutput {
        environment: env_name.clone(),
        profile_path: profile_path.to_string_lossy().to_string(),
        database: database_info(&pg_config, connection_status, args.docker),
        secrets_configured: secrets_info(&secrets_data),
        migrations_run: run_migrations,
        message: format!("Environment '{}' setup completed successfully", env_name),
    };

    if !config.is_json_output() {
        print_summary(&env_name, &profile_path);
    }

    let result = CommandOutput::card_value("Setup Complete", &output);
    if config.is_json_output() {
        Ok(result)
    } else {
        Ok(result.with_skip_render())
    }
}

fn announce_project(project_root: &Path, config: &CliConfig) {
    if config.is_json_output() {
        return;
    }
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("systemprompt");
    CliService::success(&format!(
        "Project: {} ({})",
        project_name,
        project_root.display()
    ));
}

fn confirm_setup(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    env_name: &str,
    config: &CliConfig,
) -> Result<bool> {
    if args.dry_run || args.yes || !config.is_interactive() {
        return Ok(true);
    }
    prompter.confirm(
        &format!(
            "This will create/update configuration for '{}' environment. Continue?",
            env_name
        ),
        true,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "wizard step threads discrete, already-validated setup inputs"
)]
fn write_configuration(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
    env_name: &str,
    project_root: &Path,
    pg_config: &PostgresConfig,
) -> Result<(secrets::SecretsData, PathBuf)> {
    let systemprompt_dir = project_root.join(".systemprompt");

    let (mut secrets_data, primary_provider) = collect_secrets(args, prompter, config, env_name)?;
    secrets_data.database_url = Some(pg_config.database_url());

    let secrets_path = profile::profile_dir(&systemprompt_dir, env_name).join("secrets.json");
    if should_write(&secrets_path, args.force, config) {
        secrets::save(&secrets_data, &secrets_path)?;
    }

    let profile_data = profile::build(&profile::ProfileBuildParams {
        env_name,
        secrets_path: "secrets.json",
        project_root,
        bin_path: None,
        secrets: &secrets_data,
        default_provider: primary_provider.as_ref(),
    })?;
    let profile_path = profile::default_path(&systemprompt_dir, env_name);
    if should_write(&profile_path, args.force, config) {
        profile::save(&profile_data, &profile_path)?;
    }

    if let Some(primary) = primary_provider.as_ref() {
        ai_config::reconcile(project_root, primary, &secrets_data, config)?;
    }

    Ok((secrets_data, profile_path))
}

pub fn database_info(
    pg_config: &PostgresConfig,
    connection_status: &str,
    docker: bool,
) -> DatabaseSetupInfo {
    DatabaseSetupInfo {
        host: pg_config.host.clone(),
        port: pg_config.port,
        name: pg_config.database.clone(),
        user: pg_config.user.clone(),
        connection_status: connection_status.to_owned(),
        docker,
    }
}

const fn secrets_info(secrets_data: &secrets::SecretsData) -> SecretsConfiguredInfo {
    SecretsConfiguredInfo {
        anthropic: secrets_data.anthropic.is_some(),
        openai: secrets_data.openai.is_some(),
        gemini: secrets_data.gemini.is_some(),
        github: secrets_data.github.is_some(),
    }
}

pub fn build_cancelled(args: &SetupArgs, env_name: &str, config: &CliConfig) -> CommandOutput {
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

    let result = CommandOutput::card_value("Setup Cancelled", &output);
    if config.is_json_output() {
        result
    } else {
        result.with_skip_render()
    }
}
