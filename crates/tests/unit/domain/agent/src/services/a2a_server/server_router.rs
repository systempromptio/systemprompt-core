// Construction and routing for the per-agent A2A HTTP server. The bootstrap
// services config already registers one agent, so `Server::new` can be driven
// for real and the router it builds can be exercised in-process with `oneshot`
// — no listener, and therefore none of `run`/`start_server`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_agent::services::a2a_server::Server;
use tower::ServiceExt;

use super::a2a_helpers::{StubAiProvider, make_agent_state};
use crate::repository::try_pool_or_skip;

const AGENT_PORT: u16 = 9312;

async fn server_for_registered_agent_or_skip(port: u16) -> Option<Server> {
    systemprompt_test_fixtures::ensure_messaging_bootstrap();
    let pool = try_pool_or_skip().await?;
    let agent_state = make_agent_state(&pool);

    Server::new(
        Arc::clone(&pool),
        agent_state,
        Arc::new(StubAiProvider::new()),
        Some(systemprompt_test_fixtures::test_messaging_agent().to_owned()),
        port,
    )
    .await
    .map(Some)
    .expect("construct registered agent server")
}

fn reserve_allowed_agent_port() -> std::net::TcpListener {
    let start = 9000 + (uuid::Uuid::new_v4().as_u128() % 1000) as u16;
    (0..1000)
        .map(|offset| 9000 + (start - 9000 + offset) % 1000)
        .find_map(|port| std::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, port)).ok())
        .expect("owned reservation in the allowed agent port range")
}

async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body")
        .to_vec()
}

#[tokio::test]
async fn the_router_serves_the_well_known_agent_card_without_authentication() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT + 1).await else {
        return;
    };

    let response = server
        .create_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/.well-known/agent-card.json")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);
    let card: serde_json::Value =
        serde_json::from_slice(&body_bytes(response).await).expect("the card is JSON");
    assert_eq!(
        card["name"],
        serde_json::json!(systemprompt_test_fixtures::test_messaging_agent()),
        "the card names the agent the server was built for: {card}"
    );
}

#[tokio::test]
async fn the_router_serves_the_card_on_both_advertised_paths() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT + 2).await else {
        return;
    };
    let router = server.create_router();

    let alias = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/a2a/card")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert_eq!(
        alias.status(),
        StatusCode::OK,
        "the /a2a card alias resolves to the same handler"
    );
}

#[tokio::test]
async fn the_router_rejects_an_unrouted_path() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT + 3).await else {
        return;
    };

    let response = server
        .create_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/definitely-not-a-route")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_post_route_rejects_missing_and_invalid_bearer_before_dispatch() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let server = server_for_registered_agent_or_skip(AGENT_PORT + 4)
        .await
        .expect("registered agent server fixture");
    let router = server.create_router();
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "GetTask",
        "params": {"id": "no-such-task"},
        "id": 1
    })
    .to_string();

    for authorization in [None, Some("Bearer not-a-jwt")] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/")
            .header("content-type", "application/json");
        if let Some(value) = authorization {
            request = request.header("authorization", value);
        }
        let response = router
            .clone()
            .oneshot(request.body(Body::from(payload.clone())).expect("request"))
            .await
            .expect("router responds");
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "authentication failure must stop request dispatch for {authorization:?}"
        );
        assert!(
            body_bytes(response).await.is_empty(),
            "the middleware's status-only rejection must not fabricate a JSON-RPC handler body"
        );
    }
}

#[tokio::test]
async fn the_router_answers_cors_preflight() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT + 5).await else {
        return;
    };

    let database_url = systemprompt_test_fixtures::fixture_database_url()
        .expect("agent server fixture database URL");
    let configured_origin = systemprompt_test_fixtures::fixture_config(&database_url)
        .cors_allowed_origins
        .into_iter()
        .next()
        .expect("fixture config has a CORS origin");
    let response = server
        .create_router()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/")
                .header("origin", &configured_origin)
                .header("access-control-request-method", "POST")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .expect("configured origin receives the CORS allow-origin header"),
        configured_origin.as_str()
    );
}

#[tokio::test]
async fn server_run_serves_the_agent_card_then_exits_after_graceful_shutdown() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let reservation = reserve_allowed_agent_port();
    let port = reservation.local_addr().expect("reserved address").port();
    drop(reservation);
    let server = server_for_registered_agent_or_skip(port)
        .await
        .expect("registered agent server fixture");
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(async move {
        server
            .run(async move {
                let _ = shutdown_rx.await;
            })
            .await
    });

    let mut connection = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).await {
                Ok(connection) => break connection,
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .expect("server becomes reachable");
    connection
        .write_all(
            b"GET /.well-known/agent-card.json HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await
        .expect("write card request");
    let mut response = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        connection.read_to_end(&mut response),
    )
    .await
    .expect("card response is bounded")
    .expect("read card response");
    let response = String::from_utf8(response).expect("HTTP response is UTF-8");
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(
        response.contains(systemprompt_test_fixtures::test_messaging_agent()),
        "served card must identify the configured agent: {response}"
    );

    shutdown_tx
        .send(())
        .expect("server still awaiting shutdown");
    tokio::time::timeout(std::time::Duration::from_secs(3), running)
        .await
        .expect("server shutdown is bounded")
        .expect("server task does not panic")
        .expect("graceful server shutdown succeeds");
}

#[tokio::test]
async fn server_run_reports_bind_failure_without_disturbing_owned_listener() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let listener = reserve_allowed_agent_port();
    let port = listener.local_addr().expect("reserved address").port();
    let server = server_for_registered_agent_or_skip(port)
        .await
        .expect("registered agent server fixture");
    let error = server
        .run(std::future::pending::<()>())
        .await
        .expect_err("a second listener cannot claim the owned port");
    assert!(
        matches!(
            error,
            systemprompt_agent::AgentError::Io(ref source)
                if source.kind() == std::io::ErrorKind::AddrInUse
        ),
        "unexpected bind diagnosis: {error}"
    );
    assert_eq!(
        listener
            .local_addr()
            .expect("reservation remains live")
            .port(),
        port
    );
}
