use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::fs;

use crate::CliConfig;
use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::content_config::ContentConfigRaw;

use super::selection::prompt_content_type_selection;
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

pub fn execute(
    args: &EditArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContentTypeEditOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let content_config_path = profile.paths.content_config();

    let content = fs::read_to_string(&content_config_path)
        .with_context(|| format!("Failed to read content config at {}", content_config_path))?;

    let mut content_config: ContentConfigRaw = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse content config at {}", content_config_path))?;

    let name = resolve_required(args.name.clone(), "name", config, || {
        prompt_content_type_selection(&content_config, "Select content type to edit")
    })?;

    let source = content_config
        .content_sources
        .get_mut(&name)
        .ok_or_else(|| anyhow!("Content type '{}' not found", name))?;

    let mut changes = Vec::new();
    apply_basic_flags(source, args, &mut changes);
    apply_sitemap_flags(source, args, &mut changes, &name)?;
    apply_set_value_changes(source, &args.set_values, &mut changes)?;

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

fn apply_basic_flags(
    source: &mut systemprompt_models::content_config::ContentSourceConfigRaw,
    args: &EditArgs,
    changes: &mut Vec<String>,
) {
    if args.enable {
        source.enabled = true;
        changes.push("enabled: true".to_string());
    }
    if args.disable {
        source.enabled = false;
        changes.push("enabled: false".to_string());
    }
    if let Some(ref path) = args.path {
        source.path.clone_from(path);
        changes.push(format!("path: {}", path));
    }
    if let Some(ref description) = args.description {
        source.description.clone_from(description);
        changes.push(format!("description: {}", description));
    }
}

fn apply_sitemap_flags(
    source: &mut systemprompt_models::content_config::ContentSourceConfigRaw,
    args: &EditArgs,
    changes: &mut Vec<String>,
    name: &str,
) -> Result<()> {
    if args.url_pattern.is_none() && args.priority.is_none() && args.changefreq.is_none() {
        return Ok(());
    }
    let Some(ref mut sitemap) = source.sitemap else {
        return Err(anyhow!(
            "Content type '{}' has no sitemap configuration. Create sitemap config first.",
            name
        ));
    };
    if let Some(ref url_pattern) = args.url_pattern {
        sitemap.url_pattern.clone_from(url_pattern);
        changes.push(format!("sitemap.url_pattern: {}", url_pattern));
    }
    if let Some(priority) = args.priority {
        if !(0.0..=1.0).contains(&priority) {
            return Err(anyhow!("Priority must be between 0.0 and 1.0"));
        }
        sitemap.priority = priority;
        changes.push(format!("sitemap.priority: {}", priority));
    }
    if let Some(ref changefreq) = args.changefreq {
        sitemap.changefreq.clone_from(changefreq);
        changes.push(format!("sitemap.changefreq: {}", changefreq));
    }
    Ok(())
}

fn apply_set_value_changes(
    source: &mut systemprompt_models::content_config::ContentSourceConfigRaw,
    set_values: &[String],
    changes: &mut Vec<String>,
) -> Result<()> {
    for set_value in set_values {
        let parts: Vec<&str> = set_value.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid --set format: '{}'. Expected key=value",
                set_value
            ));
        }
        apply_set_key(source, parts[0], parts[1], changes)?;
    }
    Ok(())
}

fn apply_set_key(
    source: &mut systemprompt_models::content_config::ContentSourceConfigRaw,
    key: &str,
    value: &str,
    changes: &mut Vec<String>,
) -> Result<()> {
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
            sitemap_mut(source)?.url_pattern = value.to_string();
            changes.push(format!("sitemap.url_pattern: {}", value));
        },
        "sitemap.priority" => {
            let priority: f32 = value
                .parse()
                .map_err(|_| anyhow!("Invalid float value for priority: '{}'", value))?;
            sitemap_mut(source)?.priority = priority;
            changes.push(format!("sitemap.priority: {}", value));
        },
        "sitemap.changefreq" => {
            sitemap_mut(source)?.changefreq = value.to_string();
            changes.push(format!("sitemap.changefreq: {}", value));
        },
        _ => {
            return Err(anyhow!(
                "Unknown configuration key: '{}'. Supported keys: description, path, enabled, \
                 sitemap.url_pattern, sitemap.priority, sitemap.changefreq",
                key
            ));
        },
    }
    Ok(())
}

fn sitemap_mut(
    source: &mut systemprompt_models::content_config::ContentSourceConfigRaw,
) -> Result<&mut systemprompt_models::content_config::SitemapConfig> {
    source
        .sitemap
        .as_mut()
        .ok_or_else(|| anyhow!("No sitemap configuration exists"))
}
