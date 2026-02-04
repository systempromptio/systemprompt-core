use anyhow::{bail, Context, Result};
use clap::Subcommand;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_cloud::{CloudApiClient, CloudCredentials, ProfilePath};
use systemprompt_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::tenant::get_credentials;
use super::types::SecretsOutput;
use crate::cli_settings::CliConfig;
use crate::shared::{render_result, CommandResult};

#[derive(Debug, Subcommand)]
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

    #[command(about = "Remove incorrectly synced system-managed variables")]
    Cleanup,
}

pub async fn execute(cmd: SecretsCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        SecretsCommands::Sync => {
            let result = sync_secrets(config).await?;
            render_result(&result);
            Ok(())
        },
        SecretsCommands::Set { key_values } => {
            let result = set_secrets(key_values, config).await?;
            render_result(&result);
            Ok(())
        },
        SecretsCommands::Unset { keys } => {
            let result = unset_secrets(keys, config).await?;
            render_result(&result);
            Ok(())
        },
        SecretsCommands::Cleanup => {
            let result = cleanup_secrets(config).await?;
            render_result(&result);
            Ok(())
        },
    }
}

async fn sync_secrets(config: &CliConfig) -> Result<CommandResult<SecretsOutput>> {
    if !config.is_json_output() {
        CliService::section("Sync Secrets");
    }

    let (tenant_id, secrets_path) = get_tenant_and_secrets_path()?;
    let secrets = load_secrets_json(&secrets_path)?;

    if secrets.is_empty() {
        let output = SecretsOutput {
            operation: "sync".to_string(),
            keys: Vec::new(),
            rejected_keys: None,
        };
        if !config.is_json_output() {
            CliService::warning("No secrets found in secrets.json");
        }
        return Ok(CommandResult::list(output).with_title("Sync Secrets"));
    }

    let env_secrets = map_secrets_to_env_vars(secrets);
    if !config.is_json_output() {
        CliService::info(&format!("Found {} secrets to sync", env_secrets.len()));
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let keys = if !config.is_json_output() {
        let spinner = CliService::spinner("Syncing secrets...");
        match client.set_secrets(&tenant_id, env_secrets).await {
            Ok(keys) => {
                spinner.finish_and_clear();
                CliService::success(&format!("Synced {} secrets", keys.len()));
                for key in &keys {
                    CliService::info(&format!("  - {key}"));
                }
                keys
            },
            Err(e) => {
                spinner.finish_and_clear();
                bail!("Failed to sync secrets: {e}");
            },
        }
    } else {
        client.set_secrets(&tenant_id, env_secrets).await?
    };

    let output = SecretsOutput {
        operation: "sync".to_string(),
        keys,
        rejected_keys: None,
    };

    Ok(CommandResult::list(output).with_title("Sync Secrets"))
}

async fn set_secrets(
    key_values: Vec<String>,
    config: &CliConfig,
) -> Result<CommandResult<SecretsOutput>> {
    use systemprompt_cloud::constants::env_vars;

    if !config.is_json_output() {
        CliService::section("Set Secrets");
    }

    let tenant_id = get_tenant_id()?;
    let mut secrets = HashMap::new();
    let mut rejected = Vec::new();

    for kv in &key_values {
        let parts: Vec<&str> = kv.splitn(2, '=').collect();
        if parts.len() != 2 {
            bail!("Invalid format: {kv}. Expected KEY=VALUE");
        }
        let key = parts[0].to_uppercase();
        let value = parts[1].to_string();

        if env_vars::is_system_managed(&key) {
            rejected.push(key);
            continue;
        }
        secrets.insert(key, value);
    }

    if !rejected.is_empty() && !config.is_json_output() {
        for key in &rejected {
            CliService::warning(&format!("Skipping system-managed variable: {key}"));
        }
    }

    if secrets.is_empty() {
        bail!("No valid secrets to set (all provided keys are system-managed)");
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let keys = if !config.is_json_output() {
        let spinner = CliService::spinner("Setting secrets...");
        match client.set_secrets(&tenant_id, secrets).await {
            Ok(keys) => {
                spinner.finish_and_clear();
                CliService::success(&format!("Set {} secrets", keys.len()));
                for key in &keys {
                    CliService::info(&format!("  - {key}"));
                }
                keys
            },
            Err(e) => {
                spinner.finish_and_clear();
                bail!("Failed to set secrets: {e}");
            },
        }
    } else {
        client.set_secrets(&tenant_id, secrets).await?
    };

    let output = SecretsOutput {
        operation: "set".to_string(),
        keys,
        rejected_keys: if rejected.is_empty() {
            None
        } else {
            Some(rejected)
        },
    };

    Ok(CommandResult::list(output).with_title("Set Secrets"))
}

async fn unset_secrets(
    keys: Vec<String>,
    config: &CliConfig,
) -> Result<CommandResult<SecretsOutput>> {
    if !config.is_json_output() {
        CliService::section("Remove Secrets");
    }

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
        if !config.is_json_output() {
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
        } else {
            match client.unset_secret(&tenant_id, key).await {
                Ok(()) => removed.push(key.clone()),
                Err(e) => errors.push((key.clone(), e.to_string())),
            }
        }
    }

    if !config.is_json_output() {
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
    }

    let output = SecretsOutput {
        operation: "unset".to_string(),
        keys: removed,
        rejected_keys: None,
    };

    Ok(CommandResult::list(output).with_title("Remove Secrets"))
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

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);

    if !secrets_path.exists() {
        bail!(
            "secrets.json not found at {}. Create it first.",
            secrets_path.display()
        );
    }

    Ok((tenant_id, secrets_path))
}

