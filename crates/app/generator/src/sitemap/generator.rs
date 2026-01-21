use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::path::Path;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, ContentSourceConfigRaw};
use tokio::fs;

use super::xml::{build_sitemap_index, build_sitemap_xml, SitemapUrl};

const MAX_URLS_PER_SITEMAP: usize = 50_000;
const SLUG_PLACEHOLDER: &str = "{slug}";

struct SitemapContext {
    config: ContentConfigRaw,
    db_pool: DbPool,
    base_url: String,
    web_dir: std::path::PathBuf,
}

pub async fn generate_sitemap(db_pool: DbPool) -> Result<()> {
    let ctx = load_sitemap_context(db_pool).await?;
    let urls = collect_sitemap_urls(&ctx).await?;
    write_sitemap_files(&ctx.web_dir, &urls, &ctx.base_url).await?;
    tracing::info!(url_count = urls.len(), "Sitemap generation completed");
    Ok(())
}

async fn load_sitemap_context(db_pool: DbPool) -> Result<SitemapContext> {
    let global_config = Config::get()?;
    let paths = AppPaths::get().map_err(|e| anyhow!("{}", e))?;
    let config_path = paths.system().content_config();

    let yaml_content = fs::read_to_string(&config_path)
        .await
        .context("Failed to read content config")?;

    let config: ContentConfigRaw =
        serde_yaml::from_str(&yaml_content).context("Failed to parse content config")?;

    let web_dir = AppPaths::get()
        .map_err(|e| anyhow!("{}", e))?
        .web()
        .dist()
        .to_path_buf();
    let base_url = global_config.api_external_url.clone();

    tracing::debug!(base_url = %base_url, "Using base URL");

    Ok(SitemapContext {
        config,
        db_pool,
        base_url,
        web_dir,
    })
}

async fn collect_sitemap_urls(ctx: &SitemapContext) -> Result<Vec<SitemapUrl>> {
    let mut all_urls = Vec::new();

    for (source_name, source) in &ctx.config.content_sources {
        let urls = collect_source_urls(ctx, source_name, source).await?;
        all_urls.extend(urls);
    }

    Ok(all_urls)
}

async fn collect_source_urls(
    ctx: &SitemapContext,
    source_name: &str,
    source: &ContentSourceConfigRaw,
) -> Result<Vec<SitemapUrl>> {
    let Some(sitemap_config) = get_enabled_sitemap_config(source) else {
        return Ok(Vec::new());
    };

    tracing::debug!(source = %source_name, "Processing source");

    let mut urls = fetch_urls_from_database(FetchParams {
        db_pool: &ctx.db_pool,
        source_id: source.source_id.as_str(),
        url_pattern: &sitemap_config.url_pattern,
        priority: sitemap_config.priority,
        changefreq: &sitemap_config.changefreq,
        base_url: &ctx.base_url,
    })
    .await
    .context(format!("Failed to fetch URLs for {source_name}"))?;

    urls.extend(build_parent_url(sitemap_config, &ctx.base_url));
    Ok(urls)
}

fn get_enabled_sitemap_config(
    source: &ContentSourceConfigRaw,
) -> Option<&systemprompt_models::SitemapConfig> {
    if !source.enabled {
        return None;
    }
    source.sitemap.as_ref().filter(|cfg| cfg.enabled)
}

fn build_parent_url(
    sitemap_config: &systemprompt_models::SitemapConfig,
    base_url: &str,
) -> Option<SitemapUrl> {
    let parent_config = sitemap_config.parent_route.as_ref()?;

    if !parent_config.enabled {
        return None;
    }

    Some(SitemapUrl {
        loc: format!("{}{}", base_url, parent_config.url),
        lastmod: Utc::now().format("%Y-%m-%d").to_string(),
        changefreq: parent_config.changefreq.clone(),
        priority: parent_config.priority,
    })
}

async fn write_sitemap_files(
    web_dir: &Path,
    all_urls: &[SitemapUrl],
    base_url: &str,
) -> Result<()> {
    let sitemap_chunks: Vec<Vec<_>> = all_urls
        .chunks(MAX_URLS_PER_SITEMAP)
        .map(<[_]>::to_vec)
        .collect();

    if sitemap_chunks.len() == 1 {
        write_single_sitemap(web_dir, &sitemap_chunks[0]).await
    } else {
        write_multiple_sitemaps(web_dir, &sitemap_chunks, base_url).await
    }
}

async fn write_single_sitemap(web_dir: &Path, urls: &[SitemapUrl]) -> Result<()> {
    let sitemap_xml = build_sitemap_xml(urls);
    let path = web_dir.join("sitemap.xml");
    fs::write(&path, sitemap_xml).await?;

    tracing::debug!(url_count = urls.len(), "Generated sitemap.xml");
    Ok(())
}

async fn write_multiple_sitemaps(
    web_dir: &Path,
    chunks: &[Vec<SitemapUrl>],
    base_url: &str,
) -> Result<()> {
    let sitemap_dir = web_dir.join("sitemaps");
    fs::create_dir_all(&sitemap_dir).await?;

    for (idx, chunk) in chunks.iter().enumerate() {
        write_numbered_sitemap(&sitemap_dir, idx, chunk).await?;
    }

    write_sitemap_index(web_dir, chunks, base_url).await
}

async fn write_numbered_sitemap(sitemap_dir: &Path, idx: usize, urls: &[SitemapUrl]) -> Result<()> {
    let filename = format!("sitemap-{}.xml", idx + 1);
    let sitemap_xml = build_sitemap_xml(urls);
    let path = sitemap_dir.join(&filename);
    fs::write(&path, sitemap_xml).await?;
    tracing::debug!(filename = %filename, url_count = urls.len(), "Generated sitemap file");
    Ok(())
}

async fn write_sitemap_index(
    web_dir: &Path,
    chunks: &[Vec<SitemapUrl>],
    base_url: &str,
) -> Result<()> {
    let index_xml = build_sitemap_index(chunks, base_url);
    let path = web_dir.join("sitemap.xml");
    fs::write(&path, index_xml).await?;
    tracing::debug!(file_count = chunks.len(), "Generated sitemap index");
    Ok(())
}

struct FetchParams<'a> {
    db_pool: &'a DbPool,
    source_id: &'a str,
    url_pattern: &'a str,
    priority: f32,
    changefreq: &'a str,
    base_url: &'a str,
}

async fn fetch_urls_from_database(params: FetchParams<'_>) -> Result<Vec<SitemapUrl>> {
    let repo = ContentRepository::new(params.db_pool)
        .map_err(|e| anyhow!("{}", e))
        .context("Failed to create content repository")?;

    let source_id = SourceId::new(params.source_id);
    let contents = repo
        .list_by_source(&source_id)
        .await
        .context("Failed to fetch content for sitemap")?;

    contents
        .iter()
        .map(|content| build_sitemap_url_from_content(content, params))
        .collect()
}

fn build_sitemap_url_from_content(
    content: &systemprompt_content::models::Content,
    params: &FetchParams<'_>,
) -> Result<SitemapUrl> {
    let relative_url = params.url_pattern.replace(SLUG_PLACEHOLDER, &content.slug);
    let absolute_url = format!("{}{}", params.base_url, relative_url);

    let lastmod = content.published_at.format("%Y-%m-%d").to_string();

    Ok(SitemapUrl {
        loc: absolute_url,
        lastmod,
        changefreq: params.changefreq.to_string(),
        priority: params.priority,
    })
}
