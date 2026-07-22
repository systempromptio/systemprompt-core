//! Early-bind listener lifecycle: `bind_and_serve` on an ephemeral port,
//! starting-router probe behaviour, and the in-place router swap via
//! `EarlyServer::activate`.

use axum::Router;
use axum::routing::get;
use systemprompt_api::services::server::{bind_and_serve, starting_router};
use tower::ServiceExt;

#[tokio::test]
async fn starting_router_answers_health_and_rejects_everything_else() -> anyhow::Result<()> {
    let app = starting_router();

    let health = app
        .clone()
        .oneshot(super::common::empty_get("/health"))
        .await?;
    assert_eq!(health.status().as_u16(), 200);
    let (_, body) = super::common::body_to_string(health).await?;
    assert!(body.contains("starting"), "{body}");

    let other = app.oneshot(super::common::empty_get("/anything")).await?;
    assert_eq!(other.status().as_u16(), 503);
    let (_, body) = super::common::body_to_string(other).await?;
    assert!(body.contains("service starting"), "{body}");
    Ok(())
}

#[tokio::test]
async fn bind_and_serve_swaps_from_starting_to_activated_router() -> anyhow::Result<()> {
    let server = bind_and_serve("127.0.0.1:0", None).await?;
    let base = format!("http://{}", server.local_addr());
    let client = reqwest::Client::new();

    let starting = client.get(format!("{base}/health")).send().await?;
    assert_eq!(starting.status().as_u16(), 200);
    assert!(starting.text().await?.contains("starting"));

    let blocked = client.get(format!("{base}/full-route")).send().await?;
    assert_eq!(blocked.status().as_u16(), 503);

    server.activate(Router::new().route("/full-route", get(|| async { "activated" })));

    let activated = client.get(format!("{base}/full-route")).send().await?;
    assert_eq!(activated.status().as_u16(), 200);
    assert_eq!(activated.text().await?, "activated");

    let gone = client.get(format!("{base}/health")).send().await?;
    assert_eq!(gone.status().as_u16(), 404);
    Ok(())
}

#[tokio::test]
async fn bind_and_serve_fails_when_port_is_taken() -> anyhow::Result<()> {
    let holder = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let taken = holder.local_addr()?;
    let err = bind_and_serve(&taken.to_string(), None)
        .await
        .expect_err("second bind on the same port must fail");
    assert!(err.to_string().contains("Failed to bind"), "{err}");
    Ok(())
}
