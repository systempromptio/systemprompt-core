// Conformance gate for the wire shapes McpResponseBuilder::build emits per
// negotiated ClientProfile, validated against the vendored official MCP
// CallToolResult JSON schemas. This is the regression gate the Cowork
// artifact incident lacked: a strict host rejects any result that fails
// its schema, so every profile's output must validate.

use rmcp::model::{CallToolResult, ProtocolVersion};
use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId};
use systemprompt_mcp::repository::McpArtifactRepository;
use systemprompt_mcp::{ClientProfile, McpOutputSchema, McpResponseBuilder, ToolIdentity};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{
    CliArtifact, Column, ColumnType, TableArtifact, TextArtifact,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

const SCHEMA_2025_06_18: &str = include_str!("../schemas/2025-06-18.schema.json");
const SCHEMA_2025_03_26: &str = include_str!("../schemas/2025-03-26.schema.json");

fn call_tool_result_validator(schema_doc: &str) -> jsonschema::Validator {
    let mut doc: serde_json::Value =
        serde_json::from_str(schema_doc).expect("vendored schema parses");
    doc.as_object_mut()
        .expect("schema document is an object")
        .insert(
            "$ref".to_owned(),
            serde_json::json!("#/definitions/CallToolResult"),
        );
    jsonschema::validator_for(&doc).expect("vendored schema compiles")
}

fn unknown_client() -> ClientProfile {
    ClientProfile::unknown()
}

fn old_client() -> ClientProfile {
    ClientProfile {
        protocol_version: Some(ProtocolVersion::V_2024_11_05),
        client_name: Some("legacy-host".to_owned()),
        extensions: std::collections::BTreeSet::new(),
    }
}

fn structured_client() -> ClientProfile {
    ClientProfile {
        protocol_version: Some(ProtocolVersion::V_2025_06_18),
        client_name: Some("plain-host".to_owned()),
        extensions: std::collections::BTreeSet::new(),
    }
}

fn ui_client() -> ClientProfile {
    ClientProfile {
        protocol_version: Some(ProtocolVersion::V_2025_06_18),
        client_name: Some("apps-host".to_owned()),
        extensions: [systemprompt_models::mcp::EXTENSION_ID.to_owned()].into(),
    }
}

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new(format!("s-{}", uuid::Uuid::new_v4().simple())),
        TraceId::new("t"),
        ContextId::generate(),
        AgentName::new("a"),
    )
}

async fn build(client: &ClientProfile, artifact: CliArtifact) -> Option<CallToolResult> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let repo = McpArtifactRepository::new(&db).expect("repo");
    let context = ctx();
    let exec_id = McpExecutionId::new(format!("exec-{}", uuid::Uuid::new_v4().simple()));
    let artifact_type = artifact.artifact_type_name();
    let title = artifact.artifact_title();

    Some(
        McpResponseBuilder::new(
            artifact,
            ToolIdentity::new("systemprompt", "conformance_tool"),
            &context,
            &exec_id,
            client,
        )
        .build("summary line", &repo, &artifact_type, title)
        .await
        .expect("response builds"),
    )
}

fn table() -> CliArtifact {
    CliArtifact::table(
        TableArtifact::new(vec![Column::new("email", ColumnType::String)])
            .with_rows(vec![serde_json::json!({"email": "ed@example.com"})]),
    )
}

fn text() -> CliArtifact {
    CliArtifact::text(TextArtifact::new("body of the report").with_title("Report"))
}

fn has_embedded_resource(result: &CallToolResult) -> bool {
    result.content.iter().any(|b| b.as_resource().is_some())
}

fn assert_meta_keys_are_prefixed(result: &CallToolResult) {
    if let Some(meta) = &result.meta {
        for key in meta.0.keys() {
            assert!(
                key.contains('/'),
                "custom _meta key `{key}` must be reverse-DNS prefixed"
            );
        }
    }
}

