//! A `kind: caller` credential at the HTTP boundary.
//!
//! The request a client sends with its own upstream credential reaches the
//! upstream as the client built it, once, outside the platform's trace, and
//! the upstream's answer returns whole. Everything is asserted on what the
//! wiremock upstream received and what the client got back; each refusal is
//! paired with the count of upstream requests it must leave at zero.

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::headers::{SESSION_ID, SYSTEMPROMPT_AUTHORIZATION};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    AuthedFixture, DisposableDb, TestBootstrap, init_services_bootstrap, seed_admin_credential,
    test_app_context_on, test_db_pool,
};
use systemprompt_traits::DrainOutcome;
use tokio::sync::OnceCell;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MODEL: &str = "claude-own-fixture";
const CHAIN_MODEL: &str = "claude-own-chain";
const UPSTREAM_CREDENTIAL: &str = "Bearer sk-ant-oat01-the-clients-own";
const DRAIN_BOUND: Duration = Duration::from_secs(30);

const ANSWER: &str = "{ \"id\":\"msg_own_1\",\"type\":\"message\",\"role\":\"assistant\",\
    \"model\":\"claude-own-fixture\",\n  \"content\":[{\"type\":\"tool_use\",\"id\":\"tu_1\",\
    \"name\":\"list_files\",\"input\":{}}],\"stop_reason\":\"end_turn\",\
    \"vendor_only\":{\"k\":[1,2]},\"usage\":{\"input_tokens\":11,\"output_tokens\":7} }";

const STREAM: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":\
    {\"id\":\"msg_s1\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-own-fixture\",\
    \"content\":[],\"usage\":{\"input_tokens\":11,\"output_tokens\":1}}}\n\n: ping\n\n\
    event: vendor_thing\ndata: {\"type\":\"vendor_thing\",\"k\":1}\n\n\
    event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\
    \"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
    event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\
    \"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n\
    event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
    event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\
    \"usage\":{\"output_tokens\":7}}\n\n\
    event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

struct Harness {
    boot: TestBootstrap,
    upstream: MockServer,
}

static HARNESS: OnceCell<Harness> = OnceCell::const_new();

fn gateway_yaml(endpoint: &str) -> String {
    format!(
        r#"
credentials:
  subscription:
    kind: caller
    label: Own subscription
providers:
  claude-own:
    credential: subscription
    endpoints:
      api:
        wire: anthropic
        url: {endpoint}
      a:
        wire: anthropic
        url: {endpoint}/a
      b:
        wire: anthropic
        url: {endpoint}/b
models:
  {MODEL}:
    clients:
    - claude
    pricing:
      input_per_million: 3.0
      output_per_million: 15.0
      cache_read_per_million: 0.0
  {CHAIN_MODEL}:
    clients:
    - claude
    pricing:
      input_per_million: 3.0
      output_per_million: 15.0
      cache_read_per_million: 0.0
deployments:
  {MODEL}:
  - claude-own/api
  {CHAIN_MODEL}:
  - claude-own/a
  - claude-own/b
gateway:
  enabled: true
"#
    )
}

async fn harness() -> &'static Harness {
    HARNESS
        .get_or_init(|| async {
            let upstream = MockServer::start().await;
            let boot = init_services_bootstrap(&gateway_yaml(&upstream.uri()));
            Harness { boot, upstream }
        })
        .await
}

async fn app_on(pool: &DbPool) -> (Router, Arc<AppContext>, &'static MockServer) {
    let h = harness().await;
    let ctx = test_app_context_on(&h.boot, pool);
    (
        gateway_router(&ctx).expect("gateway router builds"),
        ctx,
        &h.upstream,
    )
}

async fn member(pool: &DbPool) -> Result<AuthedFixture> {
    seed_admin_credential(pool, &format!("own-{}@example.invalid", Uuid::new_v4())).await
}

fn request_body(model: &str, marker: &str, stream: bool) -> String {
    format!(
        "{{ \"model\":\"{model}\", \"max_tokens\":64,\n \"stream\":{stream}, \
         \"vendor_only_field\":{{\"k\":[1,2]}}, \
         \"messages\":[{{\"role\":\"user\",\"content\":\"{marker} what is in /srv?\"}}] }}"
    )
}

fn presented(cred: &AuthedFixture, uri: &str) -> http::request::Builder {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", UPSTREAM_CREDENTIAL)
        .header(
            SYSTEMPROMPT_AUTHORIZATION,
            format!("Bearer {}", cred.jwt.as_str()),
        )
        .header(SESSION_ID, cred.session_id.as_str())
        .header("user-agent", "claude-cli/9.9 (external, cli)")
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "client-capability-a,client-capability-b")
        .header("x-stainless-os", "Linux")
}

