//! `/v1/me/provider-credentials` and `/v1/messages/count_tokens` against a
//! profile with the gateway enabled and one personal credential whose
//! endpoint is a local mock upstream that accepts only the key `good-key`.

use std::sync::OnceLock;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_test_fixtures::{
    TestBootstrap, init_services_bootstrap, seed_admin_credential, test_app_context_on,
    test_db_pool,
};
use tower::ServiceExt;

use super::common::body_to_string;

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn mock_upstream() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock upstream");
    listener.set_nonblocking(true).expect("nonblocking");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("mock runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            let app = Router::new().route(
                "/v1/models",
                axum::routing::get(|headers: HeaderMap| async move {
                    let auth = headers
                        .get(header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default()
                        .to_owned();
                    if auth.ends_with("good-key") {
                        (StatusCode::OK, r#"{"data":[]}"#)
                    } else if auth.ends_with("flaky-key") {
                        (StatusCode::INTERNAL_SERVER_ERROR, "{}")
                    } else {
                        (StatusCode::UNAUTHORIZED, "{}")
                    }
                }),
            );
            axum::serve(listener, app).await.expect("mock serves");
        });
    });
    port
}

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| {
        let port = mock_upstream();
        init_services_bootstrap(&format!(
            r#"
credentials:
  byok:
    kind: personal
    label: Bring Your Own
providers:
  byok:
    credential: byok
    endpoints:
      api:
        wire: openai-chat
        url: http://127.0.0.1:{port}/v1
models:
  byok-model-1:
    clients:
    - openai
    pricing:
      input_per_million: 1.0
      output_per_million: 1.0
      cache_read_per_million: 0.0
deployments:
  byok-model-1:
  - byok/api
gateway:
  enabled: true
"#
        ))
    })
}

pub(super) async fn app() -> anyhow::Result<Router> {
    let b = boot();
    let pool = test_db_pool().await;
    let ctx = test_app_context_on(b, &pool);
    Ok(gateway_router(&ctx).expect("gateway router builds"))
}

pub(super) async fn token(email: &str) -> anyhow::Result<String> {
    boot();
    let pool = test_db_pool().await;
    Ok(seed_admin_credential(&pool, email)
        .await?
        .jwt
        .as_str()
        .to_owned())
}

pub(super) fn request(
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<String>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    builder
        .body(body.map_or_else(Body::empty, Body::from))
        .expect("request builds")
}

pub(super) async fn send(req: Request<Body>) -> anyhow::Result<(StatusCode, String)> {
    body_to_string(app().await?.oneshot(req).await?).await
}

#[tokio::test]
async fn provider_credentials_refuse_a_missing_or_forged_credential() -> anyhow::Result<()> {
    let (status, _) = send(request("GET", "/me/provider-credentials", None, None)).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(request(
        "GET",
        "/me/provider-credentials",
        Some("not-a-jwt"),
        None,
    ))
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn a_verified_key_is_stored_listed_and_removed_without_being_echoed() -> anyhow::Result<()> {
    let token = token("byok-roundtrip@example.invalid").await?;
    let (status, body) = send(request(
        "GET",
        "/me/provider-credentials",
        Some(&token),
        None,
    ))
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(rows[0]["credential"], "byok");
    assert_eq!(rows[0]["configured"], false);

    let put = serde_json::json!({ "api_key": "good-key", "label": " laptop " }).to_string();
    let (status, body) = send(request(
        "PUT",
        "/me/provider-credentials/byok",
        Some(&token),
        Some(put),
    ))
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        !body.contains("good-key"),
        "the key is never echoed: {body}"
    );
    let row: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(row["configured"], true);
    assert_eq!(row["label"], "laptop");

    let (_, body) = send(request(
        "GET",
        "/me/provider-credentials",
        Some(&token),
        None,
    ))
    .await?;
    let rows: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(rows[0]["configured"], true, "{body}");

    let (status, _) = send(request(
        "DELETE",
        "/me/provider-credentials/byok",
        Some(&token),
        None,
    ))
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = send(request(
        "GET",
        "/me/provider-credentials",
        Some(&token),
        None,
    ))
    .await?;
    let rows: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(rows[0]["configured"], false, "{body}");
    Ok(())
}

#[tokio::test]
async fn rejected_unverified_unknown_and_malformed_keys_are_refused() -> anyhow::Result<()> {
    let token = token("byok-refusals@example.invalid").await?;
    let put = |key: &str| serde_json::json!({ "api_key": key }).to_string();

    let (status, body) = send(request(
        "PUT",
        "/me/provider-credentials/byok",
        Some(&token),
        Some(put("bad-key")),
    ))
    .await?;
    assert!(status.is_client_error(), "{status} {body}");
    assert!(!body.contains("bad-key"), "{body}");

    let (status, body) = send(request(
        "PUT",
        "/me/provider-credentials/byok",
        Some(&token),
        Some(put("flaky-key")),
    ))
    .await?;
    assert_ne!(status, StatusCode::OK, "{body}");

    let (status, body) = send(request(
        "PUT",
        "/me/provider-credentials/nope",
        Some(&token),
        Some(put("good-key")),
    ))
    .await?;
    assert!(status.is_client_error(), "{status} {body}");
    let (status, _) = send(request(
        "DELETE",
        "/me/provider-credentials/nope",
        Some(&token),
        None,
    ))
    .await?;
    assert!(status.is_client_error());

    let (status, _) = send(request(
        "PUT",
        "/me/provider-credentials/byok",
        Some(&token),
        Some("{not json".to_owned()),
    ))
    .await?;
    assert!(status.is_client_error());

    let (_, body) = send(request(
        "GET",
        "/me/provider-credentials",
        Some(&token),
        None,
    ))
    .await?;
    let rows: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(
        rows[0]["configured"], false,
        "no refused key is stored: {body}"
    );
    Ok(())
}
