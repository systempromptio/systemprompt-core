use axum::body::Body;
use axum::http::{Request, header};
use serde_json::json;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_identifiers::headers::SESSION_ID;
use systemprompt_test_fixtures::{
    fixture_app_context, fixture_db_pool, init_services_bootstrap, install_test_signing_key,
    seed_admin_credential, test_key,
};
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn dispatch(
    secret: String,
    token_server: &MockServer,
    expected_success: bool,
) -> anyhow::Result<()> {
    // SAFETY: nextest starts each test in its own process; initialize the
    // secret before constructing the one-shot bootstrap.
    unsafe {
        std::env::set_var("SYSTEMPROMPT_CUSTOM_SECRETS", "coverage_google_key");
        std::env::set_var("coverage_google_key", secret);
    }
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates":[{"content":{"role":"model","parts":[{"text":"credential exchange worked"}]},"finishReason":"STOP"}],
            "usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":5,"totalTokenCount":8}
        }))).expect(if expected_success { 2 } else { 0 }).mount(&upstream).await;
    let boot = init_services_bootstrap(&format!(
        r#"
providers:
  - name: google-fixture
    wire: gemini
    surface: gemini
    endpoint: {}
    api_key_secret: coverage_google_key
    models:
      - id: gemini-fixture
        pricing:
          input_per_million: 1.0
          output_per_million: 1.0
          cache_read_per_million: 0.0
gateway:
  enabled: true
  routes:
    - id: google-coverage
      model_pattern: gemini-fixture
      provider: google-fixture
"#,
        upstream.uri()
    ));
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url).await?;
    let ctx = fixture_app_context(&pool, &boot.database_url)?;
    let app = gateway_router(&ctx).expect("gateway enabled");
    let cred = seed_admin_credential(
        &pool,
        &format!("google-{}@example.invalid", uuid::Uuid::new_v4()),
    )
    .await?;
    for _ in 0..if expected_success { 2 } else { 1 } {
        let request = Request::builder().method("POST").uri("/messages")
            .header(header::AUTHORIZATION, format!("Bearer {}", cred.jwt.as_str()))
            .header(SESSION_ID, cred.session_id.as_str())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"model":"gemini-fixture","max_tokens":32,"messages":[{"role":"user","content":"hello"}]}).to_string()))?;
        let response = app.clone().oneshot(request).await?;
        let (status, body) = super::common::body_to_string(response).await?;
        if expected_success {
            assert_eq!(status, http::StatusCode::OK, "{body}");
            assert!(body.contains("credential exchange worked"), "{body}");
        } else {
            assert!(status.is_server_error(), "{status}: {body}");
            assert!(
                !body.contains("PRIVATE KEY"),
                "private key must not enter the error body"
            );
        }
    }
    for request in upstream.received_requests().await.unwrap() {
        assert_eq!(
            request.headers["authorization"],
            "Bearer minted-google-token"
        );
        assert!(!request.headers.contains_key("x-goog-api-key"));
        assert!(!request.url.query().unwrap_or_default().contains("key="));
    }
    token_server.verify().await;
    Ok(())
}

fn account(server: &MockServer) -> serde_json::Value {
    json!({"type":"service_account", "client_email":"gateway@example.invalid", "private_key":test_key(1).to_pkcs8_pem().unwrap(), "token_uri":format!("{}/token", server.uri())})
}

#[tokio::test]
async fn coverage_google_gateway_mints_once_and_forwards_bearer_authentication()
-> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token":"minted-google-token","expires_in":3600})),
        )
        .expect(1)
        .mount(&server)
        .await;
    dispatch(account(&server).to_string(), &server, true).await
}

#[tokio::test]
async fn coverage_google_gateway_stops_before_inference_when_exchange_fails() -> anyhow::Result<()>
{
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(403).set_body_string("account disabled"))
        .expect(1)
        .mount(&server)
        .await;
    dispatch(account(&server).to_string(), &server, false).await
}

#[tokio::test]
async fn coverage_google_gateway_rejects_malformed_declared_service_accounts() -> anyhow::Result<()>
{
    let server = MockServer::start().await;
    dispatch(
        json!({"type":"service_account","client_email":"no-key@example.invalid"}).to_string(),
        &server,
        false,
    )
    .await?;
    assert!(server.received_requests().await.unwrap().is_empty());
    Ok(())
}