fn post(cred: &AuthedFixture, uri: &str, body: String) -> Request<Body> {
    presented(cred, uri)
        .body(Body::from(body))
        .expect("request builds")
}

async fn collect(response: Response<Body>) -> (http::StatusCode, http::HeaderMap, Vec<u8>) {
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .map(|collected| collected.to_bytes().to_vec())
        .unwrap_or_default();
    (status, headers, body)
}

async fn drain(ctx: &AppContext) {
    assert_eq!(
        ctx.background_tasks().drain(DRAIN_BOUND).await,
        DrainOutcome::Drained,
        "the post-response audit work must finish"
    );
}

async fn received_with(upstream: &MockServer, marker: &str) -> Vec<wiremock::Request> {
    upstream
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|request| String::from_utf8_lossy(&request.body).contains(marker))
        .collect()
}

fn answer(status: u16) -> ResponseTemplate {
    ResponseTemplate::new(status)
        .insert_header("anthropic-ratelimit-unified-5h-utilization", "0.42")
        .insert_header("request-id", "req_upstream_own_1")
        .insert_header("set-cookie", "__cf_bm=1; Path=/")
}

type AuditRow = (String, String, Option<i32>, Option<i32>, i64);

async fn audit_row(pool: &DbPool, cred: &AuthedFixture) -> AuditRow {
    sqlx::query_as(
        "SELECT status, billing_source, input_tokens, output_tokens, cost_microdollars FROM \
         ai_requests WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(cred.user_id.as_str())
    .fetch_one(pool.pool().as_ref())
    .await
    .expect("a dispatched request leaves an audit row")
}

#[tokio::test]
async fn a_request_reaches_the_upstream_as_sent_and_its_answer_returns_whole() -> Result<()> {
    let pool = test_db_pool().await;
    let (app, ctx, upstream) = app_on(&pool).await;
    let cred = member(&pool).await?;
    let marker = "marker-whole";
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains(marker))
        .respond_with(answer(200).set_body_raw(ANSWER, "application/json"))
        .mount(upstream)
        .await;
    let body = request_body(MODEL, marker, false);
    let request = presented(&cred, "/messages")
        .header("cookie", "session=abc")
        .header("x-forwarded-for", "203.0.113.9")
        .header(
            "traceparent",
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        )
        .header("x-trace-id", "client-trace")
        .header("x-goog-api-key", "a-second-credential")
        .header("x-systemprompt-memory", "off")
        .header("mcp-session-id", "platform-mcp-session")
        .body(Body::from(body.clone()))?;

    let (status, headers, returned) = collect(app.oneshot(request).await?).await;
    drain(&ctx).await;

    let received = received_with(upstream, marker).await;
    assert_eq!(received.len(), 1, "sent once");
    let sent = &received[0];
    assert_eq!(
        String::from_utf8_lossy(&sent.body),
        body,
        "the upstream reads the client's bytes"
    );
    let header = |name: &str| {
        sent.headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    assert_eq!(
        header("authorization").as_deref(),
        Some(UPSTREAM_CREDENTIAL)
    );
    assert_eq!(
        header("user-agent").as_deref(),
        Some("claude-cli/9.9 (external, cli)")
    );
    assert_eq!(
        header("anthropic-beta").as_deref(),
        Some("client-capability-a,client-capability-b")
    );
    assert_eq!(header("anthropic-version").as_deref(), Some("2023-06-01"));
    assert_eq!(header("x-stainless-os").as_deref(), Some("Linux"));
    let hop_framing = [
        "host",
        "content-length",
        "content-type",
        "accept",
        "accept-encoding",
    ];
    let client_headers = [
        "authorization",
        "user-agent",
        "anthropic-beta",
        "anthropic-version",
        "x-stainless-os",
    ];
    for name in sent.headers.keys() {
        assert!(
            hop_framing.contains(&name.as_str()) || client_headers.contains(&name.as_str()),
            "the upstream received a header the client did not address to it: {name}"
        );
    }

    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(
        String::from_utf8_lossy(&returned),
        ANSWER,
        "the client reads the upstream's bytes, contradictory stop_reason included"
    );
    assert_eq!(
        headers
            .get("anthropic-ratelimit-unified-5h-utilization")
            .and_then(|v| v.to_str().ok()),
        Some("0.42")
    );
    assert_eq!(
        headers.get("request-id").and_then(|v| v.to_str().ok()),
        Some("req_upstream_own_1")
    );
    assert!(headers.get("set-cookie").is_none());
    assert!(headers.get("x-systemprompt-request-id").is_some());

    let (row_status, billing, input, output, cost) = audit_row(&pool, &cred).await;
    assert_eq!(row_status, "completed");
    assert_eq!(billing, "user_subscription");
    assert_eq!((input, output), (Some(11), Some(7)));
    assert_eq!(
        cost, 138,
        "the row prices the tokens at the catalog rate for reporting; billing_source says who paid"
    );
    Ok(())
}

#[tokio::test]
async fn a_stream_returns_byte_for_byte_and_is_metered_from_its_copy() -> Result<()> {
    let pool = test_db_pool().await;
    let (app, ctx, upstream) = app_on(&pool).await;
    let cred = member(&pool).await?;
    let marker = "marker-stream";
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains(marker))
        .respond_with(answer(200).set_body_raw(STREAM, "text/event-stream"))
        .mount(upstream)
        .await;

    let response = app
        .oneshot(post(&cred, "/messages", request_body(MODEL, marker, true)))
        .await?;
    let (status, headers, returned) = collect(response).await;
    drain(&ctx).await;

    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(
        String::from_utf8_lossy(&returned),
        STREAM,
        "the comment and the frame the decoder does not know are delivered in place"
    );
    assert_eq!(
        headers.get("content-type").and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );
    assert_eq!(
        headers.get("request-id").and_then(|v| v.to_str().ok()),
        Some("req_upstream_own_1")
    );
    let (row_status, billing, input, output, _cost) = audit_row(&pool, &cred).await;
    assert_eq!(row_status, "completed");
    assert_eq!(billing, "user_subscription");
    assert_eq!((input, output), (Some(11), Some(7)));
    Ok(())
}

