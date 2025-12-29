use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use systemprompt_core_content::models::{Content, ContentError};
use systemprompt_core_content::ContentRepository;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_models::{
    Config, ContentConfigRaw, ContentSourceConfigRaw, PathConfig, SitemapConfig, SystemPaths,
};
use tokio::fs;

use crate::content::render_markdown;
use crate::prerender::parent::{render_parent_route, RenderParentParams};
use crate::templates::data::{prepare_template_data, TemplateDataParams};
use crate::templates::{get_templates_path, load_web_config, TemplateEngine};

const MAX_RETRIES: u32 = 5;
const RETRY_DELAY_MS: u64 = 500;
const SLUG_PLACEHOLDER: &str = "{slug}";

struct PrerenderContext {
    db_pool: DbPool,
    config: ContentConfigRaw,
    web_config: serde_yaml::Value,
    templates: TemplateEngine,
    dist_dir: PathBuf,
}

pub async fn prerender_content(db_pool: DbPool) -> Result<()> {
    let ctx = load_prerender_context(db_pool).await?;
    let total_rendered = process_all_sources(&ctx).await?;
    tracing::info!(items_rendered = total_rendered, "Prerendering completed");
    Ok(())
}

async fn load_prerender_context(db_pool: DbPool) -> Result<PrerenderContext> {
    let global_config = Config::get()?;
    let config_path = SystemPaths::content_config(global_config);

    let yaml_content = fs::read_to_string(&config_path)
        .await
        .context("Failed to read content config")?;
    let config: ContentConfigRaw =
        serde_yaml::from_str(&yaml_content).context("Failed to parse content config")?;

    let web_config = load_web_config()
        .await
        .context("Failed to load web config")?;

    tracing::debug!(config_path = %config_path.display(), "Loaded config");

    let template_dir = get_templates_path(&web_config)?;
    let templates = TemplateEngine::new(&template_dir)
        .await
        .context("Failed to load templates")?;

    let dist_dir = PathConfig::from_profile()?.web_dist().clone();

    Ok(PrerenderContext {
        db_pool,
        config,
        web_config,
        templates,
        dist_dir,
    })
}

async fn process_all_sources(ctx: &PrerenderContext) -> Result<u32> {
    let mut total_rendered = 0;

    for (source_name, source) in &ctx.config.content_sources {
        let Some(sitemap_config) = get_enabled_sitemap(source_name, source) else {
            continue;
        };

        let rendered = process_source(ctx, source_name, source, sitemap_config).await?;
        total_rendered += rendered;
    }

    Ok(total_rendered)
}

fn get_enabled_sitemap<'a>(
    source_name: &str,
    source: &'a ContentSourceConfigRaw,
) -> Option<&'a SitemapConfig> {
    if !source.enabled {
        tracing::debug!(source = %source_name, "Skipping disabled source");
        return None;
    }

    source
        .sitemap
        .as_ref()
        .filter(|cfg| cfg.enabled)
        .or_else(|| {
            tracing::debug!(source = %source_name, "Skipping source with disabled sitemap");
            None
        })
}

async fn process_source(
    ctx: &PrerenderContext,
    source_name: &str,
    source: &ContentSourceConfigRaw,
    sitemap_config: &SitemapConfig,
) -> Result<u32> {
    let contents = fetch_content_for_source(ctx, source_name, source.source_id.as_str()).await;
    if contents.is_empty() {
        return Ok(0);
    }

    let items = contents_to_json(&contents);
    let popular_ids = fetch_popular_ids(ctx, source_name, source.source_id.as_str()).await;

    let rendered = render_all_items(ctx, source_name, sitemap_config, &items, &popular_ids).await?;
    let parent = render_parent_if_enabled(ctx, source_name, source, sitemap_config, &items).await?;
    Ok(rendered + parent)
}

async fn fetch_content_for_source(
    ctx: &PrerenderContext,
    source_name: &str,
    source_id: &str,
) -> Vec<Content> {
    if source_name.contains("skill") {
        return Vec::new();
    }
    match ContentRepository::new(&ctx.db_pool) {
        Ok(repo) => fetch_with_retries(&repo, source_id, source_name).await,
        Err(_) => Vec::new(),
    }
}

async fn fetch_with_retries(
    repo: &ContentRepository,
    source_id_str: &str,
    source_name: &str,
) -> Vec<Content> {
    let source_id = SourceId::new(source_id_str);

    for retry in 0..=MAX_RETRIES {
        match repo.list_by_source(&source_id).await {
            Ok(contents) if !contents.is_empty() => return contents,
            Ok(_) if retry < MAX_RETRIES => {
                tracing::warn!(source = %source_name, attempt = retry + 1, "Retrying");
                tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
            },
            _ => return Vec::new(),
        }
    }
    Vec::new()
}

