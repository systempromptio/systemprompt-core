use anyhow::{Context, Result};
use clap::Args;
use std::fs;

use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_core_logging::CliService;
use systemprompt_models::content_config::ContentConfigRaw;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::super::types::{SitemapRoute, SitemapShowOutput};

#[derive(Debug, Clone, Copy, Args)]
pub struct ShowArgs {
    #[arg(long, help = "Show XML preview")]
    pub preview: bool,
}

pub fn execute(args: ShowArgs, _config: &CliConfig) -> Result<CommandResult<SitemapShowOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let content_config_path = profile.paths.content_config();

    let content = fs::read_to_string(&content_config_path)
        .with_context(|| format!("Failed to read content config at {}", content_config_path))?;

    let config: ContentConfigRaw = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse content config at {}", content_config_path))?;

    let mut routes: Vec<SitemapRoute> = Vec::new();

    for (name, source) in &config.content_sources {
        if !source.enabled {
            continue;
        }

        if let Some(sitemap) = &source.sitemap {
            if !sitemap.enabled {
                continue;
            }

            if let Some(parent) = &sitemap.parent_route {
                if parent.enabled {
                    routes.push(SitemapRoute {
                        url: parent.url.clone(),
                        priority: parent.priority,
                        changefreq: parent.changefreq.clone(),
                        source: format!("{} (parent)", name),
                    });
                }
            }

            routes.push(SitemapRoute {
                url: sitemap.url_pattern.clone(),
                priority: sitemap.priority,
                changefreq: sitemap.changefreq.clone(),
                source: name.clone(),
            });
        }
    }

    routes.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_routes = routes.len();

    if args.preview {
        let xml = generate_xml_preview(&routes);
        CliService::output(&xml);
    }

    let output = SitemapShowOutput {
        routes,
        total_routes,
    };

    Ok(CommandResult::table(output)
        .with_title("Sitemap Configuration")
        .with_columns(vec![
            "url".to_string(),
            "priority".to_string(),
            "changefreq".to_string(),
            "source".to_string(),
        ]))
}

fn generate_xml_preview(routes: &[SitemapRoute]) -> String {
    let mut xml = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n"
    ));

    for route in routes {
        xml.push_str(&format!(
            "  <url>\n    <loc>{}</loc>\n    <priority>{:.1}</priority>\n    \
             <changefreq>{}</changefreq>\n  </url>\n",
            route.url, route.priority, route.changefreq
        ));
    }

    xml.push_str("</urlset>\n");
    xml
}