pub fn load_secrets_json(path: &PathBuf) -> Result<HashMap<String, String>> {
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

pub fn map_secrets_to_env_vars(secrets: HashMap<String, String>) -> HashMap<String, String> {
    use systemprompt_cloud::constants::env_vars;

    let has_internal = secrets.contains_key("internal_database_url");

    secrets
        .into_iter()
        .filter_map(|(k, v)| {
            let env_key = to_env_var_name(&k, has_internal)?;
            if env_vars::is_system_managed(&env_key) {
                tracing::warn!(key = %env_key, "Skipping system-managed variable from secrets.json");
                return None;
            }
            Some((env_key, v))
        })
        .collect()
}

fn to_env_var_name(key: &str, has_internal_db_url: bool) -> Option<String> {
    match key {
        "gemini" => Some("GEMINI_API_KEY".to_string()),
        "anthropic" => Some("ANTHROPIC_API_KEY".to_string()),
        "openai" => Some("OPENAI_API_KEY".to_string()),
        "internal_database_url" => Some("DATABASE_URL".to_string()),
        "database_url" if has_internal_db_url => None,
        _ => Some(key.to_uppercase()),
    }
}

/// Syncs cloud credentials to the deployment environment.
///
/// This sets the environment variables that allow the CLI to authenticate
/// with the cloud API when running inside a deployed container.
pub async fn sync_cloud_credentials(
    api_client: &CloudApiClient,
    tenant_id: &str,
    creds: &CloudCredentials,
) -> Result<Vec<String>> {
    let mut secrets = HashMap::new();

    secrets.insert(
        "SYSTEMPROMPT_API_TOKEN".to_string(),
        creds.api_token.clone(),
    );

    secrets.insert(
        "SYSTEMPROMPT_USER_EMAIL".to_string(),
        creds.user_email.clone(),
    );

    secrets.insert("SYSTEMPROMPT_CLI_REMOTE".to_string(), "true".to_string());

    api_client.set_secrets(tenant_id, secrets).await
}

async fn cleanup_secrets(config: &CliConfig) -> Result<CommandResult<SecretsOutput>> {
    if !config.is_json_output() {
        CliService::section("Cleanup System-Managed Secrets");
    }

    let tenant_id = get_tenant_id()?;
    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let keys_to_remove = ["SYSTEMPROMPT_API_URL"];
    let mut removed = Vec::new();
    let mut errors = Vec::new();

    for key in keys_to_remove {
        if !config.is_json_output() {
            let spinner = CliService::spinner(&format!("Removing {key}..."));
            match client.unset_secret(&tenant_id, key).await {
                Ok(()) => {
                    spinner.finish_and_clear();
                    removed.push(key.to_string());
                },
                Err(e) => {
                    spinner.finish_and_clear();
                    errors.push((key, e.to_string()));
                },
            }
        } else {
            match client.unset_secret(&tenant_id, key).await {
                Ok(()) => removed.push(key.to_string()),
                Err(e) => errors.push((key, e.to_string())),
            }
        }
    }

    if !config.is_json_output() {
        if !removed.is_empty() {
            CliService::success(&format!(
                "Removed {} system-managed variables",
                removed.len()
            ));
            for key in &removed {
                CliService::info(&format!("  - {key}"));
            }
        }

        if !errors.is_empty() {
            for (key, err) in &errors {
                CliService::warning(&format!("Could not remove {key}: {err}"));
            }
        }

        if removed.is_empty() && errors.is_empty() {
            CliService::info("No system-managed variables to clean up");
        }
    }

    let output = SecretsOutput {
        operation: "cleanup".to_string(),
        keys: removed,
        rejected_keys: None,
    };

    Ok(CommandResult::list(output).with_title("Cleanup Secrets"))
}
