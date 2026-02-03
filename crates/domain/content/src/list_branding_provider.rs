use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_models::ContentConfigRaw;
use systemprompt_provider_contracts::{PageContext, PageDataProvider};

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultListBrandingProvider;

#[async_trait]
impl PageDataProvider for DefaultListBrandingProvider {
    fn provider_id(&self) -> &'static str {
        "default-list-branding"
    }

    async fn provide_page_data(&self, ctx: &PageContext<'_>) -> Result<Value> {
        let Some(source_name) = ctx.page_type.strip_suffix("-list") else {
            return Ok(serde_json::json!({}));
        };

        let content_config = ctx
            .content_config::<ContentConfigRaw>()
            .ok_or_else(|| anyhow::anyhow!("ContentConfigRaw not available in PageContext"))?;

        let source = content_config.content_sources.get(source_name);
        let org = &content_config.metadata.structured_data.organization;
        let language = &content_config.metadata.language;
        let branding = &ctx.web_config.branding;

        let source_branding = source.and_then(|s| s.branding.as_ref());

        let blog_name = source_branding
            .and_then(|b| b.name.as_deref())
            .unwrap_or(&branding.name);

        let blog_description = source_branding
            .and_then(|b| b.description.as_deref())
            .unwrap_or(&branding.description);

        let blog_image = source_branding
            .and_then(|b| b.image.as_deref())
            .map_or_else(String::new, |img| format!("{}{}", org.url, img));

        let blog_keywords = source_branding
            .and_then(|b| b.keywords.as_deref())
            .unwrap_or("");

        Ok(serde_json::json!({
            "BLOG_NAME": blog_name,
            "BLOG_DESCRIPTION": blog_description,
            "BLOG_IMAGE": blog_image,
            "BLOG_KEYWORDS": blog_keywords,
            "BLOG_URL": format!("{}/{}", org.url, source_name),
            "BLOG_LANGUAGE": language,
        }))
    }
}

pub fn default_list_branding_provider() -> Arc<dyn PageDataProvider> {
    Arc::new(DefaultListBrandingProvider)
}
