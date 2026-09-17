//! DB-backed tests for what an in-process tool result puts on the wire once
//! the ingest has scanned it: a redacted body replaces the tool's own copy,
//! from the ingest outcome itself, and an oversized body is still scanned.

use std::sync::Arc;

use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::json;
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ContextId, McpExecutionId, SessionId, TraceId, UserId,
};
use systemprompt_mcp::{
    ArtifactIngest, ClientProfile, IngestRequest, MAX_PAYLOAD_BYTES, McpResponseBuilder,
    ToolIdentity,
};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::TextArtifact;
use systemprompt_models::auth::UserType;
use systemprompt_models::mcp::ExecutionSource;
use systemprompt_security::policy::secrets::{REDACTION_MARKER, SecretScanner};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

const PATTERNS: &str =
    "patterns:\n  - id: recovery-key\n    name: Recovery Key\n    regex: 'XRECOVERY-[0-9]+'\n";
const KEY: &str = "XRECOVERY-1234567890";

async fn db_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn scanner() -> Arc<SecretScanner> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(PATTERNS).unwrap();
    Arc::new(SecretScanner::from_policy_yaml(&yaml).unwrap())
}

fn ctx() -> RequestContext {
    let session = format!("s-{}", uuid::Uuid::new_v4().simple());
    RequestContext::new(
        SessionId::new(session.clone()),
        TraceId::new(session),
        ContextId::generate(),
        AgentName::try_new("redaction-tests").expect("valid AgentName"),
    )
    .with_actor(Actor::user(UserId::new(
        "11111111-1111-4111-8111-111111111abc",
    )))
    .with_user_type(UserType::User)
}

#[tokio::test]
async fn a_redacted_result_reaches_the_wire_without_the_secret() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, Some(scanner())).expect("ingest");
    let context = ctx();
    let exec_id = McpExecutionId::generate();

    let result = McpResponseBuilder::new(
        TextArtifact::new(format!("token {KEY} here")),
        ToolIdentity::new("systemprompt", "read_secret"),
        &context,
        &exec_id,
        &ClientProfile {
            protocol_version: Some(rmcp::model::ProtocolVersion::V_2025_06_18),
            client_name: Some("test-host".to_owned()),
            extensions: [systemprompt_models::mcp::EXTENSION_ID.to_owned()].into(),
        },
    )
    .build("summary", &ingest, "text", None)
    .await
    .expect("response builds");

    let serialized = serde_json::to_string(&result).expect("serializable");
    assert!(
        !serialized.contains(KEY),
        "the secret the scanner removed never reaches the model: {serialized}"
    );
    assert!(serialized.contains(REDACTION_MARKER));
    let structured = result.structured_content.expect("typed output on the wire");
    assert_eq!(
        structured.get("x-artifact-type").and_then(|v| v.as_str()),
        Some("text"),
        "the redacted copy is unwrapped back to the typed object"
    );
}

#[tokio::test]
async fn the_ingest_outcome_carries_the_redacted_body_so_no_read_back_is_needed() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, Some(scanner())).expect("ingest");

    let outcome = ingest
        .ingest(IngestRequest {
            result: CallToolResult::success(vec![ContentBlock::text(format!("token {KEY}"))]),
            tool_name: "Read".to_owned(),
            server_name: Some("tests".to_owned()),
            ai_tool_call_id: Some(AiToolCallId::new(format!(
                "toolu-{}",
                uuid::Uuid::new_v4().simple()
            ))),
            mcp_execution_id: None,
            ctx: ctx(),
            skill: None,
            source: ExecutionSource::HookClaudeCode,
            started_at: None,
            input: None,
        })
        .await
        .expect("ingest");

    assert_eq!(outcome.secret_redactions, 1);
    let redacted = outcome
        .redacted_body
        .expect("redacted body travels with the outcome");
    let text = redacted.to_string();
    assert!(!text.contains(KEY));
    assert!(text.contains(REDACTION_MARKER));
    assert_eq!(outcome.stored_body, redacted);

    let stored = ingest
        .artifacts()
        .find_by_id(&outcome.artifact_id)
        .await
        .unwrap()
        .expect("artifact stored");
    assert_eq!(stored.data.get("artifact"), Some(&outcome.stored_body));
}

#[tokio::test]
async fn an_oversized_body_is_scanned_before_only_its_header_is_stored() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, Some(scanner())).expect("ingest");
    let filler = "x".repeat(MAX_PAYLOAD_BYTES + 1024);

    let outcome = ingest
        .ingest(IngestRequest {
            result: CallToolResult::success(vec![ContentBlock::text(format!(
                "{filler} token {KEY}"
            ))]),
            tool_name: "Read".to_owned(),
            server_name: Some("tests".to_owned()),
            ai_tool_call_id: Some(AiToolCallId::new(format!(
                "toolu-{}",
                uuid::Uuid::new_v4().simple()
            ))),
            mcp_execution_id: None,
            ctx: ctx(),
            skill: None,
            source: ExecutionSource::HookClaudeCode,
            started_at: None,
            input: None,
        })
        .await
        .expect("ingest");

    assert_eq!(
        outcome.secret_redactions, 1,
        "the body was scanned despite its size"
    );
    assert_eq!(outcome.findings, 1);
    assert_eq!(
        outcome.stored_body.get("truncated"),
        Some(&json!(true)),
        "only the header is stored"
    );
    let redacted = outcome
        .redacted_body
        .expect("the scanned body still travels to the wire");
    let text = redacted.to_string();
    assert!(!text.contains(KEY));
    assert!(text.contains(REDACTION_MARKER));
    assert!(
        text.len() > MAX_PAYLOAD_BYTES,
        "the redacted copy is the whole body"
    );

    let findings = ingest
        .findings_repository()
        .list_for_artifact(&outcome.artifact_id)
        .await
        .unwrap();
    assert_eq!(findings.len(), 1);
}
