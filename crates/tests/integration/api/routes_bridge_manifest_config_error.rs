//! `GET /bridge/manifest` when the services config on disk is malformed — the
//! candidate assembly must fail closed with a 500 naming the services-config
//! load, not serve an unsigned or partial manifest.

use axum::body::{Body, to_bytes};
use axum::http::{Request, header};
use http::StatusCode;
use std::sync::Arc;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{install_test_signing_key, seed_admin_credential};
use tower::ServiceExt;

#[tokio::test]
async fn malformed_services_config_fails_manifest_with_500() -> anyhow::Result<()> {
    let b = systemprompt_test_fixtures::bootstrap::init_isolated_bootstrap(
        "http://127.0.0.1",
        "mcp_servers: [this is not a map\n",
    );
    let pool = systemprompt_test_fixtures::fixture_db_pool(&b.database_url).await?;
    let paths = PathsConfig {
        system: b.system_path.to_string_lossy().into_owned(),
        services: b.services_path.to_string_lossy().into_owned(),
        bin: b.bin_path.to_string_lossy().into_owned(),
        web_path: None,
        storage: Some(b.storage_path.to_string_lossy().into_owned()),
        geoip_database: None,
    };
    let ctx = systemprompt_test_fixtures::fixture_app_context_with(
        &pool,
        &b.database_url,
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )?;
    install_test_signing_key();
    let app = gateway_router(&ctx).expect("gateway router");
    let cred = seed_admin_credential(&pool, "manifest-badcfg@example.invalid").await?;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/bridge/manifest")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", cred.jwt.as_str()),
                )
                .body(Body::empty())?,
        )
        .await?;
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await?;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(body.contains("services"), "{body}");
    Ok(())
}
