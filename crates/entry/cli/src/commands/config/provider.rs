use anyhow::Result;
use clap::{Args, Subcommand};

use super::types::{
    read_yaml_file, write_yaml_file, ConfigSection, ProviderInfo, ProviderListOutput,
    ProviderSetOutput,
};
use crate::shared::CommandResult;
use crate::CliConfig;

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

#[derive(Debug, Clone, Args)]
pub struct ListArgs {}

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

pub fn execute(
    cmd: ProviderCommands,
    _config: &CliConfig,
) -> Result<CommandResult<serde_json::Value>> {
    match cmd {
        ProviderCommands::List(_args) => {
            let result = list_providers()?;
            Ok(CommandResult::table(serde_json::to_value(result)?).with_title("AI Providers"))
        },
        ProviderCommands::Set(args) => {
            let result = set_default_provider(&args.provider)?;
            Ok(CommandResult::card(serde_json::to_value(result)?).with_title("Provider Updated"))
        },
        ProviderCommands::Enable(args) => {
            let result = set_provider_enabled(&args.provider, true)?;
            Ok(CommandResult::card(serde_json::to_value(result)?).with_title("Provider Enabled"))
        },
        ProviderCommands::Disable(args) => {
            let result = set_provider_enabled(&args.provider, false)?;
            Ok(CommandResult::card(serde_json::to_value(result)?).with_title("Provider Disabled"))
        },
    }
}

fn get_ai_config_path() -> Result<std::path::PathBuf> {
    ConfigSection::Ai.file_path()
}

fn list_providers() -> Result<ProviderListOutput> {
    let file_path = get_ai_config_path()?;
    let content = read_yaml_file(&file_path)?;

    let ai = content
        .get("ai")
        .ok_or_else(|| anyhow::anyhow!("Missing 'ai' section in config"))?;

    let default_provider = ai
        .get("default_provider")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let providers_section = ai.get("providers");

    let mut providers = Vec::new();

    if let Some(serde_yaml::Value::Mapping(providers_map)) = providers_section {
        for (name, config) in providers_map {
            let name_str = name.as_str().unwrap_or("unknown").to_string();

            let enabled = config
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let model = config
                .get("default_model")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let endpoint = config
                .get("endpoint")
                .and_then(|v| v.as_str())
                .map(String::from);

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
    let file_path = get_ai_config_path()?;
    let mut content = read_yaml_file(&file_path)?;

    let providers = content
        .get("ai")
        .and_then(|ai| ai.get("providers"))
        .ok_or_else(|| anyhow::anyhow!("Missing providers section"))?;

    if !providers
        .as_mapping()
        .is_some_and(|m| m.contains_key(serde_yaml::Value::String(provider.to_string())))
    {
        anyhow::bail!(
            "Unknown provider: '{}'. Available providers: {:?}",
            provider,
            providers
                .as_mapping()
                .map(|m| m.keys().filter_map(|k| k.as_str()).collect::<Vec<_>>())
                .unwrap_or_else(String::new)
        );
    }

    if let Some(ai) = content.get_mut("ai") {
        if let serde_yaml::Value::Mapping(ai_map) = ai {
            ai_map.insert(
                serde_yaml::Value::String("default_provider".to_string()),
                serde_yaml::Value::String(provider.to_string()),
            );
        }
    }

    write_yaml_file(&file_path, &content)?;

    Ok(ProviderSetOutput {
        provider: provider.to_string(),
        action: "set_default".to_string(),
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

    if let serde_yaml::Value::Mapping(config_map) = provider_config {
        config_map.insert(
            serde_yaml::Value::String("enabled".to_string()),
            serde_yaml::Value::Bool(enabled),
        );
    }

    write_yaml_file(&file_path, &content)?;

    let action = if enabled { "enabled" } else { "disabled" };

    Ok(ProviderSetOutput {
        provider: provider.to_string(),
        action: action.to_string(),
        message: format!("Provider '{}' {}", provider, action),
    })
}
