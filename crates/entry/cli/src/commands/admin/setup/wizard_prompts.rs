//! Interactive prompts for the setup wizard.
//!
//! The administrator email has no default. It is written into
//! `system_admin.email`, from there onto the `users` row that
//! `admin bootstrap` creates, and it is displayed as the operator's identity —
//! including on the bridge device-link consent screen, directly above a control
//! that mints a durable personal access token. A default of
//! `admin@localhost.localdomain` made that screen name a mailbox nobody owns,
//! which is precisely the recognition the screen exists to invite. Interactive
//! runs prompt for it; non-interactive runs fail with the flag to pass.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::CliConfig;
use crate::interactive::Prompter;
use anyhow::{Context, Result};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::{Email, ProviderId};
use systemprompt_logging::CliService;

use super::common::PostgresConfig;
use super::{SetupArgs, postgres, secrets};

#[doc(hidden)]
pub fn get_environment_name(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<String> {
    if let Some(ref env) = args.environment {
        return Ok(env.clone());
    }

    if !config.is_interactive() {
        return Ok("dev".to_owned());
    }

    CliService::info("Enter environment name (e.g., 'dev', 'staging', 'prod')");
    CliService::info("Press Enter for default: dev");

    let input = prompter.input_with_default("Environment name", "dev")?;

    Ok(input.trim().to_lowercase())
}

#[doc(hidden)]
pub fn resolve_admin_email(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<Email> {
    if let Some(ref email) = args.admin_email {
        return parse_admin_email(email);
    }

    if !config.is_interactive() {
        anyhow::bail!(
            "An administrator email is required. Pass --admin-email <email>.\n\nIt identifies \
             the platform admin on sign-in and consent screens, so it must be an address you \
             actually control."
        );
    }

    CliService::info("Enter the administrator's email address");
    CliService::info("Used to identify you on sign-in and consent screens — no default.");

    let input = prompter.input("Administrator email")?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        anyhow::bail!("An administrator email is required; none was entered.");
    }

    parse_admin_email(trimmed)
}

fn parse_admin_email(raw: &str) -> Result<Email> {
    Email::try_new(raw.trim()).context("--admin-email is not a valid email address")
}

pub(super) async fn setup_postgres(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
    env_name: &str,
) -> Result<PostgresConfig> {
    if args.yes || !config.is_interactive() {
        return postgres::setup_non_interactive(args, env_name, config).await;
    }
    postgres::setup_interactive(args, prompter, env_name, config).await
}

pub(super) fn collect_secrets(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
    env_name: &str,
) -> Result<(secrets::SecretsData, Option<ProviderId>)> {
    if args.has_ai_provider() {
        return secrets::collect_non_interactive(args, config);
    }
    if std::io::stdin().is_terminal() {
        return secrets::collect_interactive(args, prompter, env_name, config);
    }
    secrets::collect_non_interactive(args, config)
}

#[doc(hidden)]
pub fn should_run_migrations(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<bool> {
    if args.migrate {
        return Ok(true);
    }
    if args.no_migrate {
        return Ok(false);
    }
    if !config.is_interactive() {
        return Ok(false);
    }

    prompter.confirm("Run database migrations now?", true)
}

#[doc(hidden)]
pub fn detect_project_root() -> Result<PathBuf> {
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
    CliService::info("   systemprompt cloud auth login");
    CliService::info("   systemprompt cloud config");
}
