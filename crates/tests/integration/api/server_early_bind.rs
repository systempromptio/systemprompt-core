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
    let server = bind_and_serve(
        "127.0.0.1:0",
        None,
        systemprompt_runtime::ShutdownRequest::default(),
    )
    .await?;
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
    let err = bind_and_serve(
        &taken.to_string(),
        None,
        systemprompt_runtime::ShutdownRequest::default(),
    )
    .await
    .expect_err("second bind on the same port must fail");
    assert!(err.to_string().contains("Failed to bind"), "{err}");
    Ok(())
}

#[tokio::test]
async fn owned_shutdown_request_drains_listener_when_requested_before_or_after_bind()
-> anyhow::Result<()> {
    for request_before_bind in [true, false] {
        let shutdown = systemprompt_runtime::ShutdownRequest::default();
        if request_before_bind {
            shutdown.request("fixture requested before listener wait");
        }
        let server = bind_and_serve("127.0.0.1:0", None, shutdown.clone()).await?;
        let address = server.local_addr();
        if !request_before_bind {
            let response = reqwest::get(format!("http://{address}/health")).await?;
            assert_eq!(response.status().as_u16(), 200);
            assert!(response.text().await?.contains("starting"));
            shutdown.request("fixture requested after listener wait");
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), server.join())
            .await
            .expect("owned shutdown request must drain listener")?;
        let reconnect = tokio::net::TcpStream::connect(address).await;
        assert!(
            reconnect.is_err(),
            "listener still accepted connections after {} shutdown request",
            if request_before_bind {
                "retained"
            } else {
                "live"
            }
        );
    }
    Ok(())
}

#[tokio::test]
async fn early_bind_emits_typed_binding_and_listening_events_before_shutdown() -> anyhow::Result<()>
{
    use systemprompt_traits::StartupEvent;

    let shutdown = systemprompt_runtime::ShutdownRequest::default();
    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    let server = bind_and_serve("127.0.0.1:0", Some(tx), shutdown.clone()).await?;
    let local = server.local_addr();
    let binding = rx.try_recv().expect("binding event");
    let listening = rx.try_recv().expect("listening event");
    assert!(matches!(binding, StartupEvent::ServerBinding { address } if address == "127.0.0.1:0"));
    assert!(matches!(
        listening,
        StartupEvent::ServerListening { address, pid }
            if address == "127.0.0.1:0" && pid == std::process::id()
    ));
    assert_eq!(
        reqwest::get(format!("http://{local}/livez"))
            .await?
            .status()
            .as_u16(),
        200
    );
    shutdown.request("typed event fixture complete");
    tokio::time::timeout(std::time::Duration::from_secs(5), server.join())
        .await
        .expect("typed event listener drains")?;
    Ok(())
}

#[tokio::test]
async fn dropped_startup_observer_does_not_prevent_binding_or_shutdown() -> anyhow::Result<()> {
    let shutdown = systemprompt_runtime::ShutdownRequest::default();
    let (tx, rx) = futures::channel::mpsc::unbounded();
    drop(rx);
    let server = bind_and_serve("127.0.0.1:0", Some(tx), shutdown.clone()).await?;
    let address = server.local_addr();
    let response = reqwest::get(format!("http://{address}/readyz")).await?;
    assert_eq!(response.status().as_u16(), 503);
    let body = response.text().await?;
    assert!(body.contains("starting"), "{body}");
    shutdown.request("observer-disconnect fixture complete");
    tokio::time::timeout(std::time::Duration::from_secs(5), server.join())
        .await
        .expect("listener drains without startup observer")?;
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
    Ok(())
}
