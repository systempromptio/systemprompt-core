//! `/otel` with a real gateway credential: the authenticated handler accepts
//! a bound session and refuses a header that contradicts the JWT claim, and
//! each decoded signal (protobuf traces, logs and metrics, JSON logs) is
//! counted under its own label on `gateway_desktop_telemetry_exports_total`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue, any_value};
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::metrics::v1::{Metric, ResourceMetrics, ScopeMetrics};
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
use prost::Message;
use systemprompt_api::routes::gateway::otel::{handle, ingest_envelope};
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_api::services::server::metrics::install_recorder;
use systemprompt_identifiers::{InstanceId, SessionId};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, seed_bridge_credential, test_app_context,
    test_database_url, test_db_pool,
};
use systemprompt_traits::AppContext as _;

const EXPORTS: &str = "gateway_desktop_telemetry_exports_total";

struct Harness {
    ctx: Arc<AppContext>,
    extractor: Arc<JwtContextExtractor>,
    authed: AuthedFixture,
}

async fn harness() -> Harness {
    let url = test_database_url();
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&pool, &url);
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.session_provider().expect("session provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(
            ctx.oauth_repositories().oauth.clone(),
            systemprompt_traits::CacheEpochs::new(),
        ),
        ctx.config().jwt_issuer.clone(),
    ));
    let authed = seed_bridge_credential(&pool, "otel@gateway-otel.invalid")
        .await
        .expect("bridge credential");
    Harness {
        ctx,
        extractor,
        authed,
    }
}

fn exports(signal: &str) -> f64 {
    let rendered = install_recorder(&InstanceId::new("fixture-instance"), &[])
        .expect("recorder installs")
        .render();
    rendered
        .lines()
        .filter(|line| line.starts_with(EXPORTS) && line.contains(&format!("signal=\"{signal}\"")))
        .filter_map(|line| line.rsplit(' ').next()?.parse::<f64>().ok())
        .sum()
}

fn spoofed_resource() -> Option<Resource> {
    Some(Resource {
        attributes: vec![KeyValue {
            key: "systemprompt.user.id".to_owned(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue("spoofed".to_owned())),
            }),
            ..Default::default()
        }],
        ..Default::default()
    })
}

fn traces() -> Vec<u8> {
    ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: spoofed_resource(),
            scope_spans: vec![ScopeSpans {
                spans: vec![Span {
                    name: "authed-span".to_owned(),
                    trace_id: vec![1; 16],
                    span_id: vec![2; 8],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
    }
    .encode_to_vec()
}

fn logs() -> Vec<u8> {
    ExportLogsServiceRequest {
        resource_logs: vec![ResourceLogs {
            resource: spoofed_resource(),
            scope_logs: vec![ScopeLogs {
                log_records: vec![LogRecord::default()],
                ..Default::default()
            }],
            ..Default::default()
        }],
    }
    .encode_to_vec()
}

fn metrics_export() -> Vec<u8> {
    ExportMetricsServiceRequest {
        resource_metrics: vec![ResourceMetrics {
            scope_metrics: vec![ScopeMetrics {
                metrics: vec![Metric {
                    name: "claude_code.token.usage".to_owned(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
    }
    .encode_to_vec()
}

fn post(uri: &str, content_type: &str) -> axum::http::request::Builder {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", content_type)
}

fn authed(h: &Harness, uri: &str, session: &SessionId, body: Vec<u8>) -> Request<Body> {
    post(uri, "application/x-protobuf")
        .header("authorization", format!("Bearer {}", h.authed.jwt.as_str()))
        .header("x-session-id", session.as_str())
        .body(Body::from(body))
        .expect("request")
}

#[tokio::test]
async fn an_attested_session_is_accepted_and_each_protobuf_signal_is_counted() {
    let h = harness().await;
    let session = h.authed.session_id.clone();
    for (uri, body, signal) in [
        ("/otel/v1/traces", traces(), "traces"),
        ("/otel/v1/logs", logs(), "logs"),
        ("/otel/v1/metrics", metrics_export(), "metrics"),
    ] {
        let before = exports(signal);
        let resp = handle(
            Arc::clone(&h.extractor),
            (*h.ctx).clone(),
            authed(&h, uri, &session, body),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::ACCEPTED, "{uri}");
        assert!(
            exports(signal) > before,
            "{signal} export decoded and counted"
        );
    }
}

#[tokio::test]
async fn a_session_header_the_credential_does_not_attest_is_refused() {
    let h = harness().await;
    let resp = handle(
        Arc::clone(&h.extractor),
        (*h.ctx).clone(),
        authed(&h, "/otel/v1/traces", &SessionId::generate(), traces()),
    )
    .await;
    assert!(
        resp.status().is_client_error(),
        "a mismatched X-Session-ID must not be written: {}",
        resp.status()
    );
}

#[tokio::test]
async fn a_jwt_without_a_session_header_is_bound_to_its_claim() {
    let h = harness().await;
    let request = post("/otel/v1/traces", "application/x-protobuf")
        .header("authorization", format!("Bearer {}", h.authed.jwt.as_str()))
        .body(Body::from(traces()))
        .expect("request");
    let resp = handle(Arc::clone(&h.extractor), (*h.ctx).clone(), request).await;
    assert_eq!(
        resp.status(),
        StatusCode::ACCEPTED,
        "the JWT's session claim binds the request when no header is sent"
    );
}

#[tokio::test]
async fn a_json_log_export_is_decoded_and_counted() {
    let before = exports("logs");
    let body = serde_json::json!({
        "resourceLogs": [{
            "resource": {"attributes": [{"key": "service.name", "value": {"stringValue": "desktop"}}]},
            "scopeLogs": [{"logRecords": [{"timeUnixNano": "1"}]}]
        }]
    });
    let request = post("/v1/logs", "application/json; charset=utf-8")
        .body(Body::from(body.to_string()))
        .expect("request");
    let resp = ingest_envelope(
        &systemprompt_test_fixtures::fixture_logging_runtime(),
        request,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert!(exports("logs") > before, "a JSON log export is ingested");
}
