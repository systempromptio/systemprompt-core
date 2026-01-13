use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Select};
use systemprompt_core_logging::CliService;
use systemprompt_models::{Environment, LogLevel, Profile};

pub fn edit_server_settings(profile: &mut Profile) -> Result<()> {
    CliService::section("Server Settings");

    profile.server.host = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Host")
        .default(profile.server.host.clone())
        .interact_text()?;

    profile.server.port = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Port")
        .default(profile.server.port)
        .interact_text()?;

    profile.server.api_server_url = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("API Server URL")
        .default(profile.server.api_server_url.clone())
        .interact_text()?;

    profile.server.api_external_url = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("API External URL")
        .default(profile.server.api_external_url.clone())
        .interact_text()?;

    profile.server.use_https = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Use HTTPS?")
        .default(profile.server.use_https)
        .interact()?;

    CliService::success("Server settings updated");
    Ok(())
}

pub fn edit_security_settings(profile: &mut Profile) -> Result<()> {
    CliService::section("Security Settings");

    profile.security.issuer = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("JWT Issuer")
        .default(profile.security.issuer.clone())
        .interact_text()?;

    profile.security.access_token_expiration = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Access Token Expiration (seconds)")
        .default(profile.security.access_token_expiration)
        .interact_text()?;

    profile.security.refresh_token_expiration = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Refresh Token Expiration (seconds)")
        .default(profile.security.refresh_token_expiration)
        .interact_text()?;

    CliService::success("Security settings updated");
    Ok(())
}

pub fn edit_runtime_settings(profile: &mut Profile) -> Result<()> {
    CliService::section("Runtime Settings");

    let env_options = vec!["development", "test", "staging", "production"];
    let current_env_idx = env_options
        .iter()
        .position(|&e| e == profile.runtime.environment.to_string())
        .unwrap_or(0);

    let env_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Environment")
        .items(&env_options)
        .default(current_env_idx)
        .interact()?;

    profile.runtime.environment = env_options[env_selection]
        .parse::<Environment>()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let log_options = vec!["quiet", "normal", "verbose", "debug"];
    let current_log_idx = log_options
        .iter()
        .position(|&l| l == profile.runtime.log_level.to_string())
        .unwrap_or(1);

    let log_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Log Level")
        .items(&log_options)
        .default(current_log_idx)
        .interact()?;

    profile.runtime.log_level = log_options[log_selection]
        .parse::<LogLevel>()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    CliService::success("Runtime settings updated");
    Ok(())
}
