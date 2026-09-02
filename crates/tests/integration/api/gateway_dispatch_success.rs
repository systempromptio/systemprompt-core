//! A gateway request that completes end to end against a real upstream.
//!
//! Everything to date stops before the outbound call: with no provider secret
//! configured, dispatch fails closed at credential resolution. Supplying the
//! secret and pointing the provider at a `wiremock` server drives the whole
//! path for the first time — route resolution, credential lookup, the outbound
//! adapter, response canonicalisation, the audit row, and the tool-call
//! persistence that only runs when a completion actually returns tool uses.
//!
//! The provider endpoint is only known once the mock server is bound, so the
//! profile is rendered after it starts; both are held in one async `OnceCell`
//! because the bootstrap's tempdir must outlive every request.

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use serde_json::json;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::headers::SESSION_ID;
use systemprompt_test_fixtures::{
    AuthedFixture, TestBootstrap, fixture_app_context, fixture_db_pool, init_gateway_bootstrap,
    install_test_signing_key, seed_admin_credential,
};
use tokio::sync::OnceCell;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MODEL: &str = "claude-dispatch-fixture";
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
gateway:
  enabled: true
  allow_unlisted_models: false
  routes:
    - id: dispatch-fixture
      model_pattern: "{MODEL}"
      provider: anthropic
"#
    )
}

fn upstream_response() -> serde_json::Value {
    json!({
        "id": "msg_dispatch_fixture",
        "type": "message",
        "role": "assistant",
        "model": MODEL,
        "content": [
            {"type": "text", "text": "checking that for you"},
            {
                "type": "tool_use",
                "id": "toolu_fixture_1",
                "name": "list_files",
                "input": {"path": "/srv"}
            }
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 11, "output_tokens": 7}
    })
}

async fn harness() -> &'static Harness {
    HARNESS
        .get_or_init(|| async {
            // The secret must be resolvable before the bootstrap initialises the
            // secrets singleton, and the fixture only seeds the keys it knows.
            // SAFETY: set before any bootstrap call; process-local under nextest.
            unsafe {
                std::env::set_var("SYSTEMPROMPT_CUSTOM_SECRETS", SECRET_NAME);
                std::env::set_var(SECRET_NAME, "test-provider-key");
            }

            let upstream = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/messages"))
                .respond_with(ResponseTemplate::new(200).set_body_json(upstream_response()))
                .mount(&upstream)
                .await;

            let boot = init_gateway_bootstrap(&gateway_yaml(&upstream.uri()));
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
    seed_admin_credential(
        pool,
        &format!("dispatch-{}@example.invalid", Uuid::new_v4()),
    )
    .await
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
                "messages": [{"role": "user", "content": "what is in /srv?"}],
            })
            .to_string(),
        ))
        .expect("request must build")
}

async fn dispatch() -> Result<(u16, serde_json::Value)> {
    let (app, pool) = app().await?;
    let cred = credential(&pool).await?;
    let (status, body) =
        super::common::body_to_string(app.oneshot(messages_post(&cred)).await?).await?;
    Ok((
        status.as_u16(),
        serde_json::from_str(&body).unwrap_or(serde_json::Value::String(body)),
    ))
}

