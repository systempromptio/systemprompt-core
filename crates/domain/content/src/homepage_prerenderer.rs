use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use systemprompt_cloud::constants::storage;
use systemprompt_models::ContentConfigRaw;
use systemprompt_provider_contracts::{
    PagePrepareContext, PagePrerenderer, PageRenderSpec, WebConfig,
};

const PAGE_TYPE: &str = "homepage";
const TEMPLATE_NAME: &str = "homepage";
const OUTPUT_FILE: &str = "index.html";

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultHomepagePrerenderer;

impl DefaultHomepagePrerenderer {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn extract_branding(
        web_config: &WebConfig,
        content_config: &ContentConfigRaw,
    ) -> HomepageBranding {
        let org = &content_config.metadata.structured_data.organization;
        let branding = &web_config.branding;

        HomepageBranding {
            org_name: org.name.clone(),
            org_url: org.url.clone(),
            org_logo: org.logo.clone(),
            logo_path: branding
                .logo
                .primary
                .svg
                .clone()
                .unwrap_or_else(String::new),
            favicon_path: branding.favicon.clone(),
            twitter_handle: branding.twitter_handle.clone(),
            display_sitename: branding.display_sitename,
        }
    }

    fn generate_footer_html(web_config: &WebConfig) -> String {
        let footer = &web_config.navigation.footer;
        if footer.legal.is_empty() && footer.resources.is_empty() {
            return String::new();
        }

        let mut html = String::from("<nav class=\"footer-nav\" aria-label=\"Footer navigation\">");

        if !footer.legal.is_empty() {
            html.push_str(
                "<div class=\"footer-section\"><h3 class=\"footer-section-title\">Legal</h3><ul>",
            );
            for link in &footer.legal {
                let target = if link.path.starts_with("http") {
                    " target=\"_blank\" rel=\"noopener noreferrer\""
                } else {
                    ""
                };
                html.push_str(&format!(
                    "<li><a href=\"{}\"{}>{}</a></li>",
                    link.path, target, link.label
                ));
            }
            html.push_str("</ul></div>");
        }

        if !footer.resources.is_empty() {
            html.push_str(
                "<div class=\"footer-section\"><h3 \
                 class=\"footer-section-title\">Resources</h3><ul>",
            );
            for link in &footer.resources {
                let target = if link.path.starts_with("http") {
                    " target=\"_blank\" rel=\"noopener noreferrer\""
                } else {
                    ""
                };
                html.push_str(&format!(
                    "<li><a href=\"{}\"{}>{}</a></li>",
                    link.path, target, link.label
                ));
            }
            html.push_str("</ul></div>");
        }

        html.push_str("</nav>");
        html
    }
}

struct HomepageBranding {
    org_name: String,
    org_url: String,
    org_logo: String,
    logo_path: String,
    favicon_path: String,
    twitter_handle: String,
    display_sitename: bool,
}

#[async_trait]
impl PagePrerenderer for DefaultHomepagePrerenderer {
    fn page_type(&self) -> &str {
        PAGE_TYPE
    }

    fn priority(&self) -> u32 {
        100
    }

    async fn prepare(&self, ctx: &PagePrepareContext<'_>) -> Result<Option<PageRenderSpec>> {
        let content_config = ctx
            .content_config::<ContentConfigRaw>()
            .ok_or_else(|| anyhow::anyhow!("ContentConfigRaw not available in context"))?;

        let branding = Self::extract_branding(ctx.web_config, content_config);
        let footer_html = Self::generate_footer_html(ctx.web_config);

        let base_data = serde_json::json!({
            "site": ctx.web_config,
            "nav": {
                "app_url": "/app",
                "blog_url": "/blog"
            },
            "ORG_NAME": branding.org_name,
            "ORG_URL": branding.org_url,
            "ORG_LOGO": branding.org_logo,
            "LOGO_PATH": branding.logo_path,
            "FAVICON_PATH": branding.favicon_path,
            "TWITTER_HANDLE": branding.twitter_handle,
            "DISPLAY_SITENAME": branding.display_sitename,
            "FOOTER_NAV": footer_html,
            "HEADER_CTA_URL": "/",
            "JS_BASE_PATH": format!("/{}", storage::JS),
            "CSS_BASE_PATH": format!("/{}", storage::CSS)
        });

        Ok(Some(PageRenderSpec::new(
            TEMPLATE_NAME,
            base_data,
            PathBuf::from(OUTPUT_FILE),
        )))
    }
}

pub fn default_homepage_prerenderer() -> Arc<dyn PagePrerenderer> {
    Arc::new(DefaultHomepagePrerenderer::new())
}
