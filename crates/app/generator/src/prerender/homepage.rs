use anyhow::Result;
use systemprompt_cloud::constants::storage;
use systemprompt_template_provider::{ComponentContext, PageContext};
use tokio::fs;

use crate::error::PublishError;
use crate::prerender::context::{extract_homepage_branding, PrerenderContext};
use crate::templates::navigation::generate_footer_html;

pub async fn prerender_homepage(ctx: &PrerenderContext) -> Result<()> {
    if !ctx.template_registry.has_template("homepage") {
        tracing::info!("No homepage template found, skipping homepage prerender");
        return Ok(());
    }

    let branding = extract_homepage_branding(&ctx.web_config, &ctx.config);
    let footer_html = generate_footer_html(&ctx.web_config)?;

    let mut homepage_data = serde_json::json!({
        "site": &ctx.web_config,
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

    let page_ctx = PageContext::new("homepage", &ctx.web_config, &ctx.db_pool);

    let providers = ctx.template_registry.page_providers_for("homepage");
    let provider_ids: Vec<_> = providers.iter().map(|p| p.provider_id()).collect();
    tracing::info!(
        provider_count = providers.len(),
        provider_ids = ?provider_ids,
        "Homepage page data providers"
    );

    for provider in &providers {
        let data = provider
            .provide_page_data(&page_ctx)
            .await
            .map_err(|e| PublishError::provider_failed(provider.provider_id(), e.to_string()))?;
        merge_json_data(&mut homepage_data, &data);
    }

    let component_ctx = ComponentContext::for_page(&ctx.web_config);

    for component in ctx.template_registry.components_for("homepage") {
        let rendered = component
            .render(&component_ctx)
            .await
            .map_err(|e| PublishError::provider_failed(component.component_id(), e.to_string()))?;

        if let Some(obj) = homepage_data.as_object_mut() {
            obj.insert(
                rendered.variable_name,
                serde_json::Value::String(rendered.html),
            );
        }
    }

    let html = ctx
        .template_registry
        .render("homepage", &homepage_data)
        .map_err(|e| PublishError::render_failed("homepage", None, e.to_string()))?;

    let output_path = ctx.dist_dir.join("index.html");
    fs::write(&output_path, html).await?;

    tracing::info!(path = %output_path.display(), "Generated homepage");
    Ok(())
}

fn merge_json_data(base: &mut serde_json::Value, extension: &serde_json::Value) {
    match (base, extension) {
        (serde_json::Value::Object(base_obj), serde_json::Value::Object(ext_obj)) => {
            for (key, ext_value) in ext_obj {
                match base_obj.get_mut(key) {
                    Some(base_value) => merge_json_data(base_value, ext_value),
                    None => {
                        base_obj.insert(key.clone(), ext_value.clone());
                    },
                }
            }
        },
        (base, extension) => {
            *base = extension.clone();
        },
    }
}
