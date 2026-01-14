use anyhow::{Context, Result};
use clap::Args;
use std::fs;

use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_models::content_config::ContentConfigRaw;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::{ContentTypeListOutput, ContentTypeSummary};

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only enabled content types")]
    pub enabled: bool,

    #[arg(
        long,
        help = "Show only disabled content types",
        conflicts_with = "enabled"
    )]
    pub disabled: bool,

    #[arg(long, help = "Filter by category")]
    pub category: Option<String>,
}

pub fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<ContentTypeListOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let content_config_path = profile.paths.content_config();

    let content = fs::read_to_string(&content_config_path)
        .with_context(|| format!("Failed to read content config at {}", content_config_path))?;

    let config: ContentConfigRaw = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse content config at {}", content_config_path))?;

    let mut content_types: Vec<ContentTypeSummary> = config
        .content_sources
        .iter()
        .filter(|(_, source)| {
            if args.enabled {
                source.enabled
            } else if args.disabled {
                !source.enabled
            } else {
                true
            }
        })
        .filter(|(_, source)| {
            if let Some(ref category) = args.category {
                source.category_id.as_str() == category
            } else {
                true
            }
        })
        .map(|(name, source)| ContentTypeSummary {
            name: name.clone(),
            source_id: source.source_id.to_string(),
            category_id: source.category_id.to_string(),
            enabled: source.enabled,
            path: source.path.clone(),
            url_pattern: source.sitemap.as_ref().map(|s| s.url_pattern.clone()),
        })
        .collect();

    content_types.sort_by(|a, b| a.name.cmp(&b.name));

    let output = ContentTypeListOutput { content_types };

    Ok(CommandResult::table(output)
        .with_title("Content Types")
        .with_columns(vec![
            "name".to_string(),
            "source_id".to_string(),
            "category_id".to_string(),
            "enabled".to_string(),
            "path".to_string(),
            "url_pattern".to_string(),
        ]))
}
