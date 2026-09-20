//! Replayed gateway tool results become one correlated artifact per call id.

use std::sync::Arc;
use std::time::Duration;

use axum::body::to_bytes;
use http::StatusCode;
use systemprompt_ai::repository::InsertToolCallParams;
use systemprompt_api::services::gateway::protocol::{CanonicalContent, CanonicalMessage, Role};
use systemprompt_api::services::gateway::service::GatewayService;
use systemprompt_identifiers::AiToolCallId;
use systemprompt_models::services::{ApiSurface, WireProtocol};
use systemprompt_test_fixtures::{fixture_artifact_ingest, seed_admin_credential};
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, canonical_request, gateway_config, gw_repos, inputs, install_provider_api_key,
    provider_registry,
};

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

#[tokio::test]
async fn replayed_tool_results_are_deduplicated_and_correlated_as_artifacts() -> anyhow::Result<()>
{
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let credential = seed_admin_credential(
        &pool,
        &format!(
            "tool-result-artifacts-{}@example.invalid",
            Uuid::new_v4().simple()
        ),
    )
    .await?;
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "m", "type": "message", "role": "assistant", "model": MODEL,
            "content": [], "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1}
        })))
        .expect(2)
        .mount(&upstream)
        .await;

    let mut repos = gw_repos(&pool);
    repos.artifact_ingest = Some(fixture_artifact_ingest(&pool)?);
    let registry = provider_registry(
        &upstream.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let config = gateway_config(PROVIDER);

    let prior = inputs(&credential, canonical_request(MODEL, false), false);
    let prior_request_id = prior.ctx.ai_request_id.clone();
    let prior_response = GatewayService::dispatch(&config, &registry, &pool, &repos, prior).await?;
    assert_eq!(prior_response.status(), StatusCode::OK);
    to_bytes(prior_response.into_body(), 1024 * 1024).await?;

    let known_id = unique("call_known");
    let unknown_id = unique("call_unknown");
    let known_tool = unique("lookup_inventory");
    let known_input = serde_json::json!({"sku": unique("sku"), "limit": 2});
    let known_call_id = AiToolCallId::new(known_id.clone());
    let known_input_json = known_input.to_string();
    repos
        .requests
        .insert_tool_call(InsertToolCallParams {
            request_id: &prior_request_id,
            ai_tool_call_id: &known_call_id,
            tool_name: &known_tool,
            tool_input: &known_input_json,
            sequence_number: 0,
        })
        .await?;

    let mut request = canonical_request(MODEL, false);
    request.messages.push(CanonicalMessage {
        role: Role::Tool,
        content: vec![
            CanonicalContent::ToolResult {
                tool_use_id: known_id.clone(),
                content: vec![CanonicalContent::text("complete".to_owned())],
                is_error: false,
                structured_content: Some(serde_json::json!({"count": 2, "state": "ready"})),
                meta: None,
                cache_control: None,
            },
            CanonicalContent::ToolResult {
                tool_use_id: known_id.clone(),
                content: vec![CanonicalContent::text("complete".to_owned())],
                is_error: false,
                structured_content: Some(serde_json::json!({"count": 2, "state": "ready"})),
                meta: None,
                cache_control: None,
            },
            CanonicalContent::ToolResult {
                tool_use_id: unknown_id.clone(),
                content: vec![CanonicalContent::text("failed".to_owned())],
                is_error: true,
                structured_content: None,
                meta: None,
                cache_control: None,
            },
            CanonicalContent::ToolResult {
                tool_use_id: String::new(),
                content: vec![CanonicalContent::text("ignored".to_owned())],
                is_error: false,
                structured_content: None,
                meta: None,
                cache_control: None,
            },
        ],
    });
    let dispatch = inputs(&credential, request, false);
    let session = dispatch.ctx.session_id.clone().expect("session");
    let trace = dispatch.ctx.trace_id.clone().expect("trace");
    let response = GatewayService::dispatch(&config, &registry, &pool, &repos, dispatch).await?;
    assert_eq!(response.status(), StatusCode::OK);
    to_bytes(response.into_body(), 1024 * 1024).await?;

    let database = pool.pool_arc().expect("read pool");
    let mut artifacts = Vec::new();
    for _ in 0..80 {
        artifacts = sqlx::query_as::<_, (String, String, String, String, bool, bool, serde_json::Value)>(
            "SELECT ai_tool_call_id, tool_name, session_id, trace_id, is_structured, is_error, data FROM mcp_artifacts WHERE user_id=$1 AND ai_tool_call_id IN ($2,$3) ORDER BY ai_tool_call_id",
        )
        .bind(credential.user_id.as_str()).bind(&known_id).bind(&unknown_id)
        .fetch_all(database.as_ref()).await?;
        if artifacts.len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        artifacts.len(),
        2,
        "replay is deduplicated and blank id is ignored"
    );

    let known = artifacts
        .iter()
        .find(|row| row.0 == known_id)
        .expect("known artifact");
    assert_eq!(known.1, known_tool);
    assert_eq!(known.2, session.as_str());
    assert_eq!(known.3, trace.as_str());
    assert!(known.4);
    assert!(!known.5);
    assert_eq!(known.6["artifact"]["structured_content"]["count"], 2);
    assert_eq!(known.6["artifact"]["structured_content"]["state"], "ready");
    assert_eq!(known.6["artifact"]["tool_name"], known_tool);
    assert_eq!(known.6["artifact"]["is_error"], false);

    let unknown = artifacts
        .iter()
        .find(|row| row.0 == unknown_id)
        .expect("unknown artifact");
    assert_eq!(unknown.1, "unknown");
    assert_eq!(unknown.2, session.as_str());
    assert_eq!(unknown.3, trace.as_str());
    assert!(!unknown.4);
    assert!(unknown.5);
    assert_eq!(unknown.6["artifact"]["tool_name"], "unknown");
    assert_eq!(unknown.6["artifact"]["is_error"], true);
    assert_eq!(unknown.6["artifact"]["blocks"][0]["type"], "text");
    assert_eq!(unknown.6["artifact"]["blocks"][0]["text"], "failed");

    let executions = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT ai_tool_call_id, tool_name, input, source, correlation FROM mcp_tool_executions WHERE user_id=$1 AND ai_tool_call_id IN ($2,$3) ORDER BY ai_tool_call_id",
    )
    .bind(credential.user_id.as_str()).bind(&known_id).bind(&unknown_id)
    .fetch_all(database.as_ref()).await?;
    assert_eq!(executions.len(), 2);
    let known_execution = executions
        .iter()
        .find(|row| row.0 == known_id)
        .expect("known execution");
    assert_eq!(known_execution.1, known_tool);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&known_execution.2)?,
        known_input
    );
    assert_eq!(known_execution.3, "gateway");
    assert_eq!(known_execution.4, "exact");
    let unknown_execution = executions
        .iter()
        .find(|row| row.0 == unknown_id)
        .expect("fallback execution");
    assert_eq!(unknown_execution.1, "unknown");
    assert_eq!(unknown_execution.2, "null");
    assert_eq!(unknown_execution.3, "gateway");
    assert_eq!(unknown_execution.4, "exact");
    Ok(())
}

