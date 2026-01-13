use anyhow::{Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Password, Select};
use serde::{Deserialize, Serialize};
use std::path::Path;
use systemprompt_core_logging::CliService;

use crate::shared::profile::generate_jwt_secret;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsData {
    pub jwt_secret: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
}

impl SecretsData {
    pub const fn has_ai_provider(&self) -> bool {
        self.gemini.is_some() || self.anthropic.is_some() || self.openai.is_some()
    }

    pub fn summary(&self) -> String {
        let mut keys = Vec::new();
        if self.gemini.is_some() {
            keys.push("Gemini");
        }
        if self.anthropic.is_some() {
            keys.push("Anthropic");
        }
        if self.openai.is_some() {
            keys.push("OpenAI");
        }
        if self.github.is_some() {
            keys.push("GitHub");
        }

        if keys.is_empty() {
            "None".to_string()
        } else {
            keys.join(", ")
        }
    }
}

pub fn collect_interactive(env_name: &str) -> Result<SecretsData> {
    CliService::section(&format!("Secrets Setup ({})", env_name));
    CliService::info("At least one AI provider API key is required.");

    let jwt_secret = generate_jwt_secret();
    CliService::success("Generated secure JWT secret (64 characters)");

    let mut secrets = SecretsData {
        jwt_secret,
        ..Default::default()
    };

    let providers = vec![
        "Google AI (Gemini) - https://aistudio.google.com/app/apikey",
        "Anthropic (Claude) - https://console.anthropic.com/api-keys",
        "OpenAI (GPT) - https://platform.openai.com/api-keys",
        "Enter multiple keys",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select your AI provider")
        .items(&providers)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            let key = prompt_api_key("Gemini API Key")?;
            secrets.gemini = Some(key);
        },
        1 => {
            let key = prompt_api_key("Anthropic API Key")?;
            secrets.anthropic = Some(key);
        },
        2 => {
            let key = prompt_api_key("OpenAI API Key")?;
            secrets.openai = Some(key);
        },
        3 => {
            CliService::info("Enter API keys (press Enter to skip any):");

            if let Some(key) = prompt_optional_api_key("Gemini API Key")? {
                secrets.gemini = Some(key);
            }
            if let Some(key) = prompt_optional_api_key("Anthropic API Key")? {
                secrets.anthropic = Some(key);
            }
            if let Some(key) = prompt_optional_api_key("OpenAI API Key")? {
                secrets.openai = Some(key);
            }
            if let Some(key) = prompt_optional_api_key("GitHub Token (optional)")? {
                secrets.github = Some(key);
            }
        },
        _ => unreachable!(),
    }

    validate_secrets(&secrets)?;

    CliService::success(&format!("Configured keys: {}", secrets.summary()));

    Ok(secrets)
}

fn prompt_api_key(prompt: &str) -> Result<String> {
    let key = Password::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact()?;

    if key.is_empty() {
        anyhow::bail!("API key is required");
    }

    Ok(key)
}

fn prompt_optional_api_key(prompt: &str) -> Result<Option<String>> {
    let key = Password::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .allow_empty_password(true)
        .interact()?;

    if key.is_empty() {
        Ok(None)
    } else {
        Ok(Some(key))
    }
}

fn validate_secrets(secrets: &SecretsData) -> Result<()> {
    if secrets.jwt_secret.len() < 32 {
        anyhow::bail!("JWT secret must be at least 32 characters");
    }

    if !secrets.has_ai_provider() {
        anyhow::bail!("At least one AI provider API key is required");
    }

    Ok(())
}

pub fn save(secrets: &SecretsData, secrets_path: &Path) -> Result<()> {
    if let Some(parent) = secrets_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(secrets).context("Failed to serialize secrets")?;

    std::fs::write(secrets_path, content)
        .with_context(|| format!("Failed to write {}", secrets_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(secrets_path, permissions)
            .with_context(|| format!("Failed to set permissions on {}", secrets_path.display()))?;
    }

    CliService::success(&format!("Saved secrets to {}", secrets_path.display()));

    Ok(())
}

pub fn default_path(systemprompt_dir: &Path, env_name: &str) -> std::path::PathBuf {
    systemprompt_dir
        .join("secrets")
        .join(format!("{}.secrets.json", env_name))
}
