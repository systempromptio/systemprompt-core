use anyhow::{bail, Context, Result};
use clap::Subcommand;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_cloud::CloudApiClient;
use systemprompt_core_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::tenant_ops::get_credentials;

#[derive(Subcommand)]
pub enum SecretsCommands {
    #[command(about = "Sync secrets from profile secrets.json to cloud")]
    Sync,

    #[command(about = "Set secrets (KEY=VALUE pairs)")]
    Set {
        #[arg(required = true)]
        key_values: Vec<String>,
    },

    #[command(about = "Remove secrets")]
    Unset {
        #[arg(required = true)]
        keys: Vec<String>,
    },
}

pub async fn execute(cmd: SecretsCommands) -> Result<()> {
    match cmd {
        SecretsCommands::Sync => sync_secrets().await,
        SecretsCommands::Set { key_values } => set_secrets(key_values).await,
        SecretsCommands::Unset { keys } => unset_secrets(keys).await,
    }
}

async fn sync_secrets() -> Result<()> {
    CliService::section("Sync Secrets");

    let (tenant_id, secrets_path) = get_tenant_and_secrets_path()?;
    let secrets = load_secrets_json(&secrets_path)?;

    if secrets.is_empty() {
        CliService::warning("No secrets found in secrets.json");
        return Ok(());
    }

    let env_secrets = map_secrets_to_env_vars(secrets);
    CliService::info(&format!("Found {} secrets to sync", env_secrets.len()));

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let spinner = CliService::spinner("Syncing secrets...");
    match client.set_secrets(&tenant_id, env_secrets).await {
        Ok(keys) => {
            spinner.finish_and_clear();
            CliService::success(&format!("Synced {} secrets", keys.len()));
            for key in &keys {
                CliService::info(&format!("  - {key}"));
            }
        },
        Err(e) => {
            spinner.finish_and_clear();
            bail!("Failed to sync secrets: {e}");
        },
    }

    Ok(())
}

async fn set_secrets(key_values: Vec<String>) -> Result<()> {
    CliService::section("Set Secrets");

    let tenant_id = get_tenant_id()?;
    let mut secrets = HashMap::new();

    for kv in &key_values {
        let parts: Vec<&str> = kv.splitn(2, '=').collect();
        if parts.len() != 2 {
            bail!("Invalid format: {kv}. Expected KEY=VALUE");
        }
        let key = parts[0].to_uppercase();
        let value = parts[1].to_string();
        secrets.insert(key, value);
    }

    if secrets.is_empty() {
        bail!("No secrets provided");
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let spinner = CliService::spinner("Setting secrets...");
    match client.set_secrets(&tenant_id, secrets).await {
        Ok(keys) => {
            spinner.finish_and_clear();
            CliService::success(&format!("Set {} secrets", keys.len()));
            for key in &keys {
                CliService::info(&format!("  - {key}"));
            }
        },
        Err(e) => {
            spinner.finish_and_clear();
            bail!("Failed to set secrets: {e}");
        },
    }

    Ok(())
}

async fn unset_secrets(keys: Vec<String>) -> Result<()> {
    CliService::section("Remove Secrets");

    if keys.is_empty() {
        bail!("No keys provided");
    }

    let tenant_id = get_tenant_id()?;
    let uppercase_keys: Vec<String> = keys.iter().map(|k| k.to_uppercase()).collect();

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let mut removed = Vec::new();
    let mut errors = Vec::new();

    for key in &uppercase_keys {
        let spinner = CliService::spinner(&format!("Removing {key}..."));
        match client.unset_secret(&tenant_id, key).await {
            Ok(()) => {
                spinner.finish_and_clear();
                removed.push(key.clone());
            },
            Err(e) => {
                spinner.finish_and_clear();
                errors.push((key.clone(), e.to_string()));
            },
        }
    }

    if !removed.is_empty() {
        CliService::success(&format!("Removed {} secrets", removed.len()));
        for key in &removed {
            CliService::info(&format!("  - {key}"));
        }
    }

    if !errors.is_empty() {
        for (key, err) in &errors {
            CliService::error(&format!("Failed to remove {key}: {err}"));
        }
        if removed.is_empty() {
            bail!("Failed to remove any secrets");
        }
    }

    Ok(())
}

fn get_tenant_id() -> Result<String> {
    let profile =
        ProfileBootstrap::get().map_err(|_| anyhow::anyhow!("Profile not initialized"))?;

    let cloud = profile
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Cloud not configured in profile"))?;

    cloud
        .tenant_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("No tenant_id in profile. Create a cloud tenant first."))
}

fn get_tenant_and_secrets_path() -> Result<(String, PathBuf)> {
    let tenant_id = get_tenant_id()?;

    let profile_path =
        ProfileBootstrap::get_path().map_err(|_| anyhow::anyhow!("Profile path not available"))?;

    let profile_dir = std::path::Path::new(profile_path)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid profile path"))?;

    let secrets_path = profile_dir.join("secrets.json");

    if !secrets_path.exists() {
        bail!(
            "secrets.json not found at {}. Create it first.",
            secrets_path.display()
        );
    }

    Ok((tenant_id, secrets_path))
}

fn load_secrets_json(path: &PathBuf) -> Result<HashMap<String, String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let json: serde_json::Value =
        serde_json::from_str(&content).with_context(|| "Failed to parse secrets.json")?;

    let mut secrets = HashMap::new();

    if let Some(obj) = json.as_object() {
        for (key, value) in obj {
            if let Some(s) = value.as_str() {
                if !s.is_empty() {
                    secrets.insert(key.clone(), s.to_string());
                }
            }
        }
    }

    Ok(secrets)
}

fn map_secrets_to_env_vars(secrets: HashMap<String, String>) -> HashMap<String, String> {
    secrets
        .into_iter()
        .map(|(k, v)| {
            let env_key = to_env_var_name(&k);
            (env_key, v)
        })
        .collect()
}

fn to_env_var_name(key: &str) -> String {
    match key {
        "gemini" => "GEMINI_API_KEY".to_string(),
        "anthropic" => "ANTHROPIC_API_KEY".to_string(),
        "openai" => "OPENAI_API_KEY".to_string(),
        _ => key.to_uppercase(),
    }
}
