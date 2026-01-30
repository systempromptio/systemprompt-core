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

        let base_data = serde_json::json!({
            "site": ctx.web_config,
            "ORG_NAME": branding.org_name,
            "ORG_URL": branding.org_url,
            "ORG_LOGO": branding.org_logo,
            "LOGO_PATH": branding.logo_path,
            "FAVICON_PATH": branding.favicon_path,
            "TWITTER_HANDLE": branding.twitter_handle,
            "DISPLAY_SITENAME": branding.display_sitename,
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
