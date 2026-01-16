use anyhow::{Context, Result};
use systemprompt_cloud::constants::storage;
use systemprompt_template_provider::{ComponentContext, PageContext};
use tokio::fs;

use crate::prerender::context::{extract_homepage_branding, PrerenderContext};
use crate::templates::navigation::generate_footer_html;

pub async fn prerender_homepage(ctx: &PrerenderContext) -> Result<()> {
    if !ctx.template_registry.has_template("homepage") {
        tracing::info!("No homepage template found, skipping homepage prerender");
        return Ok(());
    }

    let branding = extract_homepage_branding(&ctx.web_config, &ctx.config);
    let footer_html = generate_footer_html(&ctx.web_config).unwrap_or_default();

    let mut homepage_data = serde_json::json!({
        "site": &ctx.web_config,
        "nav": {
            "agent_url": "/agent",
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

    for provider in ctx.template_registry.page_providers_for("homepage") {
        match provider.provide_page_data(&page_ctx).await {
            Ok(data) => merge_json_data(&mut homepage_data, &data),
            Err(e) => {
                tracing::warn!(
                    provider_id = %provider.provider_id(),
                    error = %e,
                    "Page data provider failed"
                );
            },
        }
    }

    let component_ctx = ComponentContext::for_page(&ctx.web_config);

    for component in ctx.template_registry.components_for("homepage") {
        match component.render(&component_ctx).await {
            Ok(rendered) => {
                if let Some(obj) = homepage_data.as_object_mut() {
                    obj.insert(
                        rendered.variable_name,
                        serde_json::Value::String(rendered.html),
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    component_id = %component.component_id(),
                    error = %e,
                    "Homepage component render failed"
                );
            },
        }
    }

    let html = ctx
        .template_registry
        .render("homepage", &homepage_data)
        .context("Failed to render homepage template")?;

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
                    }
                }
            }
        }
        (base, extension) => {
            *base = extension.clone();
        }
    }
}
