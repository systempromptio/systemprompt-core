//! Sitemap URLs for one content source, one entry per slug and locale with
//! `hreflang` alternates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use std::collections::HashMap;
use systemprompt_content::ContentRepository;
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_manifest::WebConfig;

use super::xml::{SitemapUrl, SitemapUrlAlternate};
use crate::error::{GeneratorResult as Result, PublishError};

const SLUG_PLACEHOLDER: &str = "{slug}";

pub(super) struct FetchParams<'a> {
    pub(super) content_repo: &'a ContentRepository,
    pub(super) web_config: &'a WebConfig,
    pub(super) source_id: &'a SourceId,
    pub(super) url_pattern: &'a str,
    pub(super) priority: f32,
    pub(super) changefreq: &'a str,
    pub(super) base_url: &'a str,
}

pub(super) async fn fetch_urls_from_database(params: FetchParams<'_>) -> Result<Vec<SitemapUrl>> {
    let repo = params.content_repo;

    let pairs = repo
        .list_slugs_with_locales_by_source(params.source_id)
        .await
        .map_err(|e| PublishError::content("Failed to fetch content for sitemap", e))?;

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
                    hreflang: "x-default".to_owned(),
                    href: default_url.clone(),
                }))
                .collect();

            urls.push(SitemapUrl {
                loc: format!("{}{}{}", params.base_url, prefix, relative),
                lastmod: lastmod.clone(),
                changefreq: params.changefreq.to_owned(),
                priority: params.priority,
                alternates,
            });
        }
    }

    Ok(urls)
}
