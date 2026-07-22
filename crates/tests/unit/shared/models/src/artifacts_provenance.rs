//! Unit tests for artifact builders and execution provenance metadata.

use systemprompt_identifiers::{AgentName, ContextId, SessionId, SkillId, TraceId};
use systemprompt_models::artifacts::{
    Artifact, CliArtifact, CopyPasteTextArtifact, ExecutionMetadata, ResearchArtifact,
    SourceCitation, TableArtifact, TextArtifact,
};
use systemprompt_models::execution::RequestContext;

const CTX: &str = "00000000-0000-4000-8000-0000000000a1";

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("sess-1"),
        TraceId::new("trace-1"),
        ContextId::new(CTX),
        AgentName::new("agent_one"),
    )
}

#[test]
fn table_to_response_reports_count_and_execution_id() {
    let table = TableArtifact::new(vec![])
        .with_rows(vec![
            serde_json::json!({"a": 1}),
            serde_json::json!({"a": 2}),
        ])
        .with_execution_id("exec-9");

    let response = table.to_response();
    assert_eq!(response["x-artifact-type"], "table");
    assert_eq!(response["count"], 2);
    assert_eq!(response["items"][1]["a"], 2);
    assert_eq!(response["execution_id"], "exec-9");
    assert!(response.get("hints").is_some());
}

#[test]
fn table_with_request_and_skill_populate_metadata_via_response() {
    let table = TableArtifact::new(vec![])
        .with_request(&ctx())
        .with_skill(SkillId::new("skill-x"), "Skill X");

    let response = table.to_response();
    assert!(
        response.get("execution_id").is_none(),
        "no execution id was set, so none must be serialized"
    );
    assert_eq!(response["count"], 0);
}

#[test]
fn table_to_schema_declares_table_contract() {
    let schema = TableArtifact::new(vec![]).to_schema();
    assert_eq!(schema["x-artifact-type"], "table");
    assert_eq!(schema["required"][0], "columns");
    assert!(schema.get("x-table-hints").is_some());
}

#[test]
fn text_artifact_builders_and_schema() {
    let text = TextArtifact::new("body")
        .with_title("Title")
        .with_execution_id("e-1")
        .with_skill(SkillId::new("s"), "S")
        .with_request(&ctx());

    let json = serde_json::to_value(&text).unwrap();
    assert_eq!(json["x-artifact-type"], "text");
    assert_eq!(json["content"], "body");
    assert_eq!(json["title"], "Title");

    let schema = text.to_schema();
    assert_eq!(schema["x-artifact-type"], "text");
    assert_eq!(schema["required"][0], "content");
}

#[test]
fn copy_paste_text_builders_and_schema() {
    let artifact = CopyPasteTextArtifact::new("SELECT 1")
        .with_title("Query")
        .with_execution_id("e-2")
        .with_skill(SkillId::new("sql"), "SQL")
        .with_request(&ctx());

    let json = serde_json::to_value(&artifact).unwrap();
    assert_eq!(json["x-artifact-type"], "copy_paste_text");
    assert_eq!(json["content"], "SELECT 1");
    assert_eq!(json["title"], "Query");
    assert!(json.get("language").is_none());

    let schema = artifact.to_schema();
    assert_eq!(schema["x-artifact-type"], "copy_paste_text");

    let envelope = CliArtifact::copy_paste_text(artifact);
    assert_eq!(envelope.artifact_type_str(), "copy_paste_text");
    assert_eq!(envelope.title().as_deref(), Some("Query"));
}

#[test]
fn execution_metadata_builder_copies_request_identity() {
    let meta = ExecutionMetadata::builder(&ctx())
        .with_tool("my_tool")
        .with_skill(SkillId::new("skill-1"), "Skill One")
        .with_execution("exec-1")
        .build();

    assert_eq!(meta.context_id.as_str(), CTX);
    assert_eq!(meta.trace_id.as_str(), "trace-1");
    assert_eq!(meta.session_id.as_str(), "sess-1");
    assert_eq!(meta.agent_name.as_str(), "agent_one");
    assert_eq!(meta.tool_name.as_deref(), Some("my_tool"));
    assert_eq!(meta.skill_name.as_deref(), Some("Skill One"));
    assert_eq!(meta.execution_id.as_deref(), Some("exec-1"));
    assert!(meta.task_id.is_none());
}

#[test]
fn execution_metadata_chained_setters_match_builder() {
    let meta = ExecutionMetadata::with_request(&ctx())
        .with_tool("t")
        .with_skill(SkillId::new("s"), "S")
        .with_execution("e");

    assert_eq!(meta.tool_name.as_deref(), Some("t"));
    assert_eq!(meta.skill_id.as_ref().map(|s| s.as_str()), Some("s"));
    assert_eq!(meta.execution_id.as_deref(), Some("e"));
}

#[test]
fn execution_metadata_to_meta_and_schema_are_objects() {
    let meta = ExecutionMetadata::with_request(&ctx());

    let rmcp_meta = meta.to_meta().expect("serializes to an object");
    assert_eq!(
        rmcp_meta.0.get("context_id").and_then(|v| v.as_str()),
        Some(CTX)
    );

    let schema = ExecutionMetadata::schema();
    assert!(schema["properties"].get("context_id").is_some());
}

#[test]
fn research_artifact_counts_sources_and_query_override() {
    let card: systemprompt_models::artifacts::PresentationCardResponse =
        serde_json::from_value(serde_json::json!({
            "x-artifact-type": "presentation_card",
            "title": "T",
            "sections": [],
            "theme": "default"
        }))
        .expect("card deserializes");

    let artifact = ResearchArtifact::new(
        "topic",
        card,
        vec![
            SourceCitation::new("A", "https://a", 0.9),
            SourceCitation::new("B", "https://b", 0.5),
        ],
    )
    .with_query_count(4);

    assert_eq!(artifact.source_count, 2);
    assert_eq!(artifact.query_count, 4);
    assert_eq!(artifact.topic, "topic");
    assert_eq!(artifact.sources[1].uri, "https://b");
}
