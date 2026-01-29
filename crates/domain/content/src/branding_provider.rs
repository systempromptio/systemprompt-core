use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_cloud::constants::storage;
use systemprompt_models::ContentConfigRaw;
use systemprompt_provider_contracts::{PageContext, PageDataProvider};

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultBrandingProvider;

#[async_trait]
impl PageDataProvider for DefaultBrandingProvider {
    fn provider_id(&self) -> &str {
        "default-branding"
    }

    async fn provide_page_data(&self, ctx: &PageContext<'_>) -> Result<Value> {
        let content_config = ctx
            .content_config::<ContentConfigRaw>()
            .ok_or_else(|| anyhow::anyhow!("ContentConfigRaw not available in PageContext"))?;

        let org = &content_config.metadata.structured_data.organization;
        let branding = &ctx.web_config.branding;

        Ok(serde_json::json!({
            "ORG_NAME": org.name,
            "ORG_URL": org.url,
            "ORG_LOGO": org.logo,
            "LOGO_PATH": branding.logo.primary.svg.as_deref().unwrap_or(""),
            "FAVICON_PATH": &branding.favicon,
            "TWITTER_HANDLE": &branding.twitter_handle,
            "DISPLAY_SITENAME": branding.display_sitename,
            "CSS_BASE_PATH": format!("/{}", storage::CSS),
            "JS_BASE_PATH": format!("/{}", storage::JS),
            "HEADER_CTA_URL": "/",
        }))
    }
}

pub fn default_branding_provider() -> Arc<dyn PageDataProvider> {
    Arc::new(DefaultBrandingProvider)
}