#[tokio::test]
async fn a_fully_configured_request_reaches_the_upstream_and_returns_its_answer() -> Result<()> {
    let (status, body) = dispatch().await?;

    assert_eq!(status, 200, "{body}");
    let text = body["content"]
        .as_array()
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
        })
        .unwrap_or_default();
    assert_eq!(
        text, "checking that for you",
        "the upstream's answer must reach the caller unaltered: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn the_tool_call_the_model_asked_for_survives_the_round_trip() -> Result<()> {
    let (status, body) = dispatch().await?;
    assert_eq!(status, 200, "{body}");

    let tool = body["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find(|b| b["type"] == "tool_use"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    // A dropped or renamed tool call is a silently broken agent loop: the
    // client executes nothing and the model waits forever.
    assert_eq!(tool["name"].as_str(), Some("list_files"), "{body}");
    assert_eq!(tool["input"]["path"].as_str(), Some("/srv"), "{body}");
    assert_eq!(body["stop_reason"].as_str(), Some("tool_use"), "{body}");
    Ok(())
}

#[tokio::test]
async fn a_completed_request_is_recorded_with_its_usage_and_cost() -> Result<()> {
    let (app, pool) = app().await?;
    let cred = credential(&pool).await?;

    let (status, _body) =
        super::common::body_to_string(app.oneshot(messages_post(&cred)).await?).await?;
    assert_eq!(status.as_u16(), 200);

    // The completion audit is written after the response is handed back, so
    // the row settles a moment later.
    let pg = pool.pool_arc()?;
    let mut settled = None;
    for _ in 0..100 {
        let row: Option<(String, Option<i32>, Option<i32>, i64)> = sqlx::query_as(
            "SELECT status, input_tokens, output_tokens, cost_microdollars FROM ai_requests \
             WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(cred.user_id.as_str())
        .fetch_optional(pg.as_ref())
        .await?;
        if let Some(row) = row
            && row.0 == "completed"
        {
            settled = Some(row);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let (req_status, input, output, cost) =
        settled.expect("a dispatched request must settle as a completed audit row");
    assert_eq!(req_status, "completed", "the row must record the outcome");
    assert_eq!(input, Some(11), "the upstream's token counts are recorded");
    assert_eq!(output, Some(7));
    assert!(
        cost > 0,
        "a priced model must bill something, or usage reporting is blind"
    );
    Ok(())
}

#[tokio::test]
async fn the_tool_calls_are_persisted_against_the_request() -> Result<()> {
    let (app, pool) = app().await?;
    let cred = credential(&pool).await?;

    let (status, _body) =
        super::common::body_to_string(app.oneshot(messages_post(&cred)).await?).await?;
    assert_eq!(status.as_u16(), 200);

    let pg = pool.pool_arc()?;
    let mut recorded = None;
    for _ in 0..100 {
        let row: Option<(String, i32)> = sqlx::query_as(
            "SELECT t.tool_name, t.sequence_number FROM ai_request_tool_calls t \
             JOIN ai_requests r ON r.id = t.request_id \
             WHERE r.user_id = $1 ORDER BY r.created_at DESC, t.sequence_number ASC LIMIT 1",
        )
        .bind(cred.user_id.as_str())
        .fetch_optional(pg.as_ref())
        .await?;
        if row.is_some() {
            recorded = row;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let (tool_name, sequence) = recorded.expect("a completion with a tool use must record it");
    assert_eq!(tool_name, "list_files");
    assert_eq!(sequence, 1, "tool calls are numbered from one, in order");
    Ok(())
}

// A streaming dispatch is the only thing that constructs the tap wrappers in
// `services::gateway::stream_tap` — they are private and built inside
// `GatewayService`, so nothing short of a real streaming round trip polls them.
mod streaming {
    use super::{MODEL, SECRET_NAME, credential, upstream_response};

    use anyhow::Result;
    use axum::body::Body;
    use axum::http::{Request, header};
    use http_body_util::BodyExt;
    use serde_json::json;
    use systemprompt_api::routes::gateway::gateway_router;
    use systemprompt_database::DbPool;
    use systemprompt_identifiers::headers::SESSION_ID;
    use systemprompt_test_fixtures::{
        AuthedFixture, TestBootstrap, fixture_app_context, fixture_db_pool, init_gateway_bootstrap,
        install_test_signing_key,
    };
    use tokio::sync::OnceCell;
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const SSE: &str = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_stream\",",
        "\"model\":\"claude-dispatch-fixture\",\"role\":\"assistant\",\"content\":[],",
        "\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\n",
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,",
        "\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,",
        "\"delta\":{\"type\":\"text_delta\",\"text\":\"streamed answer\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},",
        "\"usage\":{\"output_tokens\":4}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );

    struct Harness {
        boot: TestBootstrap,
        _upstream: MockServer,
    }

    static HARNESS: OnceCell<Harness> = OnceCell::const_new();

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
                    // `set_body_string` stamps `text/plain`; the Anthropic wire is a
                    // byte-level passthrough, so the upstream's declared type is
                    // what the gateway relays and it has to be set properly here.
                    .respond_with(ResponseTemplate::new(200).set_body_raw(SSE, "text/event-stream"))
                    .mount(&upstream)
                    .await;
                // A non-streaming request against this harness would get the SSE
                // body, so the buffered assertions stay in the parent module.
                let _ = upstream_response();

                let boot = init_gateway_bootstrap(&super::gateway_yaml(&upstream.uri()));
                Harness {
                    boot,
                    _upstream: upstream,
                }
            })
            .await
    }

    async fn app() -> Result<(axum::Router, DbPool)> {
        let h = harness().await;
        install_test_signing_key();
        let pool = fixture_db_pool(&h.boot.database_url).await?;
        let ctx = fixture_app_context(&pool, &h.boot.database_url)?;
        Ok((
            gateway_router(&ctx).expect("gateway router available"),
            pool,
        ))
    }

    fn streaming_post(cred: &AuthedFixture) -> Request<Body> {
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
                    "stream": true,
                    "messages": [{"role": "user", "content": "stream it"}],
                })
                .to_string(),
            ))
            .expect("request must build")
    }

    #[tokio::test]
    async fn a_streaming_dispatch_relays_the_upstream_frames() -> Result<()> {
        let (app, pool) = app().await?;
        let cred = credential(&pool).await?;

        let resp = app.oneshot(streaming_post(&cred)).await?;
        assert_eq!(resp.status().as_u16(), 200, "{:?}", resp.status());
        let content_type = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        assert!(
            content_type.contains("event-stream"),
            "a streamed answer must stay SSE end to end; a client that dispatches on \
             content-type would not treat {content_type} as a stream"
        );

        let body = resp.into_body().collect().await?.to_bytes();
        let text = String::from_utf8_lossy(&body).into_owned();

        assert!(
            text.contains("streamed answer"),
            "the model's tokens must reach the caller: {text}"
        );
        assert!(
            text.contains("message_stop"),
            "the terminal frame must be relayed, or the client hangs: {text}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_streamed_request_is_audited_once_the_stream_ends() -> Result<()> {
        let (app, pool) = app().await?;
        let cred = credential(&pool).await?;

        let resp = app.oneshot(streaming_post(&cred)).await?;
        assert_eq!(resp.status().as_u16(), 200);
        // The tap finalises on stream EOF, so the body must be drained first.
        let _ = resp.into_body().collect().await?.to_bytes();

        let pg = pool.pool_arc()?;
        let mut settled = None;
        for _ in 0..100 {
            let row: Option<(String, bool)> = sqlx::query_as(
                "SELECT status, is_streaming FROM ai_requests WHERE user_id = $1 \
                 ORDER BY created_at DESC LIMIT 1",
            )
            .bind(cred.user_id.as_str())
            .fetch_optional(pg.as_ref())
            .await?;
            if let Some(row) = row
                && row.0 == "completed"
            {
                settled = Some(row);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        let (status, is_streaming) =
            settled.expect("a streamed request must settle as a completed audit row");
        assert_eq!(status, "completed");
        assert!(
            is_streaming,
            "the audit row must record that this was a stream, or usage reporting mislabels it"
        );
        Ok(())
    }
}