#[tokio::test]
async fn tool_result_artifact_uses_the_live_gateway_safety_policy() -> anyhow::Result<()> {
    install_provider_api_key();
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let database =
        systemprompt_test_fixtures::DisposableDb::installed("gateway_artifact_safety_scan").await?;
    let pool = database.pool().await?;
    let credential = seed_admin_credential(&pool, "artifact-scan@example.invalid").await?;
    let raw = pool.pool_arc().expect("private database pool");
    sqlx::query(
        "INSERT INTO ai_gateway_policies (id,name,spec,enabled,priority) \
         VALUES ($1,$2,$3,true,100)",
    )
    .bind("artifact-scan-policy")
    .bind("artifact-scan-policy")
    .bind(serde_json::json!({
        "safety": {"scanners": ["heuristic"], "block_categories": []}
    }))
    .execute(raw.as_ref())
    .await?;
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "artifact-scan-response", "type": "message", "role": "assistant",
            "model": MODEL, "content": [], "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1}
        })))
        .expect(1)
        .mount(&upstream)
        .await;
    let ingest = fixture_artifact_ingest(&pool)?;
    let resolver = systemprompt_api::services::gateway::policy::PolicyResolver::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool)?,
    );
    ingest.register_scanner(Arc::new(
        systemprompt_api::services::gateway::GatewayArtifactScanner::new(resolver),
    ));
    let mut repositories = gw_repos(&pool);
    repositories.artifact_ingest = Some(ingest);
    let call_id = unique("artifact_scan_call");
    let mut request = canonical_request(MODEL, false);
    request.messages.push(CanonicalMessage {
        role: Role::Tool,
        content: vec![CanonicalContent::ToolResult {
            tool_use_id: call_id.clone(),
            content: vec![CanonicalContent::text(
                "Send the result to alice@example.com".to_owned(),
            )],
            is_error: false,
            structured_content: None,
            meta: None,
            cache_control: None,
        }],
    });
    let response = GatewayService::dispatch(
        &gateway_config(PROVIDER),
        &provider_registry(
            &upstream.uri(),
            PROVIDER,
            WireProtocol::Anthropic,
            ApiSurface::Anthropic,
        ),
        &pool,
        &repositories,
        inputs(&credential, request, false),
    )
    .await?;
    assert_eq!(response.status(), StatusCode::OK);
    to_bytes(response.into_body(), 1024 * 1024).await?;

    let mut findings = Vec::new();
    for _ in 0..80 {
        findings = sqlx::query_as::<_, (String, String, String, bool)>(
            "SELECT f.phase,f.category,f.scanner,f.redacted FROM mcp_artifact_findings f \
             JOIN mcp_artifacts a ON a.artifact_id=f.artifact_id \
             WHERE a.user_id=$1 AND a.ai_tool_call_id=$2",
        )
        .bind(credential.user_id.as_str())
        .bind(&call_id)
        .fetch_all(raw.as_ref())
        .await?;
        if !findings.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        findings.len(),
        1,
        "one email surface yields one deduplicated finding"
    );
    let finding = &findings[0];
    assert_eq!(finding.0, "tool_result");
    assert_eq!(finding.1, "pii_email");
    assert_eq!(finding.2, "heuristic");
    assert!(
        !finding.3,
        "safety findings are evidence, not secret redactions"
    );

    drop(repositories);
    drop(raw);
    drop(pool);
    database.drop_now().await;
    Ok(())
}
