//! `GET /bridge/manifest` serves the services config the instance loaded. A
//! tree corrupted on disk after boot is refused on reload and the route keeps
//! serving the last good config: the response is unchanged.

use axum::body::{Body, to_bytes};
use axum::http::{Request, header};
use http::StatusCode;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_loader::ConfigLoader;
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_test_fixtures::{seed_admin_credential, test_db_pool};
use systemprompt_traits::{CacheDomain, ConfigVersionSource, RepositoryError, WatchSignals};
use tower::ServiceExt;

#[derive(Debug)]
struct FixedVersion;

#[async_trait::async_trait]
impl ConfigVersionSource for FixedVersion {
    async fn current_version(&self) -> Result<i64, RepositoryError> {
        Ok(0)
    }
}

async fn fetch_manifest(
    app: axum::Router,
    jwt: &str,
) -> anyhow::Result<(StatusCode, serde_json::Value)> {
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/bridge/manifest")
                .header(header::AUTHORIZATION, format!("Bearer {jwt}"))
                .body(Body::empty())?,
        )
        .await?;
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await?;
    Ok((status, serde_json::from_slice(&bytes).unwrap_or_default()))
}

fn stable_payload(envelope: &serde_json::Value) -> serde_json::Value {
    let mut payload: serde_json::Value =
        serde_json::from_str(envelope["payload"].as_str().expect("signed payload"))
            .expect("payload is json");
    if let Some(fields) = payload.as_object_mut() {
        for volatile in ["issued_at", "not_before", "manifest_version"] {
            fields.remove(volatile);
        }
    }
    payload
}

#[tokio::test]
async fn a_services_file_corrupted_after_boot_is_refused_and_the_route_keeps_serving()
-> anyhow::Result<()> {
    let b = systemprompt_test_fixtures::bootstrap::init_services_bootstrap(
        "agents: {}\nmcp_servers: {}\n",
    );
    let pool = test_db_pool().await;
    let paths = PathsConfig {
        system: b.system_path.to_string_lossy().into_owned(),
        services: b.services_path.to_string_lossy().into_owned(),
        bin: b.bin_path.to_string_lossy().into_owned(),
        web_path: None,
        storage: Some(b.storage_path.to_string_lossy().into_owned()),
        geoip_database: None,
        shipped_files: None,
        data: None,
    };
    let ctx = systemprompt_test_fixtures::fixture_app_context_with(
        &pool,
        &b.database_url,
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )?;
    let app = gateway_router(&ctx).expect("gateway router builds");
    let cred = seed_admin_credential(&pool, "manifest-badcfg@example.invalid").await?;

    let (before_status, before) = fetch_manifest(app.clone(), cred.jwt.as_str()).await?;
    assert_eq!(before_status, StatusCode::OK, "{before}");

    let root = b.services_path.join("config/config.yaml");
    std::fs::write(&root, "mcp_servers: [this is not a map\n")?;
    assert!(
        ConfigLoader::load_from_path(&root).is_err(),
        "the corrupted tree must not load"
    );
    ctx.services_watch().spawn(
        ctx.background_tasks(),
        WatchSignals {
            domain: CacheDomain::Services,
            epochs: ctx.cache_epochs().clone(),
            version: Arc::new(FixedVersion),
            poll: Duration::from_secs(3600),
        },
        {
            let root = root.clone();
            move || {
                let root = root.clone();
                async move { ConfigLoader::load_from_path(&root) }
            }
        },
    );
    for _ in 0..20 {
        ctx.cache_epochs().advance(CacheDomain::Services);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        ctx.services_watch().reloads(),
        0,
        "a failed load never swaps the value"
    );

    let (after_status, after) = fetch_manifest(app, cred.jwt.as_str()).await?;
    assert_eq!(after_status, StatusCode::OK, "{after}");
    assert_eq!(stable_payload(&before), stable_payload(&after));
    Ok(())
}
