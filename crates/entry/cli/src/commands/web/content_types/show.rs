use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::fs;

use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use crate::CliConfig;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_models::content_config::ContentConfigRaw;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::{
    BrandingInfo, ContentTypeDetailOutput, IndexingInfo, ParentRouteInfo, SitemapInfo,
};

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Content type name")]
    pub name: Option<String>,
}

pub fn execute(
    args: ShowArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContentTypeDetailOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let content_config_path = profile.paths.content_config();

    let content = fs::read_to_string(&content_config_path)
        .with_context(|| format!("Failed to read content config at {}", content_config_path))?;

    let content_config: ContentConfigRaw = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse content config at {}", content_config_path))?;

    let name = resolve_required(args.name, "name", config, || {
        prompt_content_type_selection(&content_config)
    })?;

    let source = content_config
        .content_sources
        .get(&name)
        .ok_or_else(|| anyhow!("Content type '{}' not found", name))?;

    let output = ContentTypeDetailOutput {
        name: name.clone(),
        source_id: source.source_id.to_string(),
        category_id: source.category_id.to_string(),
        enabled: source.enabled,
        path: source.path.clone(),
        description: source.description.clone(),
        allowed_content_types: source.allowed_content_types.clone(),
        sitemap: source.sitemap.as_ref().map(|s| SitemapInfo {
            enabled: s.enabled,
            url_pattern: s.url_pattern.clone(),
            priority: s.priority,
            changefreq: s.changefreq.clone(),
            fetch_from: s.fetch_from.clone(),
            parent_route: s.parent_route.as_ref().map(|p| ParentRouteInfo {
                enabled: p.enabled,
                url: p.url.clone(),
                priority: p.priority,
                changefreq: p.changefreq.clone(),
            }),
        }),
        branding: source.branding.as_ref().map(|b| BrandingInfo {
            name: b.name.clone(),
            description: b.description.clone(),
            image: b.image.clone(),
            keywords: b.keywords.clone(),
        }),
        indexing: source.indexing.map(|i| IndexingInfo {
            clear_before: i.clear_before,
            recursive: i.recursive,
            override_existing: i.override_existing,
        }),
    };

    Ok(CommandResult::card(output).with_title(format!("Content Type: {}", name)))
}

fn prompt_content_type_selection(config: &ContentConfigRaw) -> Result<String> {
    let mut names: Vec<&String> = config.content_sources.keys().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No content types configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select content type")
        .items(&names)
        .default(0)
        .interact()
        .context("Failed to get content type selection")?;

    Ok(names[selection].clone())
}