fn contents_to_json(contents: &[Content]) -> Vec<serde_json::Value> {
    contents
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "slug": c.slug,
                "title": c.title,
                "description": c.description,
                "content": c.body,
                "author": c.author,
                "published_at": c.published_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                "updated_at": c.updated_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                "keywords": c.keywords,
                "content_type": c.kind,
                "image": c.image,
                "category_id": c.category_id,
                "source_id": c.source_id,
                "links": c.links,
            })
        })
        .collect()
}

async fn fetch_popular_ids(
    ctx: &PrerenderContext,
    source_name: &str,
    source_id_str: &str,
) -> Vec<String> {
    // Popular content is available for all sources, not just blog
    if source_name.is_empty() {
        return Vec::new();
    }

    let Ok(content_repo) = ContentRepository::new(&ctx.db_pool) else {
        return Vec::new();
    };

    let source_id = SourceId::new(source_id_str);
    match content_repo
        .get_popular_content_ids(&source_id, 30, 20)
        .await
    {
        Ok(ids) => ids.into_iter().map(|id| id.to_string()).collect(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get popular content IDs");
            Vec::new()
        },
    }
}

async fn render_all_items(
    ctx: &PrerenderContext,
    source_name: &str,
    sitemap_config: &SitemapConfig,
    items: &[serde_json::Value],
    popular_ids: &[String],
) -> Result<u32> {
    let config_value = serde_yaml::to_value(&ctx.config)?;
    let mut rendered = 0;

    for item in items {
        render_single_item(&RenderSingleItemParams {
            ctx,
            source_name,
            sitemap_config,
            item,
            all_items: items,
            popular_ids,
            config_value: &config_value,
        })
        .await?;
        rendered += 1;
    }

    Ok(rendered)
}

struct RenderSingleItemParams<'a> {
    ctx: &'a PrerenderContext,
    source_name: &'a str,
    sitemap_config: &'a SitemapConfig,
    item: &'a serde_json::Value,
    all_items: &'a [serde_json::Value],
    popular_ids: &'a [String],
    config_value: &'a serde_yaml::Value,
}

async fn render_single_item(params: &RenderSingleItemParams<'_>) -> Result<()> {
    let RenderSingleItemParams {
        ctx,
        source_name,
        sitemap_config,
        item,
        all_items,
        popular_ids,
        config_value,
    } = params;

    // Extract slug early for error context
    let item_slug = item
        .get("slug")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let markdown_content = item
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("content"))?;

    let content_html = render_markdown(markdown_content);

    let template_data = prepare_template_data(TemplateDataParams {
        item,
        all_items,
        popular_ids,
        config: config_value,
        web_config: &ctx.web_config,
        content_html: &content_html,
        url_pattern: &sitemap_config.url_pattern,
        db_pool: std::sync::Arc::clone(&ctx.db_pool),
    })
    .await
    .with_context(|| format!("Failed to prepare template data for item '{}'", item_slug))?;

    // Use source-specific template if available, fall back to generic
    let template_name = match *source_name {
        "papers" => "paper",
        name => format!("{}-post", name).leak(), // e.g., "blog-post", "docs-post"
    };

    let html = ctx
        .templates
        .render(template_name, &template_data)
        .with_context(|| format!("Failed to render template for item '{}'", item_slug))?;

    let slug = item
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("slug"))?;

    write_rendered_page(&ctx.dist_dir, &sitemap_config.url_pattern, slug, &html).await
}

async fn write_rendered_page(
    dist_dir: &Path,
    url_pattern: &str,
    slug: &str,
    html: &str,
) -> Result<()> {
    let output_dir = determine_output_dir(dist_dir, url_pattern, slug);
    fs::create_dir_all(&output_dir).await?;

    let output_path = output_dir.join("index.html");
    fs::write(&output_path, html).await?;

    let generated_path = url_pattern.replace(SLUG_PLACEHOLDER, slug);
    tracing::debug!(path = %generated_path, "Generated page");
    Ok(())
}

async fn render_parent_if_enabled(
    ctx: &PrerenderContext,
    source_name: &str,
    source: &ContentSourceConfigRaw,
    sitemap_config: &SitemapConfig,
    items: &[serde_json::Value],
) -> Result<u32> {
    let Some(parent_config) = &sitemap_config.parent_route else {
        return Ok(0);
    };

    if !parent_config.enabled {
        return Ok(0);
    }

    render_parent_route(RenderParentParams {
        items,
        config: &ctx.config,
        source,
        web_config: &ctx.web_config,
        parent_config,
        source_name,
        templates: &ctx.templates,
        dist_dir: &ctx.dist_dir,
    })
    .await?;

    Ok(1)
}

fn determine_output_dir(dist_dir: &Path, url_pattern: &str, slug: &str) -> PathBuf {
    let path = url_pattern.replace(SLUG_PLACEHOLDER, slug);
    let path = path.trim_start_matches('/');
    dist_dir.join(path)
}
