//! Build `sitemap.xml` (and a sitemap index when the URL count exceeds the
//! 50 000 URL limit) from configured content sources.

use chrono::Utc;
use std::collections::HashMap;
use std::path::Path;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, ContentSourceConfigRaw, WebConfig};
use tokio::fs;

use super::xml::{SitemapUrl, SitemapUrlAlternate, build_sitemap_index, build_sitemap_xml};
use crate::error::{GeneratorResult as Result, PublishError};
use crate::templates::load_web_config;

const MAX_URLS_PER_SITEMAP: usize = 50_000;
const SLUG_PLACEHOLDER: &str = "{slug}";

struct SitemapContext {
    config: ContentConfigRaw,
    web_config: WebConfig,
    db_pool: DbPool,
    base_url: String,
    web_dir: std::path::PathBuf,
}

pub async fn generate_sitemap(db_pool: DbPool, paths: &AppPaths) -> Result<()> {
    let ctx = load_sitemap_context(db_pool, paths).await?;
    let urls = collect_sitemap_urls(&ctx).await?;
    write_sitemap_files(&ctx.web_dir, &urls, &ctx.base_url).await?;
    tracing::info!(url_count = urls.len(), "Sitemap generation completed");
    Ok(())
}

async fn load_sitemap_context(db_pool: DbPool, paths: &AppPaths) -> Result<SitemapContext> {
    let global_config = Config::get().map_err(PublishError::other)?;
    let config_path = paths.system().content_config();

    let yaml_content = fs::read_to_string(&config_path)
        .await
        .map_err(|e| PublishError::other(format!("Failed to read content config: {e}")))?;

    let config: ContentConfigRaw = serde_yaml::from_str(&yaml_content)
        .map_err(|e| PublishError::other(format!("Failed to parse content config: {e}")))?;

    let web_config = load_web_config(paths)
        .await
        .map_err(|e| PublishError::other(format!("Failed to load web config: {e}")))?;

    let web_dir = paths.web().dist().to_path_buf();
    let base_url = global_config.api_external_url.clone();

    tracing::debug!(base_url = %base_url, "Using base URL");

    Ok(SitemapContext {
        config,
        web_config,
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
        web_config: &ctx.web_config,
        source_id: source.source_id.as_str(),
        url_pattern: &sitemap_config.url_pattern,
        priority: sitemap_config.priority,
        changefreq: &sitemap_config.changefreq,
        base_url: &ctx.base_url,
    })
    .await
    .map_err(|e| PublishError::fetch_failed(source_name, e.to_string()))?;

    urls.extend(build_parent_urls(
        sitemap_config,
        &ctx.web_config,
        &ctx.base_url,
    ));
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

fn build_parent_urls(
    sitemap_config: &systemprompt_models::SitemapConfig,
    web_config: &WebConfig,
    base_url: &str,
) -> Vec<SitemapUrl> {
    let Some(parent_config) = sitemap_config.parent_route.as_ref() else {
        return Vec::new();
    };

    if !parent_config.enabled {
        return Vec::new();
    }

    let lastmod = Utc::now().format("%Y-%m-%d").to_string();
    let i18n = &web_config.i18n;

    i18n.supported_locales
        .iter()
        .map(|locale| {
            let prefix = i18n.locale_prefix(locale);
            let alternates = i18n
                .supported_locales
                .iter()
                .map(|alt| SitemapUrlAlternate {
                    hreflang: alt.to_string(),
                    href: format!(
                        "{}{}{}",
                        base_url,
                        i18n.locale_prefix(alt),
                        parent_config.url
                    ),
                })
                .chain(std::iter::once(SitemapUrlAlternate {
                    hreflang: "x-default".to_string(),
                    href: format!("{}{}", base_url, parent_config.url),
                }))
                .collect();
            SitemapUrl {
                loc: format!("{}{}{}", base_url, prefix, parent_config.url),
                lastmod: lastmod.clone(),
                changefreq: parent_config.changefreq.clone(),
                priority: parent_config.priority,
                alternates,
            }
        })
        .collect()
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

    match sitemap_chunks.as_slice() {
        [] => write_single_sitemap(web_dir, &[]).await,
        [single] => write_single_sitemap(web_dir, single).await,
        _ => write_multiple_sitemaps(web_dir, &sitemap_chunks, base_url).await,
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
    web_config: &'a WebConfig,
    source_id: &'a str,
    url_pattern: &'a str,
    priority: f32,
    changefreq: &'a str,
    base_url: &'a str,
}

async fn fetch_urls_from_database(params: FetchParams<'_>) -> Result<Vec<SitemapUrl>> {
    let repo = ContentRepository::new(params.db_pool)
        .map_err(|e| PublishError::other(format!("Failed to create content repository: {e}")))?;

    let source_id = SourceId::new(params.source_id);
    let pairs = repo
        .list_slugs_with_locales_by_source(&source_id)
        .await
        .map_err(|e| PublishError::other(format!("Failed to fetch content for sitemap: {e}")))?;

    let mut by_slug: HashMap<String, Vec<LocaleCode>> = HashMap::new();
    for (slug, locale) in pairs {
        by_slug.entry(slug).or_default().push(locale);
    }

    let i18n = &params.web_config.i18n;
    let lastmod = Utc::now().format("%Y-%m-%d").to_string();

    let mut urls = Vec::new();
    for (slug, locales) in by_slug {
        let relative = params.url_pattern.replace(SLUG_PLACEHOLDER, &slug);
        let default_url = format!("{}{}", params.base_url, relative);

        for locale in &locales {
            let prefix = i18n.locale_prefix(locale);
            let alternates = locales
                .iter()
                .filter(|alt| i18n.supported_locales.contains(alt))
                .map(|alt| SitemapUrlAlternate {
                    hreflang: alt.to_string(),
                    href: format!("{}{}{}", params.base_url, i18n.locale_prefix(alt), relative),
                })
                .chain(std::iter::once(SitemapUrlAlternate {
                    hreflang: "x-default".to_string(),
                    href: default_url.clone(),
                }))
                .collect();

            urls.push(SitemapUrl {
                loc: format!("{}{}{}", params.base_url, prefix, relative),
                lastmod: lastmod.clone(),
                changefreq: params.changefreq.to_string(),
                priority: params.priority,
                alternates,
            });
        }
    }

    Ok(urls)
}
