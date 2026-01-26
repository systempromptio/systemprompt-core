use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::fs;

use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use crate::CliConfig;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_logging::CliService;
use systemprompt_models::content_config::ContentConfigRaw;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::ContentTypeEditOutput;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Content type name")]
    pub name: Option<String>,

    #[arg(
        long = "set",
        value_name = "KEY=VALUE",
        help = "Set a configuration value"
    )]
    pub set_values: Vec<String>,

    #[arg(long, help = "Enable the content type", conflicts_with = "disable")]
    pub enable: bool,

    #[arg(long, help = "Disable the content type", conflicts_with = "enable")]
    pub disable: bool,

    #[arg(long, help = "Set the URL pattern")]
    pub url_pattern: Option<String>,

    #[arg(long, help = "Set the sitemap priority (0.0-1.0)")]
    pub priority: Option<f32>,

    #[arg(long, help = "Set the change frequency")]
    pub changefreq: Option<String>,

    #[arg(long, help = "Set the path")]
    pub path: Option<String>,

    #[arg(long, help = "Set the description")]
    pub description: Option<String>,
}

pub fn execute(args: EditArgs, config: &CliConfig) -> Result<CommandResult<ContentTypeEditOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let content_config_path = profile.paths.content_config();

    let content = fs::read_to_string(&content_config_path)
        .with_context(|| format!("Failed to read content config at {}", content_config_path))?;

    let mut content_config: ContentConfigRaw = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse content config at {}", content_config_path))?;

    let name = resolve_required(args.name, "name", config, || {
        prompt_content_type_selection(&content_config)
    })?;

    let source = content_config
        .content_sources
        .get_mut(&name)
        .ok_or_else(|| anyhow!("Content type '{}' not found", name))?;

    let mut changes = Vec::new();

    if args.enable {
        source.enabled = true;
        changes.push("enabled: true".to_string());
    }

    if args.disable {
        source.enabled = false;
        changes.push("enabled: false".to_string());
    }

    if let Some(path) = args.path {
        source.path.clone_from(&path);
        changes.push(format!("path: {}", path));
    }

    if let Some(description) = args.description {
        source.description.clone_from(&description);
        changes.push(format!("description: {}", description));
    }

    if args.url_pattern.is_some() || args.priority.is_some() || args.changefreq.is_some() {
        if let Some(ref mut sitemap) = source.sitemap {
            if let Some(url_pattern) = args.url_pattern {
                sitemap.url_pattern.clone_from(&url_pattern);
                changes.push(format!("sitemap.url_pattern: {}", url_pattern));
            }
            if let Some(priority) = args.priority {
                if !(0.0..=1.0).contains(&priority) {
                    return Err(anyhow!("Priority must be between 0.0 and 1.0"));
                }
                sitemap.priority = priority;
                changes.push(format!("sitemap.priority: {}", priority));
            }
            if let Some(changefreq) = args.changefreq {
                sitemap.changefreq.clone_from(&changefreq);
                changes.push(format!("sitemap.changefreq: {}", changefreq));
            }
        } else {
            return Err(anyhow!(
                "Content type '{}' has no sitemap configuration. Create sitemap config first.",
                name
            ));
        }
    }

    for set_value in &args.set_values {
        let parts: Vec<&str> = set_value.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid --set format: '{}'. Expected key=value",
                set_value
            ));
        }
        let key = parts[0];
        let value = parts[1];

        match key {
            "description" => {
                source.description = value.to_string();
                changes.push(format!("description: {}", value));
            },
            "path" => {
                source.path = value.to_string();
                changes.push(format!("path: {}", value));
            },
            "enabled" => {
                source.enabled = value
                    .parse()
                    .map_err(|_| anyhow!("Invalid boolean value for enabled: '{}'", value))?;
                changes.push(format!("enabled: {}", value));
            },
            "sitemap.url_pattern" => {
                if let Some(ref mut sitemap) = source.sitemap {
                    sitemap.url_pattern = value.to_string();
                    changes.push(format!("sitemap.url_pattern: {}", value));
                } else {
                    return Err(anyhow!("No sitemap configuration exists"));
                }
            },
            "sitemap.priority" => {
                if let Some(ref mut sitemap) = source.sitemap {
                    let priority: f32 = value
                        .parse()
                        .map_err(|_| anyhow!("Invalid float value for priority: '{}'", value))?;
                    sitemap.priority = priority;
                    changes.push(format!("sitemap.priority: {}", value));
                } else {
                    return Err(anyhow!("No sitemap configuration exists"));
                }
            },
            "sitemap.changefreq" => {
                if let Some(ref mut sitemap) = source.sitemap {
                    sitemap.changefreq = value.to_string();
                    changes.push(format!("sitemap.changefreq: {}", value));
                } else {
                    return Err(anyhow!("No sitemap configuration exists"));
                }
            },
            _ => {
                return Err(anyhow!(
                    "Unknown configuration key: '{}'. Supported keys: description, path, enabled, \
                     sitemap.url_pattern, sitemap.priority, sitemap.changefreq",
                    key
                ));
            },
        }
    }

    if changes.is_empty() {
        return Err(anyhow!(
            "No changes specified. Use --enable, --disable, --path, --description, --url-pattern, \
             --priority, --changefreq, or --set key=value"
        ));
    }

    CliService::info(&format!("Updating content type '{}'...", name));

    let yaml = serde_yaml::to_string(&content_config).context("Failed to serialize config")?;
    fs::write(&content_config_path, yaml)
        .with_context(|| format!("Failed to write content config to {}", content_config_path))?;

    CliService::success(&format!("Content type '{}' updated successfully", name));

    let output = ContentTypeEditOutput {
        name: name.clone(),
        message: format!(
            "Content type '{}' updated successfully with {} change(s)",
            name,
            changes.len()
        ),
        changes,
    };

    Ok(CommandResult::text(output).with_title(format!("Edit Content Type: {}", name)))
}

fn prompt_content_type_selection(config: &ContentConfigRaw) -> Result<String> {
    let mut names: Vec<&String> = config.content_sources.keys().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No content types configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select content type to edit")
        .items(&names)
        .default(0)
        .interact()
        .context("Failed to get content type selection")?;

    Ok(names[selection].clone())
}
