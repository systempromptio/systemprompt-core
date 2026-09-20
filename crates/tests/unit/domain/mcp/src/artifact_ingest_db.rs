//! DB-backed tests for the artifact ingest narrow waist: one execution and
//! one artifact however many vantage points report a call, exact keys where
//! they exist, inferred where they do not, and a body that never carries a
//! secret the scanner matched.

use std::sync::Arc;

use rmcp::model::{CallToolResult, ContentBlock, MetaObject, ResourceContents};
use serde_json::json;
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ContextId, SessionId, TraceId, UserId,
};
use systemprompt_mcp::{ArtifactIngest, IngestRequest};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::EXECUTION_META_KEY;
use systemprompt_models::auth::UserType;
use systemprompt_models::mcp::{Correlation, ExecutionSource};
use systemprompt_security::policy::secrets::{REDACTION_MARKER, SecretScanner};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

const PATTERNS: &str =
    "patterns:\n  - id: recovery-key\n    name: Recovery Key\n    regex: 'XRECOVERY-[0-9]+'\n";
const KEY: &str = "XRECOVERY-1234567890";

async fn db_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

fn scanner() -> Arc<SecretScanner> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(PATTERNS).unwrap();
    Arc::new(SecretScanner::from_policy_yaml(&yaml).unwrap())
}

fn ctx(session: &str) -> RequestContext {
    RequestContext::new(
        SessionId::new(session),
        TraceId::new(session),
        ContextId::generate(),
        AgentName::try_new("ingest-tests").expect("valid AgentName"),
    )
    .with_actor(Actor::user(UserId::new(
        "11111111-1111-4111-8111-111111111abc",
    )))
    .with_user_type(UserType::User)
}

fn request(
    result: CallToolResult,
    session: &str,
    call: Option<&AiToolCallId>,
    source: ExecutionSource,
) -> IngestRequest {
    IngestRequest {
        result,
        tool_name: "Read".to_owned(),
        server_name: Some("tests".to_owned()),
        ai_tool_call_id: call.cloned(),
        mcp_execution_id: None,
        ctx: ctx(session),
        skill: None,
        source,
        started_at: None,
        input: Some(json!({"file_path": "/x"})),
    }
}

#[tokio::test]
async fn a_call_seen_from_two_vantage_points_is_one_execution_and_one_artifact() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, None).expect("ingest");
    let session = unique("sess");
    let call = AiToolCallId::new(unique("toolu"));

    let mut structured = CallToolResult::success(vec![ContentBlock::text("ok")]);
    structured.structured_content = Some(json!({"rows": 2}));
    let first = ingest
        .ingest(request(
            structured,
            &session,
            Some(&call),
            ExecutionSource::Gateway,
        ))
        .await
        .expect("first ingest");
    assert!(first.created);
    assert!(first.is_structured);
    assert_eq!(first.correlation, Correlation::Exact);

    let hook = CallToolResult::success(vec![ContentBlock::text("ok")]);
    let second = ingest
        .ingest(request(
            hook,
            &session,
            Some(&call),
            ExecutionSource::HookClaudeCode,
        ))
        .await
        .expect("second ingest");
    assert!(!second.created, "the hook enriches the existing artifact");
    assert_eq!(second.mcp_execution_id, first.mcp_execution_id);
    assert_eq!(second.artifact_id, first.artifact_id);

    let stored = ingest
        .artifacts()
        .find_by_id(&first.artifact_id)
        .await
        .unwrap()
        .expect("artifact stored");
    assert_eq!(stored.source(), ExecutionSource::Gateway);
    assert_eq!(stored.last_seen_source.as_deref(), Some("hook_claude_code"));
    assert_eq!(stored.ai_tool_call_id, Some(call));
    assert!(
        stored.is_structured,
        "the hook copy never downgrades the body"
    );
}

#[tokio::test]
async fn a_server_execution_key_in_meta_joins_exactly() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, None).expect("ingest");
    let session = unique("sess");
    let call = AiToolCallId::new(unique("toolu"));

    let first = ingest
        .ingest(request(
            CallToolResult::success(vec![ContentBlock::text("proxied")]),
            &session,
            Some(&call),
            ExecutionSource::Proxy,
        ))
        .await
        .expect("proxy ingest");

    let mut hook = CallToolResult::success(vec![ContentBlock::text("proxied")]);
    let mut meta = serde_json::Map::new();
    meta.insert(
        EXECUTION_META_KEY.to_owned(),
        json!({"mcp_execution_id": first.mcp_execution_id.as_str()}),
    );
    hook.meta = Some(MetaObject(meta));
    let second = ingest
        .ingest(request(hook, &session, None, ExecutionSource::HookOpenCode))
        .await
        .expect("hook ingest");
    assert!(!second.created);
    assert_eq!(second.correlation, Correlation::Exact);
    assert_eq!(second.mcp_execution_id, first.mcp_execution_id);
}

