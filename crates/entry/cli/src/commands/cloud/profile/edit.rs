//! `cloud profile edit` command applying flag-driven profile updates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::path::Path;
use systemprompt_cloud::ProfilePath;
use systemprompt_config::write_private_atomic;
use systemprompt_logging::CliService;

use super::EditArgs;
use serde_yaml::Value;

use super::edit_document::ProfileEdit;
use super::edit_secrets::edit_api_keys;
use super::edit_settings::{edit_runtime_settings, edit_security_settings, edit_server_settings};
use crate::context::CommandContext;
use crate::shared::resolve_profile_path;

pub(super) fn execute(args: &EditArgs, ctx: &CommandContext) -> Result<()> {
    let profile_path =
        resolve_profile_path(args.name.as_deref(), ctx.env.profile.as_deref(), None)?.path;
    let profile_dir = profile_path
        .parent()
        .context("Invalid profile path")?
        .to_path_buf();

    if args.has_updates() {
        return apply_updates(args, &profile_path, &profile_dir);
    }

    if !ctx.cli.is_interactive() {
        return Err(anyhow::anyhow!(
            "Profile edit requires --set-* flags in non-interactive mode.\nAvailable flags:\n  \
             --set-anthropic-key <KEY>\n  --set-openai-key <KEY>\n  --set-gemini-key <KEY>\n  \
             --set-github-token <TOKEN>\n  --set-database-url <URL>\n  --set-external-url <URL>\n  \
             --set-host <HOST>\n  --set-port <PORT>"
        ));
    }

    CliService::section(&format!("Edit Profile: {}", profile_path.display()));

    let mut edit = ProfileEdit::open(&profile_path)?;
    let prompter = ctx.prompter();

    let edit_options: Vec<String> = [
        "Server settings (host, port, URLs)",
        "Security settings (JWT)",
        "Runtime settings (environment, log level)",
        "API keys (secrets.json)",
        "Done - save and exit",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    loop {
        let selection = prompter.select("What would you like to edit?", &edit_options)?;

        match selection {
            0 => edit_server_settings(prompter, &mut edit)?,
            1 => edit_security_settings(prompter, &mut edit)?,
            2 => edit_runtime_settings(prompter, &mut edit)?,
            3 => edit_api_keys(prompter, &profile_dir)?,
            4 => break,
            other => return Err(anyhow::anyhow!("unexpected menu selection: {other}")),
        }
    }

    edit.save()?;
    CliService::success(&format!("Profile saved: {}", profile_path.display()));

    Ok(())
}

fn apply_updates(args: &EditArgs, profile_path: &Path, profile_dir: &Path) -> Result<()> {
    CliService::section(&format!("Updating Profile: {}", profile_path.display()));

    let mut edit = ProfileEdit::open(profile_path)?;
    let mut secrets_changed = false;

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);
    let mut secrets: serde_json::Value = if secrets_path.exists() {
        let content = std::fs::read_to_string(&secrets_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    if let Some(key) = &args.set_anthropic_key {
        secrets["anthropic"] = serde_json::Value::String(key.clone());
        secrets_changed = true;
        CliService::success("Updated: anthropic key");
    }

    if let Some(key) = &args.set_openai_key {
        secrets["openai"] = serde_json::Value::String(key.clone());
        secrets_changed = true;
        CliService::success("Updated: openai key");
    }

    if let Some(key) = &args.set_gemini_key {
        secrets["gemini"] = serde_json::Value::String(key.clone());
        secrets_changed = true;
        CliService::success("Updated: gemini key");
    }

    if let Some(token) = &args.set_github_token {
        secrets["github"] = serde_json::Value::String(token.clone());
        secrets_changed = true;
        CliService::success("Updated: github token");
    }

    if let Some(url) = &args.set_database_url {
        secrets["database_url"] = serde_json::Value::String(url.clone());
        secrets_changed = true;
        CliService::success("Updated: database_url");
    }

    if let Some(url) = &args.set_external_url {
        edit.set("server.external_url", url, Value::String(url.clone()))?;
        CliService::success(&format!("Updated: external_url = {}", url));
    }

    if let Some(host) = &args.set_host {
        edit.set("server.host", host, Value::String(host.clone()))?;
        CliService::success(&format!("Updated: host = {}", host));
    }

    if let Some(port) = &args.set_port {
        edit.set(
            "server.port",
            &port.to_string(),
            Value::Number((*port).into()),
        )?;
        CliService::success(&format!("Updated: port = {}", port));
    }

    if secrets_changed {
        let content = CliService::json_string(&secrets)?;
        write_private_atomic(&secrets_path, content.as_bytes())
            .with_context(|| format!("Failed to write {}", secrets_path.display()))?;
    }

    edit.save()?;

    CliService::success(&format!("Profile saved: {}", profile_path.display()));
    Ok(())
}
