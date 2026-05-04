use crate::CliConfig;
use crate::shared::CommandResult;
use std::path::Path;
use systemprompt_logging::CliService;

use super::types::{DatabaseSetupInfo, SecretsConfiguredInfo, SetupOutput};
use super::{SetupArgs, common, profile, secrets};

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
    } else if common::detect_postgresql(&args.db_host, args.db_port) {
        "reachable"
    } else {
        "unreachable"
    };

    if !config.is_json_output() {
        render_preview(
            args,
            env_name,
            &profile_path,
            &secrets_path,
            connection_status,
        );
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

fn render_preview(
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
    CliService::key_value("Anthropic", configured_label(args.anthropic_key.is_some()));
    CliService::key_value("OpenAI", configured_label(args.openai_key.is_some()));
    CliService::key_value("Gemini", configured_label(args.gemini_key.is_some()));
    CliService::key_value("GitHub", configured_label(args.github_token.is_some()));

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

const fn configured_label(present: bool) -> &'static str {
    if present { "configured" } else { "not set" }
}
