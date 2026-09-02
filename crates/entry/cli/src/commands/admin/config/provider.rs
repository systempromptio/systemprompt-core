//! `admin config provider` command: manage AI providers in the
//! `ai/config.yaml`.
//!
//! [`ProviderCommands`] lists providers, sets the default, and toggles a
//! provider's enabled flag, editing the AI config YAML in place.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;

use super::types::{
    ConfigSection, ProviderInfo, ProviderListOutput, ProviderSetOutput, read_yaml_file,
    write_yaml_file,
};
use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Subcommand)]
pub enum ProviderCommands {
    #[command(about = "List AI providers")]
    List(ListArgs),

    #[command(about = "Set default provider")]
    Set(SetArgs),

    #[command(about = "Enable a provider")]
    Enable(EnableArgs),

    #[command(about = "Disable a provider")]
    Disable(DisableArgs),
}

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs;

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(value_name = "PROVIDER")]
    pub provider: String,
}

#[derive(Debug, Clone, Args)]
pub struct EnableArgs {
    #[arg(value_name = "PROVIDER")]
    pub provider: String,
}

#[derive(Debug, Clone, Args)]
pub struct DisableArgs {
    #[arg(value_name = "PROVIDER")]
    pub provider: String,
}

pub fn execute(cmd: ProviderCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        ProviderCommands::List(_args) => {
            let result = list_providers()?;
            render_result(
                &CommandOutput::table_of(
                    vec!["name", "enabled", "is_default", "model", "endpoint"],
                    &result.providers,
                )
                .with_title("AI Providers"),
                config,
            );
        },
        ProviderCommands::Set(args) => {
            let result = set_default_provider(&args.provider)?;
            render_result(
                &CommandOutput::card_value("Provider Updated", &result),
                config,
            );
        },
        ProviderCommands::Enable(args) => {
            let result = set_provider_enabled(&args.provider, true)?;
            render_result(
                &CommandOutput::card_value("Provider Enabled", &result),
                config,
            );
        },
        ProviderCommands::Disable(args) => {
            let result = set_provider_enabled(&args.provider, false)?;
            render_result(
                &CommandOutput::card_value("Provider Disabled", &result),
                config,
            );
        },
    }
    Ok(())
}

fn get_ai_config_path() -> Result<std::path::PathBuf> {
    ConfigSection::Ai.file_path()
}

fn list_providers() -> Result<ProviderListOutput> {
    let registry = &ProfileBootstrap::get()?.providers;
    let file_path = get_ai_config_path()?;
    let content = read_yaml_file(&file_path)?;

    let ai = content
        .get("ai")
        .ok_or_else(|| anyhow::anyhow!("Missing 'ai' section in config"))?;

    let default_provider = ai
        .get("default_provider")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_owned();

    let providers_section = ai.get("providers");

    let mut providers = Vec::new();

    if let Some(serde_yaml::Value::Mapping(providers_map)) = providers_section {
        for (name, config) in providers_map {
            let name_str = name.as_str().unwrap_or("unknown").to_owned();

            let enabled = config
                .get("enabled")
                .and_then(serde_yaml::Value::as_bool)
                .unwrap_or(true);

            let model = config
                .get("default_model")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_owned();

            let endpoint = registry
                .find_provider(&name_str)
                .map(|entry| entry.endpoint.clone());

            providers.push(ProviderInfo {
                name: name_str.clone(),
                enabled,
                is_default: name_str == default_provider,
                model,
                endpoint,
            });
        }
    }

    Ok(ProviderListOutput {
        providers,
        default_provider,
    })
}

fn set_default_provider(provider: &str) -> Result<ProviderSetOutput> {
    let registry = &ProfileBootstrap::get()?.providers;
    if registry.find_provider(provider).is_none() {
        let available: Vec<&str> = registry.providers.iter().map(|p| p.name.as_str()).collect();
        anyhow::bail!(
            "Unknown provider: '{}' is not in profile.providers. Available: {:?}",
            provider,
            available
        );
    }

    let file_path = get_ai_config_path()?;
    let mut content = read_yaml_file(&file_path)?;

    let policy = content.get("ai").and_then(|ai| ai.get("providers"));
    let enabled = policy
        .and_then(|p| p.get(provider))
        .and_then(|p| p.get("enabled"))
        .and_then(serde_yaml::Value::as_bool)
        .unwrap_or(true);
    if !enabled {
        anyhow::bail!(
            "Provider '{}' is disabled in AI policy; enable it first \
             (admin config provider enable {})",
            provider,
            provider
        );
    }

    // JSON: mutates one key in the untyped AI config document, leaving unknown
    // operator-authored keys in place.
    if let Some(serde_yaml::Value::Mapping(ai_map)) = content.get_mut("ai") {
        ai_map.insert(
            serde_yaml::Value::String("default_provider".to_owned()),
            serde_yaml::Value::String(provider.to_owned()),
        );
    }

    write_yaml_file(&file_path, &content)?;

    Ok(ProviderSetOutput {
        provider: provider.to_owned(),
        action: "set_default".to_owned(),
        message: format!("Default provider set to '{}'", provider),
    })
}

fn set_provider_enabled(provider: &str, enabled: bool) -> Result<ProviderSetOutput> {
    let file_path = get_ai_config_path()?;
    let mut content = read_yaml_file(&file_path)?;

    let ai = content
        .get_mut("ai")
        .ok_or_else(|| anyhow::anyhow!("Missing 'ai' section"))?;

    let providers = ai
        .get_mut("providers")
        .ok_or_else(|| anyhow::anyhow!("Missing 'providers' section"))?;

    let provider_config = providers
        .get_mut(provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown provider: '{}'", provider))?;

    // JSON: mutates one key in the untyped AI config document, leaving unknown
    // operator-authored keys in place.
    if let serde_yaml::Value::Mapping(config_map) = provider_config {
        config_map.insert(
            serde_yaml::Value::String("enabled".to_owned()),
            serde_yaml::Value::Bool(enabled),
        );
    }

    write_yaml_file(&file_path, &content)?;

    let action = if enabled { "enabled" } else { "disabled" };

    Ok(ProviderSetOutput {
        provider: provider.to_owned(),
        action: action.to_owned(),
        message: format!("Provider '{}' {}", provider, action),
    })
}
