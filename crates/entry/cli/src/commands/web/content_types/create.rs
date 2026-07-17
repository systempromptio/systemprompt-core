//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::fs;
use systemprompt_identifiers::{CategoryId, SourceId};

use crate::CliConfig;
use crate::interactive::{Prompter, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::content_config::{ContentConfigRaw, SitemapConfig};

use super::builder::{SourceSpec, build_flag_sitemap, build_source_config, ensure_category_exists};

use super::super::types::ContentTypeCreateOutput;

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Content type name")]
    pub name: Option<String>,

    #[arg(long, help = "Content path (relative to services)")]
    pub path: Option<String>,

    #[arg(long = "source-id", help = "Source ID")]
    pub source: Option<String>,

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

    #[arg(long, help = "Sitemap change frequency", default_value = "weekly")]
    pub changefreq: String,
}

fn resolve_description(
    description: Option<String>,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> String {
    description.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_description(prompter).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to prompt for description");
                String::new()
            })
        } else {
            String::new()
        }
    })
}

fn resolve_sitemap(
    url_pattern: Option<String>,
    priority: f32,
    changefreq: &str,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<Option<SitemapConfig>> {
    if let Some(url_pattern) = url_pattern {
        return Ok(Some(build_flag_sitemap(url_pattern, priority, changefreq)));
    }
    if config.is_interactive() {
        return prompt_sitemap_config(prompter);
    }
    Ok(None)
}

pub(super) fn execute(
    args: CreateArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let content_config_path = profile.paths.content_config();

    let content = fs::read_to_string(&content_config_path)
        .with_context(|| format!("Failed to read content config at {}", content_config_path))?;

    let mut content_config: ContentConfigRaw = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse content config at {}", content_config_path))?;

    let name = resolve_required(args.name, "name", config, || prompt_name(prompter))?;

    if content_config.content_sources.contains_key(&name) {
        return Err(anyhow!("Content type '{}' already exists", name));
    }

    let path = resolve_required(args.path, "path", config, || prompt_path(prompter, &name))?;
    let source_id = resolve_required(args.source, "source-id", config, || {
        prompt_source_id(prompter, &name)
    })?;
    let category_id = resolve_required(args.category_id, "category-id", config, || {
        prompt_category_id(prompter, &content_config)
    })?;

    ensure_category_exists(&content_config, &category_id)?;

    let sitemap = resolve_sitemap(
        args.url_pattern,
        args.priority,
        &args.changefreq,
        prompter,
        config,
    )?;
    let description = resolve_description(args.description, prompter, config);

    let source_config = build_source_config(SourceSpec {
        path,
        source_id: SourceId::new(&source_id),
        category_id: CategoryId::new(&category_id),
        enabled: args.enabled,
        description,
        sitemap,
    });

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

    Ok(CommandOutput::card_value("Content Type Created", &output))
}

fn validate_type_name(input: &str) -> Result<(), &'static str> {
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
}

pub fn prompt_name(prompter: &dyn Prompter) -> Result<String> {
    loop {
        let input = prompter.input("Content type name")?;
        let trimmed = input.trim();
        match validate_type_name(trimmed) {
            Ok(()) => return Ok(trimmed.to_owned()),
            Err(message) => CliService::warning(message),
        }
    }
}

pub fn prompt_path(prompter: &dyn Prompter, name: &str) -> Result<String> {
    prompter.input_with_default("Content path", &format!("content/{}", name))
}

pub fn prompt_source_id(prompter: &dyn Prompter, name: &str) -> Result<String> {
    prompter.input_with_default("Source ID", name)
}

pub fn prompt_category_id(
    prompter: &dyn Prompter,
    content_config: &ContentConfigRaw,
) -> Result<String> {
    let mut categories: Vec<String> = content_config.categories.keys().cloned().collect();
    categories.sort();

    if categories.is_empty() {
        return prompter.input_with_default("Category ID", "blog");
    }

    let selection = prompter.select("Select category", &categories)?;
    Ok(categories[selection].clone())
}

pub fn prompt_description(prompter: &dyn Prompter) -> Result<String> {
    prompter.input("Description")
}

pub fn prompt_sitemap_config(prompter: &dyn Prompter) -> Result<Option<SitemapConfig>> {
    if !prompter.confirm("Enable sitemap?", true)? {
        return Ok(None);
    }

    let url_pattern = prompter.input("URL pattern (e.g., /blog/{slug})")?;

    let priority = loop {
        let raw = prompter.input_with_default("Priority (0.0-1.0)", "0.5")?;
        match raw.trim().parse::<f32>() {
            Ok(value) if (0.0..=1.0).contains(&value) => break value,
            Ok(_) => CliService::warning("Priority must be between 0.0 and 1.0"),
            Err(e) => CliService::warning(&format!("Priority must be a number: {}", e)),
        }
    };

    let changefreq = prompter.input_with_default("Change frequency", "weekly")?;

    Ok(Some(SitemapConfig {
        enabled: true,
        url_pattern,
        priority,
        changefreq,
        fetch_from: "database".to_owned(),
        parent_route: None,
    }))
}
