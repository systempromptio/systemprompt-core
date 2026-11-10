//! `cloud profile edit` settings prompts.
//!
//! Each prompt shows the field as written in the profile and records a change
//! only when the answer differs, so a placeholder the operator leaves alone is
//! saved unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use serde_yaml::Value;
use systemprompt_logging::CliService;
use systemprompt_manifest::{Environment, LogLevel};

use super::edit_document::{ProfileEdit, parse_number};
use crate::interactive::Prompter;

pub fn edit_server_settings(prompter: &dyn Prompter, edit: &mut ProfileEdit) -> Result<()> {
    CliService::section("Server Settings");

    let host = prompter.input_with_default("Host", &edit.text("server.host")?)?;
    edit.set("server.host", &host, Value::String(host.clone()))?;

    let port = prompter.input_with_default("Port", &edit.text("server.port")?)?;
    if port != edit.text("server.port")? {
        let parsed: u16 = parse_number(&port, "port")?;
        edit.set("server.port", &port, Value::Number(parsed.into()))?;
    }

    let url = prompter.input_with_default("External URL", &edit.text("server.external_url")?)?;
    edit.set("server.external_url", &url, Value::String(url.clone()))?;

    CliService::success("Server settings updated");
    Ok(())
}

pub fn edit_security_settings(prompter: &dyn Prompter, edit: &mut ProfileEdit) -> Result<()> {
    CliService::section("Security Settings");

    for (key, label, what) in [
        (
            "security.jwt_access_token_expiration",
            "Access Token Expiration (seconds)",
            "access token expiration",
        ),
        (
            "security.jwt_refresh_token_expiration",
            "Refresh Token Expiration (seconds)",
            "refresh token expiration",
        ),
    ] {
        let current = edit.text(key)?;
        let answer = prompter.input_with_default(label, &current)?;
        if answer != current {
            let parsed: i64 = parse_number(&answer, what)?;
            edit.set(key, &answer, Value::Number(parsed.into()))?;
        }
    }

    CliService::success("Security settings updated");
    Ok(())
}

pub fn edit_runtime_settings(prompter: &dyn Prompter, edit: &mut ProfileEdit) -> Result<()> {
    CliService::section("Runtime Settings");

    let env_options: Vec<String> = ["development", "test", "staging", "production"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let environment = &env_options[prompter.select("Environment", &env_options)?];
    let parsed = environment
        .parse::<Environment>()
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    edit.set(
        "runtime.environment",
        environment,
        serde_yaml::to_value(parsed)?,
    )?;

    let log_options: Vec<String> = ["quiet", "normal", "verbose", "debug"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let log_level = &log_options[prompter.select("Log Level", &log_options)?];
    let parsed = log_level
        .parse::<LogLevel>()
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    edit.set(
        "runtime.log_level",
        log_level,
        serde_yaml::to_value(parsed)?,
    )?;

    CliService::success("Runtime settings updated");
    Ok(())
}
