//! `cloud profile edit-settings` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_logging::CliService;
use systemprompt_models::{Environment, LogLevel, Profile};

use crate::interactive::Prompter;

pub fn edit_server_settings(prompter: &dyn Prompter, profile: &mut Profile) -> Result<()> {
    CliService::section("Server Settings");

    profile.server.host = prompter.input_with_default("Host", &profile.server.host)?;

    profile.server.port = prompter
        .input_with_default("Port", &profile.server.port.to_string())?
        .trim()
        .parse()
        .context("Invalid port")?;

    profile.server.api_server_url =
        prompter.input_with_default("API Server URL", &profile.server.api_server_url)?;

    profile.server.api_external_url =
        prompter.input_with_default("API External URL", &profile.server.api_external_url)?;

    profile.server.use_https = prompter.confirm("Use HTTPS?", profile.server.use_https)?;

    CliService::success("Server settings updated");
    Ok(())
}

pub fn edit_security_settings(prompter: &dyn Prompter, profile: &mut Profile) -> Result<()> {
    CliService::section("Security Settings");

    profile.security.issuer =
        prompter.input_with_default("JWT Issuer", &profile.security.issuer)?;

    profile.security.access_token_expiration = prompter
        .input_with_default(
            "Access Token Expiration (seconds)",
            &profile.security.access_token_expiration.to_string(),
        )?
        .trim()
        .parse()
        .context("Invalid access token expiration")?;

    profile.security.refresh_token_expiration = prompter
        .input_with_default(
            "Refresh Token Expiration (seconds)",
            &profile.security.refresh_token_expiration.to_string(),
        )?
        .trim()
        .parse()
        .context("Invalid refresh token expiration")?;

    CliService::success("Security settings updated");
    Ok(())
}

pub fn edit_runtime_settings(prompter: &dyn Prompter, profile: &mut Profile) -> Result<()> {
    CliService::section("Runtime Settings");

    let env_options: Vec<String> = ["development", "test", "staging", "production"]
        .into_iter()
        .map(str::to_owned)
        .collect();

    let env_selection = prompter.select("Environment", &env_options)?;

    profile.runtime.environment = env_options[env_selection]
        .parse::<Environment>()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let log_options: Vec<String> = ["quiet", "normal", "verbose", "debug"]
        .into_iter()
        .map(str::to_owned)
        .collect();

    let log_selection = prompter.select("Log Level", &log_options)?;

    profile.runtime.log_level = log_options[log_selection]
        .parse::<LogLevel>()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    CliService::success("Runtime settings updated");
    Ok(())
}
