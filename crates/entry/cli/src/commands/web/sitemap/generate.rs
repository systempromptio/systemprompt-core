use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use std::fs;
use std::path::PathBuf;

use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_core_logging::CliService;
use systemprompt_models::content_config::ContentConfigRaw;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::SitemapGenerateOutput;

#[derive(Debug, Args)]
pub struct GenerateArgs {
    #[arg(long, help = "Output path (default: {web_path}/dist/sitemap.xml)")]
    pub output: Option<PathBuf>,

    #[arg(long, help = "Base URL for sitemap (e.g., https://example.com)")]
    pub base_url: Option<String>,

    #[arg(long, help = "Include dynamic content from database")]
    pub include_dynamic: bool,
}

pub fn execute(
    args: GenerateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<SitemapGenerateOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let content_config_path = profile.paths.content_config();

    let content = fs::read_to_string(&content_config_path)
        .with_context(|| format!("Failed to read content config at {}", content_config_path))?;

    let content_config: ContentConfigRaw = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse content config at {}", content_config_path))?;

    let base_url = args.base_url.clone().unwrap_or_else(|| {
        let metadata_path = profile.paths.web_metadata();
        fs::read_to_string(&metadata_path)
            .ok()
            .and_then(|content| extract_base_url(&content))
            .unwrap_or_else(|| "https://example.com".to_string())
    });

    let web_path = profile.paths.web_path_resolved();
    let output_path = args.output.clone().unwrap_or_else(|| {
        PathBuf::from(&web_path).join("dist").join("sitemap.xml")
    });

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    CliService::info("Generating sitemap...");

    let mut urls: Vec<SitemapUrl> = Vec::new();
    let today = Utc::now().format("%Y-%m-%d").to_string();

    for (name, source) in &content_config.content_sources {
        if !source.enabled {
            continue;
        }

        if let Some(sitemap) = &source.sitemap {
            if !sitemap.enabled {
                continue;
            }

            if let Some(parent) = &sitemap.parent_route {
                if parent.enabled {
                    urls.push(SitemapUrl {
                        loc: format!("{}{}", base_url, parent.url),
                        lastmod: today.clone(),
                        changefreq: parent.changefreq.clone(),
                        priority: parent.priority,
                    });
                }
            }

            if !args.include_dynamic && sitemap.url_pattern.contains("{slug}") {
                CliService::warning(&format!(
                    "Skipping dynamic route '{}' for source '{}'. Use --include-dynamic to fetch \
                     from database.",
                    sitemap.url_pattern, name
                ));
            } else if !sitemap.url_pattern.contains("{slug}") {
                urls.push(SitemapUrl {
                    loc: format!("{}{}", base_url, sitemap.url_pattern),
                    lastmod: today.clone(),
                    changefreq: sitemap.changefreq.clone(),
                    priority: sitemap.priority,
                });
            }
        }
    }

    urls.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));

    let xml = generate_sitemap_xml(&urls);

    fs::write(&output_path, &xml)
        .with_context(|| format!("Failed to write sitemap to {}", output_path.display()))?;

    CliService::success(&format!(
        "Sitemap generated with {} URLs at {}",
        urls.len(),
        output_path.display()
    ));

    let output = SitemapGenerateOutput {
        output_path: output_path.to_string_lossy().to_string(),
        routes_count: urls.len(),
        message: format!(
            "Sitemap generated with {} URLs at {}",
            urls.len(),
            output_path.display()
        ),
    };

    Ok(CommandResult::text(output).with_title("Sitemap Generated"))
}

struct SitemapUrl {
    loc: String,
    lastmod: String,
    changefreq: String,
    priority: f32,
}

fn generate_sitemap_xml(urls: &[SitemapUrl]) -> String {
    let mut xml = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n"
    ));

    for url in urls {
        xml.push_str(&format!(
            "  <url>\n    <loc>{}</loc>\n    <lastmod>{}</lastmod>\n    \
             <changefreq>{}</changefreq>\n    <priority>{:.1}</priority>\n  </url>\n",
            escape_xml(&url.loc),
            url.lastmod,
            url.changefreq,
            url.priority
        ));
    }

    xml.push_str("</urlset>\n");
    xml
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn extract_base_url(metadata_content: &str) -> Option<String> {
    for line in metadata_content.lines() {
        let line = line.trim();
        if line.starts_with("baseUrl:") {
            let url = line.trim_start_matches("baseUrl:").trim();
            let url = url.trim_matches('"').trim_matches('\'');
            return Some(url.to_string());
        }
    }
    None
}
