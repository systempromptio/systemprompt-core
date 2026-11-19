//! Server discovery + health surfaces — `handle_root_discovery`,
//! `handle_core_discovery`, `handle_agents_discovery`, `handle_mcp_discovery`,
//! and `handle_health`. These are the unauthenticated discovery routes mounted
//! by `discovery_router`; we mount the handlers directly (the full
//! `discovery_router` additionally wants a process-global Prometheus handle)
//! and assert each returns a JSON body with the expected shape.

use axum::Router;
use axum::routing::get;
use systemprompt_api::services::server::builder::{
    handle_agents_discovery, handle_core_discovery, handle_health, handle_mcp_discovery,
    handle_root_discovery,
};
use systemprompt_files::{DocumentsConfig, RemoteOrigin, RenderedHtmlConfig, RenderedHtmlTier};
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_documents_configured,
    fixture_app_context_documents_disabled, fixture_config, test_db_pool,
};
use tower::ServiceExt;

use super::common::{body_to_string, empty_get, setup_ctx};

async fn discovery_app() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    Ok(discovery_router_over(&ctx))
}

fn discovery_router_over(ctx: &systemprompt_runtime::AppContext) -> Router {
    Router::new()
        .route(ApiPaths::DISCOVERY, get(handle_root_discovery))
        .route(ApiPaths::HEALTH, get(handle_health))
        .route("/health", get(handle_health))
        .route(ApiPaths::CORE_BASE, get(handle_core_discovery))
        .route(ApiPaths::AGENTS_BASE, get(handle_agents_discovery))
        .route(ApiPaths::MCP_BASE, get(handle_mcp_discovery))
        .layer(axum::Extension(std::sync::Arc::new(
            systemprompt_api::services::server::SchedulerHealth::new(),
        )))
        .with_state(ctx.clone())
}

async fn root_documents_block(app: Router) -> anyhow::Result<serde_json::Value> {
    let resp = app.oneshot(empty_get(ApiPaths::DISCOVERY)).await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status} {body}");
    let v: serde_json::Value = serde_json::from_str(&body)?;
    let data = v.get("data").unwrap_or(&v);
    Ok(data["endpoints"]["documents"].clone())
}

#[tokio::test]
async fn root_discovery_advertises_the_enabled_document_store() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let config = ctx.files_config().documents_config().clone();
    let base = ctx.config().api_external_url.clone();
    let documents = root_documents_block(discovery_router_over(&ctx)).await?;
    assert_eq!(
        documents,
        serde_json::json!({
            "enabled": true,
            "href": format!("{base}{}", ApiPaths::DOCUMENTS),
            "folders": format!("{base}{}", ApiPaths::DOCUMENT_FOLDERS),
            "uploads": format!("{base}{}", ApiPaths::UPLOADS),
            "limits": {
                "maxUploadBytes": config.max_upload_bytes,
                "inlineMaxBytes": config.uploads.inline_max_bytes,
                "uploadPartBytes": config.uploads.part_size_bytes,
            },
            "renderedHtml": {
                "scripts": false,
                "remoteOrigins": [],
                "agentAuthored": { "scripts": false, "remoteOrigins": [] },
            },
        })
    );
    Ok(())
}

#[tokio::test]
async fn root_discovery_publishes_the_rendered_html_tiers() -> anyhow::Result<()> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let cdn = RemoteOrigin::try_new("https://cdn.example.com")?;
    let documents = DocumentsConfig {
        enabled: true,
        rendered_html: RenderedHtmlConfig::new(
            RenderedHtmlTier {
                scripts: true,
                remote_origins: vec![cdn.clone()],
            },
            RenderedHtmlTier {
                scripts: false,
                remote_origins: vec![cdn],
            },
        )?,
        ..DocumentsConfig::default()
    };
    let ctx = fixture_app_context_documents_configured(
        &pool,
        fixture_config(&bootstrap.database_url),
        documents,
    )?;
    let block = root_documents_block(discovery_router_over(&ctx)).await?;
    assert_eq!(
        block["renderedHtml"],
        serde_json::json!({
            "scripts": true,
            "remoteOrigins": ["https://cdn.example.com"],
            "agentAuthored": { "scripts": false, "remoteOrigins": ["https://cdn.example.com"] },
        })
    );
    Ok(())
}

#[tokio::test]
async fn root_discovery_marks_a_disabled_document_store_without_routes() -> anyhow::Result<()> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx =
        fixture_app_context_documents_disabled(&pool, fixture_config(&bootstrap.database_url))?;
    let documents = root_documents_block(discovery_router_over(&ctx)).await?;
    assert_eq!(documents, serde_json::json!({ "enabled": false }));
    Ok(())
}

#[tokio::test]
async fn health_returns_status_field() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get("/health")).await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.as_u16() == 200 || status.as_u16() == 503, "{status}");
    let v: serde_json::Value = serde_json::from_str(&body)?;
    assert!(
        v.get("status").and_then(|s| s.as_str()).is_some(),
        "health body must carry a status field: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn root_discovery_returns_endpoints() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get(ApiPaths::DISCOVERY)).await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status} {body}");
    let v: serde_json::Value = serde_json::from_str(&body)?;
    let data = v.get("data").unwrap_or(&v);
    assert!(
        data.get("endpoints").is_some(),
        "root discovery missing endpoints: {body}"
    );
    assert!(
        data.get("version").is_some(),
        "root discovery missing version: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn core_discovery_runs() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get(ApiPaths::CORE_BASE)).await?;
    let status = resp.status();
    assert!(status.is_success(), "{status}");
    Ok(())
}

#[tokio::test]
async fn agents_discovery_runs() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get(ApiPaths::AGENTS_BASE)).await?;
    let status = resp.status();
    assert!(status.is_success(), "{status}");
    Ok(())
}

#[tokio::test]
async fn mcp_discovery_runs() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get(ApiPaths::MCP_BASE)).await?;
    let status = resp.status();
    assert!(status.is_success(), "{status}");
    Ok(())
}
