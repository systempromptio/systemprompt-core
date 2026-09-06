//! An upstream that answers 200 with a body carrying no turn must reach the
//! client as a failure, not as a successful empty message.
//!
//! Observed live on 2026-09-05 against a Vertex `MaaS` model: two of three
//! buffered calls returned HTTP 200 with `content: []` and zero tokens, and
//! the audit row recorded them as completed. The buffered parsers are total,
//! so an empty body became a well-formed empty response and nothing in the
//! chain noticed. This cell drives the whole gateway path against a mock that
//! answers `{}` and pins both halves of the fix: the client gets an error
//! status, and the audit row is `failed`.

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use serde_json::json;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::headers::SESSION_ID;
use systemprompt_test_fixtures::{
    AuthedFixture, TestBootstrap, fixture_app_context, fixture_db_pool, init_services_bootstrap,
    install_test_signing_key, seed_admin_credential,
};
use tokio::sync::OnceCell;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MODEL: &str = "claude-empty-body-fixture";
const SECRET_NAME: &str = "anthropic_api_key";

struct Harness {
    boot: TestBootstrap,
    _upstream: MockServer,
}

static HARNESS: OnceCell<Harness> = OnceCell::const_new();

fn gateway_yaml(endpoint: &str) -> String {
    format!(
        r#"
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: {endpoint}
    api_key_secret: {SECRET_NAME}
    models:
      - id: {MODEL}
        pricing:
          input_per_million: 3.0
          output_per_million: 15.0
          cache_read_per_million: 0.0
gateway:
  enabled: true
  allow_unlisted_models: false
  routes:
    - id: empty-body-fixture
      model_pattern: "{MODEL}"
      provider: anthropic
"#
    )
}

async fn harness() -> &'static Harness {
    HARNESS
        .get_or_init(|| async {
            // SAFETY: set before any bootstrap call; process-local under nextest.
            unsafe {
                std::env::set_var("SYSTEMPROMPT_CUSTOM_SECRETS", SECRET_NAME);
                std::env::set_var(SECRET_NAME, "test-provider-key");
            }

            let upstream = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/messages"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
                .mount(&upstream)
                .await;

            let boot = init_services_bootstrap(&gateway_yaml(&upstream.uri()));
            Harness {
                boot,
                _upstream: upstream,
            }
        })
        .await
}

async fn app() -> Result<(Router, DbPool)> {
    let h = harness().await;
    install_test_signing_key();
    let pool = fixture_db_pool(&h.boot.database_url).await?;
    let ctx = fixture_app_context(&pool, &h.boot.database_url)?;
    Ok((
        gateway_router(&ctx).expect("gateway router available"),
        pool,
    ))
}

async fn credential(pool: &DbPool) -> Result<AuthedFixture> {
    seed_admin_credential(pool, &format!("empty-{}@example.invalid", Uuid::new_v4())).await
}

fn messages_post(cred: &AuthedFixture) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri("/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", cred.jwt.as_str()),
        )
        .header(SESSION_ID, cred.session_id.as_str())
        .body(Body::from(
            json!({
                "model": MODEL,
                "max_tokens": 64,
                "messages": [{"role": "user", "content": "say something"}],
            })
            .to_string(),
        ))
        .expect("request must build")
}

#[tokio::test]
async fn an_empty_upstream_body_is_not_relayed_as_a_successful_turn() -> Result<()> {
    let (app, pool) = app().await?;
    let cred = credential(&pool).await?;
    let (status, body) =
        super::common::body_to_string(app.oneshot(messages_post(&cred)).await?).await?;

    assert!(
        status.is_server_error(),
        "an empty upstream body must fail the request, got {status}: {body}"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&body).unwrap_or(serde_json::Value::String(body.clone()));
    assert!(
        parsed.get("content").is_none(),
        "the client must not receive a message envelope: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn an_empty_upstream_body_is_audited_as_failed() -> Result<()> {
    let (app, pool) = app().await?;
    let cred = credential(&pool).await?;
    let _ = super::common::body_to_string(app.oneshot(messages_post(&cred)).await?).await?;

    let pg = pool.pool_arc()?;
    let mut settled = None;
    for _ in 0..100 {
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT status, error_message FROM ai_requests \
             WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(cred.user_id.as_str())
        .fetch_optional(pg.as_ref())
        .await?;
        if let Some(row) = row
            && row.0 != "pending"
        {
            settled = Some(row);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let (status, error_message) = settled.expect("an audit row must settle");
    assert_eq!(status, "failed", "audit status");
    let message = error_message.unwrap_or_default();
    assert!(
        message.contains("no content and no usage"),
        "the audit must name the defect: {message}"
    );
    Ok(())
}
