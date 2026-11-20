//! The marketplace route group on the real router, mounted from the
//! `marketplace-http` extension's declaration:
//!
//! | route | anonymous | `marketplace-http` disabled |
//! |---|---|---|
//! | `/api/v1/marketplace/marketplaces` | served | 404 |
//! | `/api/v1/marketplace/plugins/{plugin_id}/{*path}` | served from the bundle | 404 |
//! | `/api/v1/marketplace/marketplaces/{id}/manifest.yaml` outside the root | 403 | 404 |
//!
//! Only the `All` and `Api` roles serve the group, and a profile that
//! disables `managed_resources` while `marketplace-http` stays enabled is
//! refused at boot.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_marketplace_entry::extension::MARKETPLACE_HTTP_EXTENSION;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
};
use tower::ServiceExt;

const LIST: &str = "/api/v1/marketplace/marketplaces";
const PLUGIN_FILE: &str = "/api/v1/marketplace/plugins/cov-plugin/skills/covskill/SKILL.md";

async fn node(role: NodeRole, disabled: &[ExtensionId]) -> Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = role;
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx =
        fixture_app_context_with_extensions(config, super::common::reference_admitted(disabled)?)
            .await?;
    setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

async fn get(app: &Router, uri: &str) -> Result<(StatusCode, String)> {
    let mut req = Request::builder()
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let response = app.clone().oneshot(req).await?;
    let status = response.status();
    let body = response.into_body().collect().await?.to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

#[tokio::test]
async fn the_marketplace_group_serves_an_anonymous_caller() -> Result<()> {
    let app = node(NodeRole::All, &[]).await?;
    assert_eq!(get(&app, LIST).await?.0, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn a_plugin_file_is_served_from_the_bundle_and_a_missing_file_is_404() -> Result<()> {
    super::routes_bridge_plugin_file::seed_plugin_tree();
    let app = node(NodeRole::All, &[]).await?;
    let (status, body) = get(&app, PLUGIN_FILE).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Cov skill instructions."), "{body}");

    let (status, body) = get(
        &app,
        "/api/v1/marketplace/plugins/cov-plugin/skills/covskill/DOES_NOT_EXIST.md",
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!body.contains("Cov skill instructions."), "{body}");
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn a_manifest_outside_the_marketplaces_root_is_forbidden() -> Result<()> {
    let bootstrap = ensure_test_bootstrap();
    let id = format!("mounted-{}", uuid::Uuid::new_v4().simple());
    let root = bootstrap.services_path.join("marketplaces");
    std::fs::create_dir_all(&root)?;
    let outside = std::env::temp_dir().join(format!("{id}-outside"));
    std::fs::create_dir_all(&outside)?;
    std::fs::write(outside.join("config.yaml"), b"name: stolen\n")?;
    std::os::unix::fs::symlink(&outside, root.join(&id))?;

    let app = node(NodeRole::All, &[]).await?;
    let (status, body) = get(&app, &format!("{LIST}/{id}/manifest.yaml")).await?;
    let _ = std::fs::remove_file(root.join(&id));
    let _ = std::fs::remove_dir_all(&outside);
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!body.contains("stolen"), "{body}");
    Ok(())
}

#[tokio::test]
async fn disabling_marketplace_http_unmounts_the_group() -> Result<()> {
    super::routes_bridge_plugin_file::seed_plugin_tree();
    let app = node(
        NodeRole::All,
        &[ExtensionId::new(MARKETPLACE_HTTP_EXTENSION)],
    )
    .await?;
    assert_eq!(get(&app, LIST).await?.0, StatusCode::NOT_FOUND);
    assert_eq!(get(&app, PLUGIN_FILE).await?.0, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn disabling_managed_resources_while_its_routes_stay_enabled_is_refused_at_boot() -> Result<()>
{
    let refused = node(NodeRole::All, &[ExtensionId::new("managed_resources")]).await;
    let message = match refused {
        Ok(_) => anyhow::bail!("boot accepted marketplace-http without managed_resources"),
        Err(error) => format!("{error:#}"),
    };
    assert!(message.contains("managed_resources"), "{message}");
    Ok(())
}

#[tokio::test]
async fn only_the_all_and_api_roles_serve_the_marketplace_group() -> Result<()> {
    let api = node(NodeRole::Api, &[]).await?;
    assert_eq!(get(&api, LIST).await?.0, StatusCode::OK);
    for role in [NodeRole::Worker, NodeRole::Gateway, NodeRole::Runner] {
        let app = node(role, &[]).await?;
        assert_eq!(get(&app, LIST).await?.0, StatusCode::NOT_FOUND, "{role:?}");
    }
    Ok(())
}