#[tokio::test]
async fn a_rate_limit_is_the_upstreams_own_answer_and_is_not_retried() -> Result<()> {
    let pool = test_db_pool().await;
    let (app, _ctx, upstream) = app_on(&pool).await;
    let cred = member(&pool).await?;
    let marker = "marker-limited";
    let refusal = r#"{"type":"error","error":{"type":"rate_limit_error","message":"This request would exceed your account's rate limit."}}"#;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains(marker))
        .respond_with(
            answer(429)
                .insert_header("retry-after", "37")
                .set_body_raw(refusal, "application/json"),
        )
        .mount(upstream)
        .await;

    let response = app
        .oneshot(post(&cred, "/messages", request_body(MODEL, marker, false)))
        .await?;
    let (status, headers, returned) = collect(response).await;

    assert_eq!(received_with(upstream, marker).await.len(), 1);
    assert_eq!(status, http::StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(String::from_utf8_lossy(&returned), refusal);
    assert_eq!(
        headers.get("retry-after").and_then(|v| v.to_str().ok()),
        Some("37")
    );
    assert_eq!(
        headers
            .get("anthropic-ratelimit-unified-5h-utilization")
            .and_then(|v| v.to_str().ok()),
        Some("0.42")
    );
    Ok(())
}

#[tokio::test]
async fn another_request_format_is_refused_before_the_upstream_is_called() -> Result<()> {
    let pool = test_db_pool().await;
    let (app, _ctx, upstream) = app_on(&pool).await;
    let cred = member(&pool).await?;
    let marker = "marker-responses";
    let body = format!(r#"{{"model":"{MODEL}","input":"{marker} hi"}}"#);

    let (status, _headers, returned) =
        collect(app.oneshot(post(&cred, "/responses", body)).await?).await;

    let message = String::from_utf8_lossy(&returned).into_owned();
    assert_eq!(status, http::StatusCode::BAD_REQUEST, "{message}");
    assert!(message.contains(MODEL), "{message}");
    assert!(message.contains("anthropic"), "{message}");
    assert!(message.contains("openai.responses"), "{message}");
    assert!(received_with(upstream, marker).await.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_request_without_the_dedicated_header_is_unauthenticated_upstream() -> Result<()> {
    let pool = test_db_pool().await;
    let (app, _ctx, upstream) = app_on(&pool).await;
    let cred = member(&pool).await?;
    let marker = "marker-no-own-credential";
    let request = Request::builder()
        .method(http::Method::POST)
        .uri("/messages")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", cred.jwt.as_str()))
        .header(SESSION_ID, cred.session_id.as_str())
        .body(Body::from(request_body(MODEL, marker, false)))?;

    let (status, _headers, returned) = collect(app.oneshot(request).await?).await;

    assert_eq!(
        status,
        http::StatusCode::UNAUTHORIZED,
        "{}",
        String::from_utf8_lossy(&returned)
    );
    assert!(received_with(upstream, marker).await.is_empty());
    Ok(())
}

#[tokio::test]
async fn content_a_policy_would_redact_is_denied_rather_than_rewritten() -> Result<()> {
    let database = DisposableDb::with_schema("gw_caller_redact").await;
    let pool = database.test_pool().await;
    let (app, _ctx, upstream) = app_on(&pool).await;
    let cred = member(&pool).await?;
    sqlx::query(
        "INSERT INTO ai_gateway_policies (id, name, spec, enabled, priority) VALUES ($1, $2, $3, \
         TRUE, 100)",
    )
    .bind(format!("gwpol_{}", Uuid::new_v4().simple()))
    .bind(format!("gw-caller-redact-{}", Uuid::new_v4().simple()))
    .bind(serde_json::json!({ "safety": {
        "scanners": ["heuristic"],
        "redact_categories": ["jailbreak", "pii_email"]
    }}))
    .execute(pool.pool().as_ref())
    .await?;
    let marker = "marker-redact write to alice.redact@example.com";
    let clean = "marker-clean-control";
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains(clean))
        .respond_with(answer(200).set_body_raw(ANSWER, "application/json"))
        .mount(upstream)
        .await;

    let (status, _headers, returned) = collect(
        app.clone()
            .oneshot(post(&cred, "/messages", request_body(MODEL, marker, false)))
            .await?,
    )
    .await;
    let (control, _, _) = collect(
        app.oneshot(post(&cred, "/messages", request_body(MODEL, clean, false)))
            .await?,
    )
    .await;
    let reached = received_with(upstream, "marker-redact").await.len();
    database.drop_now().await;

    let message = String::from_utf8_lossy(&returned).into_owned();
    assert_eq!(status, http::StatusCode::BAD_REQUEST, "{message}");
    assert!(message.contains("pii_email"), "{message}");
    assert_eq!(reached, 0, "nothing is sent, redacted or not");
    assert_eq!(
        control,
        http::StatusCode::OK,
        "the control: the same policy serves a request it has nothing to redact in"
    );
    Ok(())
}

#[tokio::test]
async fn one_members_rate_limit_does_not_open_the_circuit_for_the_next() -> Result<()> {
    let pool = test_db_pool().await;
    let (app, _ctx, upstream) = app_on(&pool).await;
    let ann = member(&pool).await?;
    let ben = member(&pool).await?;
    let run = Uuid::new_v4().simple().to_string();
    let (ann_marker, ben_marker) = (format!("ann-{run}"), format!("ben-{run}"));
    Mock::given(method("POST"))
        .and(path("/a/messages"))
        .and(body_string_contains(ann_marker.as_str()))
        .respond_with(
            answer(429).set_body_raw(r#"{"error":{"message":"limit"}}"#, "application/json"),
        )
        .mount(upstream)
        .await;
    Mock::given(method("POST"))
        .and(path("/b/messages"))
        .and(body_string_contains(run.as_str()))
        .respond_with(answer(200).set_body_raw(ANSWER, "application/json"))
        .mount(upstream)
        .await;
    Mock::given(method("POST"))
        .and(path("/a/messages"))
        .and(body_string_contains(ben_marker.as_str()))
        .respond_with(answer(200).set_body_raw(ANSWER, "application/json"))
        .mount(upstream)
        .await;
    let on_a = |marker: String| async move {
        received_with(upstream, &marker)
            .await
            .iter()
            .filter(|request| request.url.path() == "/a/messages")
            .count()
    };

    for _ in 0..6 {
        let (status, _, _) = collect(
            app.clone()
                .oneshot(post(
                    &ann,
                    "/messages",
                    request_body(CHAIN_MODEL, &ann_marker, false),
                ))
                .await?,
        )
        .await;
        assert_eq!(
            status,
            http::StatusCode::OK,
            "the second endpoint serves ann"
        );
    }
    assert_eq!(
        on_a(ann_marker.clone()).await,
        5,
        "the control: ann's own circuit opened after five refusals and her sixth request skipped \
         the endpoint"
    );

    let (status, _, _) = collect(
        app.oneshot(post(
            &ben,
            "/messages",
            request_body(CHAIN_MODEL, &ben_marker, false),
        ))
        .await?,
    )
    .await;

    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(
        on_a(ben_marker).await,
        1,
        "ben is sent to the endpoint ann's refusals tripped for ann alone"
    );
    Ok(())
}
