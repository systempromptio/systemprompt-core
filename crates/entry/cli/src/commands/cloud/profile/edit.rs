use anyhow::{bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::path::{Path, PathBuf};
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_loader::ProfileLoader;
use systemprompt_logging::CliService;

use super::edit_secrets::edit_api_keys;
use super::edit_settings::{edit_runtime_settings, edit_security_settings, edit_server_settings};
use super::templates::save_profile;
use super::EditArgs;
use crate::cli_settings::CliConfig;

pub async fn execute(args: &EditArgs, config: &CliConfig) -> Result<()> {
    let profile_path = resolve_profile_path(args.name.as_deref(), config)?;
    let profile_dir = profile_path
        .parent()
        .context("Invalid profile path")?
        .to_path_buf();

    if args.has_updates() {
        return apply_updates(args, &profile_path, &profile_dir).await;
    }

    if !config.is_interactive() {
        return Err(anyhow::anyhow!(
            "Profile edit requires --set-* flags in non-interactive mode.\nAvailable flags:\n  \
             --set-anthropic-key <KEY>\n  --set-openai-key <KEY>\n  --set-gemini-key <KEY>\n  \
             --set-github-token <TOKEN>\n  --set-database-url <URL>\n  --set-external-url <URL>\n  \
             --set-host <HOST>\n  --set-port <PORT>"
        ));
    }

    CliService::section(&format!("Edit Profile: {}", profile_path.display()));

    let mut profile = ProfileLoader::load_from_path(&profile_path)?;

    let edit_options = vec![
        "Server settings (host, port, URLs)",
        "Security settings (JWT)",
        "Runtime settings (environment, log level)",
        "API keys (secrets.json)",
        "Done - save and exit",
    ];

    loop {
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("What would you like to edit?")
            .items(&edit_options)
            .default(0)
            .interact()?;

        match selection {
            0 => edit_server_settings(&mut profile)?,
            1 => edit_security_settings(&mut profile)?,
            2 => edit_runtime_settings(&mut profile)?,
            3 => edit_api_keys(&profile_dir).await?,
            4 => break,
            _ => unreachable!(),
        }
    }

    save_profile(&profile, &profile_path)?;
    CliService::success(&format!("Profile saved: {}", profile_path.display()));

    Ok(())
}

async fn apply_updates(args: &EditArgs, profile_path: &Path, profile_dir: &Path) -> Result<()> {
    CliService::section(&format!("Updating Profile: {}", profile_path.display()));

    let mut profile = ProfileLoader::load_from_path(profile_path)?;
    let mut profile_changed = false;
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
        profile.server.api_external_url = url.clone();
        profile_changed = true;
        CliService::success(&format!("Updated: external_url = {}", url));
    }

    if let Some(host) = &args.set_host {
        profile.server.host = host.clone();
        profile_changed = true;
        CliService::success(&format!("Updated: host = {}", host));
    }

    if let Some(port) = &args.set_port {
        profile.server.port = *port;
        profile_changed = true;
        CliService::success(&format!("Updated: port = {}", port));
    }

    if secrets_changed {
        let content = serde_json::to_string_pretty(&secrets)?;
        std::fs::write(&secrets_path, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&secrets_path, std::fs::Permissions::from_mode(0o600))?;
        }
    }

    if profile_changed {
        save_profile(&profile, profile_path)?;
    }

    CliService::success(&format!("Profile saved: {}", profile_path.display()));
    Ok(())
}

fn resolve_profile_path(name: Option<&str>, config: &CliConfig) -> Result<PathBuf> {
    if let Some(profile_name) = name {
        let ctx = ProjectContext::discover();
        let profile_path = ctx.profile_path(profile_name, ProfilePath::Config);

        if !profile_path.exists() {
            bail!(
                "Profile '{}' not found at {}",
                profile_name,
                profile_path.display()
            );
        }

        return Ok(profile_path);
    }

    if let Ok(path) = std::env::var("SYSTEMPROMPT_PROFILE") {
        let profile_path = PathBuf::from(&path);
        if profile_path.exists() {
            return Ok(profile_path);
        }
        bail!("Profile from SYSTEMPROMPT_PROFILE not found: {}", path);
    }

    if !config.is_interactive() {
        bail!(
            "Profile name required in non-interactive mode.\nProvide profile name as argument or \
             set SYSTEMPROMPT_PROFILE environment variable."
        );
    }

    select_profile_interactively()
}

fn select_profile_interactively() -> Result<PathBuf> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        bail!("No profiles directory found. Run 'systemprompt cloud profile create' first.");
    }

    let profiles: Vec<String> = std::fs::read_dir(&profiles_dir)?
        .filter_map(|e| {
            e.map_err(|err| {
                tracing::debug!(error = %err, "Failed to read profile directory entry");
                err
            })
            .ok()
        })
        .filter(|e| e.path().is_dir() && ProfilePath::Config.resolve(&e.path()).exists())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect();

    if profiles.is_empty() {
        bail!("No profiles found. Run 'systemprompt cloud profile create' first.");
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select profile to edit")
        .items(&profiles)
        .default(0)
        .interact()?;

    let selected = &profiles[selection];
    Ok(ctx.profile_path(selected, ProfilePath::Config))
}