#[tokio::test]
async fn a_hook_with_no_key_is_matched_by_fingerprint_and_marked_inferred() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, None).expect("ingest");
    let session = unique("sess");
    let body = unique("same-body");

    let first = ingest
        .ingest(request(
            CallToolResult::success(vec![ContentBlock::text(body.clone())]),
            &session,
            None,
            ExecutionSource::Proxy,
        ))
        .await
        .expect("proxy ingest");
    let second = ingest
        .ingest(request(
            CallToolResult::success(vec![ContentBlock::text(body)]),
            &session,
            None,
            ExecutionSource::HookClaudeCode,
        ))
        .await
        .expect("hook ingest");
    assert!(!second.created);
    assert_eq!(second.mcp_execution_id, first.mcp_execution_id);
    assert_eq!(second.correlation, Correlation::Inferred);
}

#[tokio::test]
async fn a_secret_in_the_result_is_redacted_before_the_body_is_stored() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, Some(scanner())).expect("ingest");
    let session = unique("sess");
    let call = AiToolCallId::new(unique("toolu"));

    let outcome = ingest
        .ingest(request(
            CallToolResult::success(vec![ContentBlock::text(format!("token {KEY} here"))]),
            &session,
            Some(&call),
            ExecutionSource::HookClaudeCode,
        ))
        .await
        .expect("ingest");
    assert_eq!(outcome.findings, 1);

    let stored = ingest
        .artifacts()
        .find_by_id(&outcome.artifact_id)
        .await
        .unwrap()
        .expect("artifact stored");
    let serialized = stored.data.to_string();
    assert!(
        !serialized.contains(KEY),
        "the secret never reaches the row"
    );
    assert!(serialized.contains(REDACTION_MARKER));
    assert_eq!(stored.secret_redactions, 1);

    let payload = ingest
        .payloads()
        .find_payload(stored.payload_sha256.as_deref().expect("digest"))
        .await
        .unwrap()
        .expect("payload stored");
    assert!(!payload.body.to_string().contains(KEY));

    let findings = ingest
        .findings_repository()
        .list_for_artifact(&outcome.artifact_id)
        .await
        .unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].category, "secret");
    assert!(findings[0].redacted);
}

#[tokio::test]
async fn an_identical_body_is_stored_once_by_digest() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, None).expect("ingest");
    let body = unique("shared-body");
    let mut ids = Vec::new();
    for _ in 0..2 {
        let outcome = ingest
            .ingest(request(
                CallToolResult::success(vec![ContentBlock::text(body.clone())]),
                &unique("sess"),
                Some(&AiToolCallId::new(unique("toolu"))),
                ExecutionSource::Gateway,
            ))
            .await
            .expect("ingest");
        assert!(outcome.created);
        ids.push(outcome.artifact_id);
    }
    let a = ingest
        .artifacts()
        .find_by_id(&ids[0])
        .await
        .unwrap()
        .unwrap();
    let b = ingest
        .artifacts()
        .find_by_id(&ids[1])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(a.payload_sha256, b.payload_sha256);
    let payload = ingest
        .payloads()
        .find_payload(a.payload_sha256.as_deref().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payload.ref_count, 2);
}

#[tokio::test]
async fn typed_error_result_keeps_declared_shape_title_ui_resource_and_client_artifact_id() {
    let db = db_or_skip()
        .await
        .expect("artifact ingest fixture database");
    let ingest = ArtifactIngest::from_db(&db, None).expect("ingest");
    let artifact_id = unique("artifact");
    let mut result = CallToolResult::error(vec![
        ContentBlock::text("tool reported an error"),
        ContentBlock::image("aGVsbG8=", "image/png"),
        ContentBlock::audio("YXVkaW8=", "audio/wav"),
        ContentBlock::resource(ResourceContents::TextResourceContents {
            uri: "ui://tests/result".to_owned(),
            mime_type: Some("text/html".to_owned()),
            text: "<strong>result</strong>".to_owned(),
            meta: None,
        }),
    ]);
    result.structured_content = Some(json!({
        "x-artifact-type": "vendor_report",
        "title": "Gateway report",
        "status": "degraded"
    }));
    let mut meta = serde_json::Map::new();
    meta.insert(
        EXECUTION_META_KEY.to_owned(),
        json!({"artifact_id": artifact_id.clone()}),
    );
    result.meta = Some(MetaObject(meta));

    let outcome = ingest
        .ingest(request(
            result,
            &unique("sess"),
            None,
            ExecutionSource::Gateway,
        ))
        .await
        .expect("typed tool result ingests");

    assert!(outcome.created);
    assert!(outcome.is_structured);
    assert_eq!(outcome.artifact_id.as_str(), artifact_id);
    assert_eq!(outcome.stored_body["x-artifact-type"], "vendor_report");
    assert_eq!(outcome.stored_body["title"], "Gateway report");
    let stored = ingest
        .artifacts()
        .find_by_id(&outcome.artifact_id)
        .await
        .unwrap()
        .expect("typed artifact stored");
    assert_eq!(stored.artifact_type, "vendor_report");
    assert_eq!(stored.title.as_deref(), Some("Gateway report"));
    assert!(stored.is_structured);
    assert!(stored.has_ui_resource);
    assert!(stored.is_error);
}
