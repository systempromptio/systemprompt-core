//! Readiness response when an already-ready server loses its database.

use anyhow::Result;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use systemprompt_api::services::server::builder::discovery_router;
use systemprompt_api::services::server::readiness::{signal_ready, signal_shutdown};
use systemprompt_test_fixtures::{DisposableDb, fixture_app_context};
use tower::ServiceExt;

#[tokio::test]
async fn readyz_withdraws_readiness_when_its_owned_database_disappears() -> Result<()> {
    let owned = DisposableDb::installed("readyz_db_loss").await?;
    let db = owned.pool().await?;
    let ctx = fixture_app_context(&db, owned.url())?;
    let app = discovery_router(&ctx);
    signal_ready();
    let raw = db.pool_arc()?;
    raw.close().await;

    let response = app
        .oneshot(Request::builder().uri("/readyz").body(Body::empty())?)
        .await?;
    assert_eq!(response.status().as_u16(), 503);
    let body = to_bytes(response.into_body(), 1 << 20).await?;
    let body: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(body["status"], "unready");
    assert_eq!(body["database"], "unreachable");
    assert_eq!(body["instance"], ctx.config().instance_id);
    assert!(
        body["version"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );

    signal_shutdown();
    drop(ctx);
    drop(raw);
    drop(db);
    owned.drop_now().await;
    Ok(())
}
