use std::sync::Arc;

use rmcp::ErrorData as McpError;
use serde_json::json;
use systemprompt_brain::BrainError;
use systemprompt_brain::BrainRepositories;
use systemprompt_brain::service::BrainService;
use systemprompt_database::Database;
use systemprompt_extension::brain::{
    BrainSink, BrainSinkError, MemoryRecall, RecallError, RecallOutcome, RecallReceipt,
    RecallRequest, SkipReason,
};
use systemprompt_identifiers::{BrainFactId, EventOutboxId, FactKindId};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{BrainEvent, FactStatus, ScopeLevel};
use systemprompt_runtime::brain::mcp::{
    BrainMcpTools, BrainMcpUnavailable, BrainRefusalCode, DisputeInput, GuideInput, ProposeInput,
    RecallPreviewInput, SearchInput, WhyInput, brain_tool_error, guide_text, refusal_code,
    refusal_of, refusal_result,
};

struct NoSink;

#[async_trait::async_trait]
impl BrainSink for NoSink {
    fn is_available(&self) -> bool {
        false
    }

    async fn emit(
        &self,
        _conn: &mut sqlx::PgConnection,
        _event: &BrainEvent,
    ) -> Result<EventOutboxId, BrainSinkError> {
        Err(BrainSinkError::Unavailable)
    }
}

struct NoRecall;

#[async_trait::async_trait]
impl MemoryRecall for NoRecall {
    fn name(&self) -> &'static str {
        "none"
    }

    async fn recall(&self, _request: &RecallRequest) -> Result<RecallOutcome, RecallError> {
        Ok(RecallOutcome::Skipped {
            reason: SkipReason::NoFacts,
        })
    }

    async fn record(&self, _receipt: &RecallReceipt) -> Result<(), RecallError> {
        Ok(())
    }
}

fn offline_service(profile: &Arc<BrainProfile>) -> Arc<BrainService> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://offline:offline@127.0.0.1:1/offline")
        .unwrap();
    let db = Arc::new(Database::from_pools(Arc::new(pool), None));
    let repos = BrainRepositories::new(&db);
    Arc::new(BrainService::builder(repos, Arc::new(NoSink), Arc::clone(profile)).build())
}

fn build(enabled: bool) -> Result<BrainMcpTools, BrainMcpUnavailable> {
    let profile = Arc::new(BrainProfile {
        enabled,
        ..BrainProfile::default()
    });
    let service = offline_service(&profile);
    BrainMcpTools::from_parts(service, Arc::new(NoRecall), profile)
}

#[tokio::test]
async fn from_parts_refuses_a_disabled_brain_naming_the_switch() {
    let error = build(false).unwrap_err();
    assert!(error.to_string().contains("brain.enabled"));
}

#[tokio::test]
async fn from_parts_builds_when_enabled_and_lists_six_terse_tools() {
    let tools = build(true).unwrap();
    let server = systemprompt_identifiers::McpServerId::new("brain");
    let listed = tools.list_tools(&server);
    let names: Vec<String> = listed.iter().map(|tool| tool.name.to_string()).collect();
    assert_eq!(names, BrainMcpTools::tool_names());
    for tool in &listed {
        assert!(tool.output_schema.is_none());
        assert!(!tool.input_schema.contains_key("$schema"));
        assert!(tool.description.as_deref().unwrap_or_default().len() < 80);
    }
}

#[test]
fn search_input_reads_every_optional_field() {
    let input: SearchInput = serde_json::from_value(json!({
        "query": "billing", "kind": "decision", "subject": "repo:core",
        "scope": "project", "limit": 5
    }))
    .unwrap();
    assert_eq!(input.kind, Some(FactKindId::new("decision")));
    assert_eq!(input.scope, Some(ScopeLevel::Project));
    assert_eq!(input.limit, Some(5));
}

#[test]
fn every_input_refuses_an_unknown_key() {
    let extra = |mut value: serde_json::Value| {
        value["confirm"] = json!(true);
        value
    };
    assert!(serde_json::from_value::<SearchInput>(extra(json!({"query": "x"}))).is_err());
    assert!(
        serde_json::from_value::<ProposeInput>(extra(
            json!({"kind": "decision", "subject": "repo:core", "fields": {}})
        ))
        .is_err()
    );
    assert!(
        serde_json::from_value::<DisputeInput>(extra(
            json!({"fact_id": BrainFactId::generate(), "reason": "wrong"})
        ))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WhyInput>(extra(json!({"fact_id": BrainFactId::generate()})))
            .is_err()
    );
    assert!(serde_json::from_value::<RecallPreviewInput>(extra(json!({"prompt": "x"}))).is_err());
    assert!(serde_json::from_value::<GuideInput>(extra(json!({}))).is_err());
}

#[test]
fn propose_input_carries_a_requested_status_for_refusal() {
    let input: ProposeInput = serde_json::from_value(json!({
        "kind": "decision", "subject": "repo:core", "fields": {"text": "x"},
        "status": "confirmed"
    }))
    .unwrap();
    assert_eq!(input.status, Some(FactStatus::Confirmed));
    assert!(input.scope.is_none());
}

#[test]
fn brain_errors_map_to_refusal_codes() {
    let id = BrainFactId::generate();
    assert_eq!(
        refusal_of(&BrainError::UnknownKind(FactKindId::new("nope"))),
        Some(BrainRefusalCode::UnknownKind)
    );
    assert_eq!(refusal_of(&BrainError::Tombstoned), Some(BrainRefusalCode::Tombstoned));
    assert_eq!(
        refusal_of(&BrainError::Forbidden {
            reason: "no".to_owned()
        }),
        Some(BrainRefusalCode::Forbidden)
    );
    assert_eq!(
        refusal_of(&BrainError::NotFound(id)),
        Some(BrainRefusalCode::NotFound)
    );
    assert_eq!(
        refusal_of(&BrainError::Sink(BrainSinkError::Unavailable)),
        None
    );
}

#[test]
fn a_refused_error_becomes_an_is_error_result_with_its_code() {
    let error = brain_tool_error(BrainError::Tombstoned);
    assert_eq!(refusal_code(&error), Some(BrainRefusalCode::Tombstoned));
    let result = refusal_result(&error).unwrap();
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content,
        Some(json!({ "code": "tombstoned" }))
    );
}

#[test]
fn a_transient_failure_is_a_protocol_error_not_a_refusal() {
    let error = brain_tool_error(BrainError::Sink(BrainSinkError::Unavailable));
    assert!(refusal_result(&error).is_none());
    let untagged = McpError::internal_error("boom", None);
    assert!(refusal_code(&untagged).is_none());
}

#[test]
fn the_guide_names_every_refusal_code_and_no_other() {
    let guide = guide_text(&[]);
    for code in BrainRefusalCode::ALL {
        assert!(guide.contains(code.as_str()), "{} missing", code.as_str());
    }
    assert!(!guide.contains("stale_base"));
    assert!(guide.contains("attached automatically"));
}
