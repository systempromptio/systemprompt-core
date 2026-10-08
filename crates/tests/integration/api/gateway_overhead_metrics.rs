//! The gateway latency histograms on a real dispatch.
//!
//! `GatewayAudit::complete` is the single sink for the buffered and the
//! streamed path, so one completed request of each kind must add exactly one
//! sample to `gateway_upstream_duration_seconds` and one to
//! `gateway_overhead_seconds`, rendered with the explicit bucket bounds the
//! recorder installs.

use axum::body::to_bytes;
use systemprompt_api::services::server::metrics::install_recorder;
use systemprompt_gateway::service::GatewayService;
use systemprompt_identifiers::InstanceId;
use systemprompt_models::providers::ApiSurface;
use systemprompt_test_fixtures::seed_admin_credential;
use systemprompt_wire::WireProtocol;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, buffered_response_json, canonical_request, gateway_config, gw_repos, inputs,
    install_provider_api_key, provider_registry, streaming_sse_body,
};

const DRAIN: std::time::Duration = std::time::Duration::from_secs(30);

fn sample_count(rendered: &str, metric: &str) -> u64 {
    rendered
        .lines()
        .filter(|line| line.starts_with(&format!("{metric}_count{{")))
        .filter(|line| line.contains("route=\"anthropic.messages\""))
        .filter_map(|line| line.rsplit(' ').next()?.parse::<u64>().ok())
        .sum()
}

async fn dispatch_once(label: &str, stream: bool) -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, &format!("gw-metrics-{label}@example.invalid")).await?;

    let upstream = MockServer::start().await;
    let template = if stream {
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_raw(streaming_sse_body(), "text/event-stream")
    } else {
        ResponseTemplate::new(200).set_body_json(buffered_response_json())
    };
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(template)
        .mount(&upstream)
        .await;

    let registry = provider_registry(
        &upstream.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let repos = gw_repos(&pool);
    let resp = GatewayService::dispatch(
        &gateway_config(PROVIDER),
        &registry,
        &pool,
        &repos,
        inputs(&cred, canonical_request(MODEL, stream), stream),
    )
    .await
    .map_err(|e| anyhow::anyhow!("dispatch failed: {e:?}"))?;
    assert_eq!(resp.status(), http::StatusCode::OK);
    to_bytes(resp.into_body(), 4 * 1024 * 1024).await?;
    assert_eq!(
        repos.background.drain(DRAIN).await,
        systemprompt_traits::DrainOutcome::Drained
    );
    Ok(())
}

#[tokio::test]
async fn a_buffered_completion_records_one_overhead_and_one_upstream_sample() -> anyhow::Result<()>
{
    let handle = install_recorder(&InstanceId::new("metrics-fixture"))?;
    let before = sample_count(&handle.render(), "gateway_overhead_seconds");

    dispatch_once("buffered", false).await?;

    let rendered = handle.render();
    assert_eq!(
        sample_count(&rendered, "gateway_overhead_seconds"),
        before + 1,
        "{rendered}"
    );
    assert_eq!(
        sample_count(&rendered, "gateway_upstream_duration_seconds"),
        before + 1,
        "{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("gateway_overhead_seconds_bucket{")
                && line.contains("le=\"0.001\"")
                && line.contains(&format!("provider=\"{PROVIDER}\""))),
        "the overhead histogram renders the explicit millisecond buckets: {rendered}"
    );
    assert!(
        rendered.lines().any(
            |line| line.starts_with("gateway_upstream_duration_seconds_bucket{")
                && line.contains("le=\"120\"")
        ),
        "the upstream histogram renders its two-minute top bucket: {rendered}"
    );
    Ok(())
}

#[tokio::test]
async fn a_streamed_completion_records_its_sample_once_the_tap_finalises() -> anyhow::Result<()> {
    let handle = install_recorder(&InstanceId::new("metrics-fixture"))?;
    let before = sample_count(&handle.render(), "gateway_upstream_duration_seconds");

    dispatch_once("streamed", true).await?;

    let rendered = handle.render();
    assert_eq!(
        sample_count(&rendered, "gateway_upstream_duration_seconds"),
        before + 1,
        "{rendered}"
    );
    assert_eq!(
        sample_count(&rendered, "gateway_overhead_seconds"),
        before + 1,
        "{rendered}"
    );
    Ok(())
}