#[tokio::test]
async fn unknown_client_gets_text_only_result() {
    let Some(result) = build(&unknown_client(), table()).await else {
        return;
    };

    assert!(result.structured_content.is_none());
    assert!(result.meta.is_none());
    assert!(!has_embedded_resource(&result));

    let json = serde_json::to_value(&result).expect("serializes");
    let text = json["content"][0]["text"].as_str().expect("text block");
    assert!(text.contains("summary line"));
    assert!(
        text.contains("ed@example.com"),
        "data must reach text-only clients: {text}"
    );

    let validator = call_tool_result_validator(SCHEMA_2025_03_26);
    assert!(
        validator.validate(&json).is_ok(),
        "2025-03-26 schema rejects: {json}"
    );
}

#[tokio::test]
async fn old_protocol_client_gets_no_structured_content() {
    let Some(result) = build(&old_client(), text()).await else {
        return;
    };

    assert!(result.structured_content.is_none());
    assert!(!has_embedded_resource(&result));

    let json = serde_json::to_value(&result).expect("serializes");
    let text = json["content"][0]["text"].as_str().expect("text block");
    assert!(
        text.contains("body of the report"),
        "text artifacts fold their body into the text block: {text}"
    );

    let validator = call_tool_result_validator(SCHEMA_2025_03_26);
    assert!(
        validator.validate(&json).is_ok(),
        "2025-03-26 schema rejects: {json}"
    );
}

#[tokio::test]
async fn structured_client_gets_typed_output_matching_the_advertised_schema() {
    let Some(result) = build(&structured_client(), table()).await else {
        return;
    };

    assert!(!has_embedded_resource(&result));
    assert_meta_keys_are_prefixed(&result);

    let structured = result
        .structured_content
        .as_ref()
        .expect("structuredContent");
    assert_eq!(
        structured.get("artifact_type").and_then(|v| v.as_str()),
        Some("table")
    );

    let output_schema = <CliArtifact as McpOutputSchema>::validated_schema();
    let output_validator =
        jsonschema::validator_for(&output_schema).expect("output schema compiles");
    assert!(
        output_validator.validate(structured).is_ok(),
        "structuredContent must match the advertised outputSchema"
    );

    let json = serde_json::to_value(&result).expect("serializes");
    let validator = call_tool_result_validator(SCHEMA_2025_06_18);
    assert!(
        validator.validate(&json).is_ok(),
        "2025-06-18 schema rejects: {json}"
    );
}

#[tokio::test]
async fn ui_client_gets_embedded_resource_and_prefixed_meta() {
    let Some(result) = build(&ui_client(), table()).await else {
        return;
    };

    assert!(has_embedded_resource(&result));
    assert!(result.structured_content.is_some());
    assert_meta_keys_are_prefixed(&result);

    let meta = result.meta.as_ref().expect("_meta present");
    let exec = meta
        .0
        .get(systemprompt_models::artifacts::EXECUTION_META_KEY)
        .and_then(|v| v.as_object())
        .expect("execution provenance nested under one prefixed key");
    assert!(exec.contains_key("artifact_id"));
    assert!(exec.contains_key("mcp_execution_id"));
    assert!(exec.contains_key("context_id"));
    assert!(
        meta.0
            .get(systemprompt_mcp::UI_RESOURCE_URI_META_KEY)
            .and_then(|v| v.as_str())
            .is_some_and(|uri| uri.starts_with("ui://"))
    );

    let json = serde_json::to_value(&result).expect("serializes");
    let validator = call_tool_result_validator(SCHEMA_2025_06_18);
    assert!(
        validator.validate(&json).is_ok(),
        "2025-06-18 schema rejects: {json}"
    );
}

#[tokio::test]
async fn text_artifact_plain_result_carries_body_without_json_dump() {
    let Some(result) = build(&unknown_client(), text()).await else {
        return;
    };

    let json = serde_json::to_value(&result).expect("serializes");
    let text = json["content"][0]["text"].as_str().expect("text block");
    assert_eq!(text, "summary line\n\nbody of the report");
}
