use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::fs;

use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use systemprompt_core_logging::CliService;
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_models::content_config::{
    ContentConfigRaw, ContentSourceConfigRaw, IndexingConfig, SitemapConfig,
};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::ContentTypeCreateOutput;

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Content type name")]
    pub name: Option<String>,

    #[arg(long, help = "Content path (relative to services)")]
    pub path: Option<String>,

    #[arg(long, help = "Source ID")]
    pub source_id: Option<String>,

    #[arg(long, help = "Category ID")]
    pub category_id: Option<String>,

    #[arg(long, help = "Description")]
    pub description: Option<String>,

    #[arg(long, help = "Enable the content type")]
    pub enabled: bool,

    #[arg(long, help = "URL pattern for sitemap (e.g., /blog/{slug})")]
    pub url_pattern: Option<String>,

    #[arg(long, help = "Sitemap priority (0.0-1.0)", default_value = "0.5")]
    pub priority: f32,

    #[arg(
        long,
        help = "Sitemap change frequency",
        default_value = "weekly"
    )]
    pub changefreq: String,
}

pub fn execute(args: CreateArgs, config: &CliConfig) -> Result<CommandResult<ContentTypeCreateOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let content_config_path = profile.paths.content_config();

    let content = fs::read_to_string(&content_config_path)
        .with_context(|| format!("Failed to read content config at {}", content_config_path))?;

    let mut content_config: ContentConfigRaw = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse content config at {}", content_config_path))?;

    let name = resolve_input(args.name, "name", config, prompt_name)?;

    if content_config.content_sources.contains_key(&name) {
        return Err(anyhow!("Content type '{}' already exists", name));
    }

    let path = resolve_input(args.path, "path", config, || prompt_path(&name))?;
    let source_id = resolve_input(args.source_id, "source-id", config, || {
        prompt_source_id(&name)
    })?;
    let category_id = resolve_input(args.category_id, "category-id", config, || {
        prompt_category_id(&content_config)
    })?;

    if !content_config.categories.contains_key(&category_id) {
        let available: Vec<&String> = content_config.categories.keys().collect();
        return Err(anyhow!(
            "Category '{}' not found. Available categories: {:?}",
            category_id,
            available
        ));
    }

    let description = args.description.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_description().unwrap_or_default()
        } else {
            String::new()
        }
    });

    let sitemap = if args.url_pattern.is_some() {
        Some(SitemapConfig {
            enabled: true,
            url_pattern: args.url_pattern.unwrap(),
            priority: args.priority,
            changefreq: args.changefreq.clone(),
            fetch_from: "database".to_string(),
            parent_route: None,
        })
    } else if config.is_interactive() {
        prompt_sitemap_config()?
    } else {
        None
    };

    let source_config = ContentSourceConfigRaw {
        path,
        source_id: SourceId::new(&source_id),
        category_id: CategoryId::new(&category_id),
        enabled: args.enabled,
        description,
        allowed_content_types: vec!["article".to_string()],
        indexing: Some(IndexingConfig {
            clear_before: false,
            recursive: true,
            override_existing: false,
        }),
        sitemap,
        branding: None,
    };

    content_config
        .content_sources
        .insert(name.clone(), source_config);

    let yaml = serde_yaml::to_string(&content_config).context("Failed to serialize config")?;
    fs::write(&content_config_path, yaml)
        .with_context(|| format!("Failed to write content config to {}", content_config_path))?;

    CliService::success(&format!("Content type '{}' created successfully", name));

    let output = ContentTypeCreateOutput {
        name: name.clone(),
        message: format!("Content type '{}' created successfully", name),
    };

    Ok(CommandResult::text(output).with_title("Content Type Created"))
}

fn prompt_name() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Content type name")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.len() < 2 {
                return Err("Name must be at least 2 characters");
            }
            if !input
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err("Name must be lowercase alphanumeric with hyphens only");
            }
            Ok(())
        })
        .interact_text()
        .context("Failed to get name")
}

fn prompt_path(name: &str) -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Content path")
        .default(format!("content/{}", name))
        .interact_text()
        .context("Failed to get path")
}

fn prompt_source_id(name: &str) -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Source ID")
        .default(name.to_string())
        .interact_text()
        .context("Failed to get source ID")
}

fn prompt_category_id(content_config: &ContentConfigRaw) -> Result<String> {
    let mut categories: Vec<&String> = content_config.categories.keys().collect();
    categories.sort();

    if categories.is_empty() {
        return Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Category ID")
            .default("blog".to_string())
            .interact_text()
            .context("Failed to get category ID");
    }

    let selection = dialoguer::Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select category")
        .items(&categories)
        .default(0)
        .interact()
        .context("Failed to get category selection")?;

    Ok(categories[selection].clone())
}

fn prompt_description() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Description")
        .allow_empty(true)
        .interact_text()
        .context("Failed to get description")
}

fn prompt_sitemap_config() -> Result<Option<SitemapConfig>> {
    let enable_sitemap = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Enable sitemap?")
        .default(true)
        .interact()
        .context("Failed to get sitemap preference")?;

    if !enable_sitemap {
        return Ok(None);
    }

    let url_pattern: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("URL pattern (e.g., /blog/{slug})")
        .interact_text()
        .context("Failed to get URL pattern")?;

    let priority: f32 = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Priority (0.0-1.0)")
        .default(0.5)
        .validate_with(|input: &f32| -> Result<(), &str> {
            if *input < 0.0 || *input > 1.0 {
                return Err("Priority must be between 0.0 and 1.0");
            }
            Ok(())
        })
        .interact()
        .context("Failed to get priority")?;

    let changefreq: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Change frequency")
        .default("weekly".to_string())
        .interact_text()
        .context("Failed to get change frequency")?;

    Ok(Some(SitemapConfig {
        enabled: true,
        url_pattern,
        priority,
        changefreq,
        fetch_from: "database".to_string(),
        parent_route: None,
    }))
}
