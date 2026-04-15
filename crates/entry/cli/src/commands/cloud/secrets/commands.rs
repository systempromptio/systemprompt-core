use anyhow::{Result, bail};
use std::collections::HashMap;
use systemprompt_cloud::CloudApiClient;
use systemprompt_logging::CliService;

use super::helpers::{
    get_tenant_and_secrets_path, get_tenant_id, load_secrets_json, map_secrets_to_env_vars,
};
use crate::cli_settings::CliConfig;
use crate::commands::cloud::tenant::get_credentials;
use crate::commands::cloud::types::SecretsOutput;
use crate::shared::CommandResult;

pub async fn sync_secrets(config: &CliConfig) -> Result<CommandResult<SecretsOutput>> {
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
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let keys = if config.is_json_output() {
        client.set_secrets(tenant_id.as_str(), env_secrets).await?
    } else {
        let spinner = CliService::spinner("Syncing secrets...");
        match client.set_secrets(tenant_id.as_str(), env_secrets).await {
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
    };

    let output = SecretsOutput {
        operation: "sync".to_string(),
        keys,
        rejected_keys: None,
    };

    Ok(CommandResult::list(output).with_title("Sync Secrets"))
}

pub async fn set_secrets(
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
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let keys = if config.is_json_output() {
        client.set_secrets(tenant_id.as_str(), secrets).await?
    } else {
        let spinner = CliService::spinner("Setting secrets...");
        match client.set_secrets(tenant_id.as_str(), secrets).await {
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

pub async fn unset_secrets(
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
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let mut removed = Vec::new();
    let mut errors = Vec::new();

    for key in &uppercase_keys {
        if config.is_json_output() {
            match client.unset_secret(tenant_id.as_str(), key).await {
                Ok(()) => removed.push(key.clone()),
                Err(e) => errors.push((key.clone(), e.to_string())),
            }
        } else {
            let spinner = CliService::spinner(&format!("Removing {key}..."));
            match client.unset_secret(tenant_id.as_str(), key).await {
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

pub async fn cleanup_secrets(config: &CliConfig) -> Result<CommandResult<SecretsOutput>> {
    if !config.is_json_output() {
        CliService::section("Cleanup System-Managed Secrets");
    }

    let tenant_id = get_tenant_id()?;
    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let keys_to_remove = ["SYSTEMPROMPT_API_URL"];
    let mut removed = Vec::new();
    let mut errors = Vec::new();

    for key in keys_to_remove {
        if config.is_json_output() {
            match client.unset_secret(tenant_id.as_str(), key).await {
                Ok(()) => removed.push(key.to_string()),
                Err(e) => errors.push((key, e.to_string())),
            }
        } else {
            let spinner = CliService::spinner(&format!("Removing {key}..."));
            match client.unset_secret(tenant_id.as_str(), key).await {
                Ok(()) => {
                    spinner.finish_and_clear();
                    removed.push(key.to_string());
                },
                Err(e) => {
                    spinner.finish_and_clear();
                    errors.push((key, e.to_string()));
                },
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
