// Conformance gate for the wire shapes McpResponseBuilder::build emits per
// negotiated ClientProfile, validated against the vendored official MCP
// CallToolResult JSON schemas. This is the regression gate the Cowork
// artifact incident lacked: a strict host rejects any result that fails
// its schema, so every profile's output must validate.

use rmcp::model::{CallToolResult, ProtocolVersion};
use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId};
use systemprompt_mcp::repository::McpArtifactRepository;
use systemprompt_mcp::{
    ClientProfile, McpOutputSchema, McpResponseBuilder, McpToolHandler, ToolIdentity,
};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{
    CliArtifact, Column, ColumnType, TableArtifact, TextArtifact,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

const SCHEMA_2025_06_18: &str = include_str!("../schemas/2025-06-18.schema.json");
const SCHEMA_2025_03_26: &str = include_str!("../schemas/2025-03-26.schema.json");
const SCHEMA_2025_11_25: &str = include_str!("../schemas/2025-11-25.schema.json");
const SCHEMA_2026_07_28: &str = include_str!("../schemas/2026-07-28.schema.json");

fn result_validator(schema_doc: &str, definition: &str) -> jsonschema::Validator {
    let mut doc: serde_json::Value =
        serde_json::from_str(schema_doc).expect("vendored schema parses");
    let obj = doc.as_object_mut().expect("schema document is an object");
    let pointer = if obj.contains_key("definitions") {
        format!("#/definitions/{definition}")
    } else {
        format!("#/$defs/{definition}")
    };
    obj.insert("$ref".to_owned(), serde_json::json!(pointer));
    jsonschema::validator_for(&doc).expect("vendored schema compiles")
}

fn call_tool_result_validator(schema_doc: &str) -> jsonschema::Validator {
    result_validator(schema_doc, "CallToolResult")
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

async fn build_or_skip(client: &ClientProfile, artifact: CliArtifact) -> Option<CallToolResult> {
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
    let Some(result) = build_or_skip(&unknown_client(), table()).await else {
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
    let Some(result) = build_or_skip(&old_client(), text()).await else {
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
    let Some(result) = build_or_skip(&structured_client(), table()).await else {
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
    let Some(result) = build_or_skip(&ui_client(), table()).await else {
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

fn modern_client() -> ClientProfile {
    ClientProfile {
        protocol_version: Some(ProtocolVersion::V_2025_11_25),
        client_name: Some("modern-host".to_owned()),
        extensions: std::collections::BTreeSet::new(),
    }
}

fn stateless_ui_client() -> ClientProfile {
    ClientProfile {
        protocol_version: Some(ProtocolVersion::V_2026_07_28),
        client_name: Some("stateless-apps-host".to_owned()),
        extensions: [systemprompt_models::mcp::EXTENSION_ID.to_owned()].into(),
    }
}

#[tokio::test]
async fn v2025_11_25_client_result_validates_against_official_schema() {
    let Some(result) = build_or_skip(&modern_client(), table()).await else {
        return;
    };

    assert!(result.structured_content.is_some());
    assert_meta_keys_are_prefixed(&result);

    let json = serde_json::to_value(&result).expect("serializes");
    let validator = call_tool_result_validator(SCHEMA_2025_11_25);
    assert!(
        validator.validate(&json).is_ok(),
        "2025-11-25 schema rejects: {json}"
    );
}

#[tokio::test]
async fn v2026_07_28_stateless_client_result_validates_against_official_schema() {
    let Some(result) = build_or_skip(&stateless_ui_client(), table()).await else {
        return;
    };

    assert!(has_embedded_resource(&result));
    assert!(result.structured_content.is_some());
    assert_meta_keys_are_prefixed(&result);

    let json = serde_json::to_value(&result).expect("serializes");
    let validator = call_tool_result_validator(SCHEMA_2026_07_28);
    assert!(
        validator.validate(&json).is_ok(),
        "2026-07-28 schema rejects: {json}"
    );
}

#[test]
fn advertised_protocol_version_is_pinned_not_sdk_latest() {
    assert_eq!(systemprompt_mcp::mcp_protocol_version(), "2026-07-28");
    let supported = systemprompt_mcp::mcp_supported_protocol_versions();
    assert!(supported.contains(&ProtocolVersion::V_2024_11_05));
    assert!(supported.contains(&ProtocolVersion::V_2026_07_28));
}

#[tokio::test]
async fn text_artifact_plain_result_carries_body_without_json_dump() {
    let Some(result) = build_or_skip(&unknown_client(), text()).await else {
        return;
    };

    let json = serde_json::to_value(&result).expect("serializes");
    let text = json["content"][0]["text"].as_str().expect("text block");
    assert_eq!(text, "summary line\n\nbody of the report");
}

// SEP-2549 gate: protocol 2026-07-28 requires ttlMs/cacheScope/resultType on
// every cacheable result (tools/list, resources/list, resources/templates/list,
// resources/read). The core builders must stamp them; a hand-rolled
// with_all_items result must fail the schema — that omission is exactly what
// made Cowork park every gateway connector.

struct ConformanceTool;

impl McpToolHandler for ConformanceTool {
    type Input = ConformanceInput;
    type Output = TextArtifact;

    fn tool_name(&self) -> &'static str {
        "conformance_tool"
    }

    fn description(&self) -> &'static str {
        "Schema-conformance fixture tool"
    }

    async fn handle(
        &self,
        _input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &systemprompt_identifiers::McpExecutionId,
    ) -> Result<(Self::Output, String), systemprompt_mcp::McpError> {
        Ok((TextArtifact::new("ok"), "ok".to_owned()))
    }
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
struct ConformanceInput {
    #[expect(dead_code, reason = "schema-only fixture; never deserialized")]
    message: String,
}

fn assert_validates(json: &serde_json::Value, definition: &str) {
    for (label, schema_doc) in [
        ("2025-06-18", SCHEMA_2025_06_18),
        ("2025-11-25", SCHEMA_2025_11_25),
        ("2026-07-28", SCHEMA_2026_07_28),
    ] {
        let validator = result_validator(schema_doc, definition);
        assert!(
            validator.validate(json).is_ok(),
            "{label} {definition} schema rejects: {json}"
        );
    }
}

#[test]
fn stamped_tool_list_result_validates_against_all_schemas() {
    let tool = ConformanceTool.tool_definition("systemprompt");
    let result = systemprompt_mcp::build_tool_list_result(vec![tool]);
    let json = serde_json::to_value(&result).expect("serializes");

    assert_eq!(json["resultType"], "complete");
    assert!(json["ttlMs"].is_u64());
    assert!(json["cacheScope"].is_string());
    assert_validates(&json, "ListToolsResult");
}

#[test]
fn unstamped_tool_list_result_is_rejected_by_2026_07_28_schema() {
    let tool = ConformanceTool.tool_definition("systemprompt");
    let bare = rmcp::model::ListToolsResult::with_all_items(vec![tool]);
    let json = serde_json::to_value(&bare).expect("serializes");

    let validator = result_validator(SCHEMA_2026_07_28, "ListToolsResult");
    assert!(
        validator.validate(&json).is_err(),
        "2026-07-28 schema must require ttlMs/cacheScope on tools/list: {json}"
    );
}

#[test]
fn artifact_viewer_resource_list_validates_against_all_schemas() {
    let result =
        systemprompt_mcp::build_artifact_viewer_resource(&systemprompt_mcp::ArtifactViewerConfig {
            server_name: "systemprompt",
            title: "Viewer",
            description: "Conformance fixture viewer",
            template: "<html></html>",
            icons: None,
        });
    let json = serde_json::to_value(&result).expect("serializes");

    assert_eq!(json["resultType"], "complete");
    assert!(json["ttlMs"].is_u64());
    assert_validates(&json, "ListResourcesResult");
}

#[test]
fn resource_template_list_result_validates_against_all_schemas() {
    let result = systemprompt_mcp::build_resource_template_list_result();
    let json = serde_json::to_value(&result).expect("serializes");

    assert_eq!(json["resultType"], "complete");
    assert!(json["ttlMs"].is_u64());
    assert!(json["cacheScope"].is_string());
    assert_validates(&json, "ListResourceTemplatesResult");
}

#[test]
fn read_viewer_resource_result_validates_against_all_schemas() {
    let request = rmcp::model::ReadResourceRequestParams::new("ui://systemprompt/artifact-viewer");
    let result =
        systemprompt_mcp::read_artifact_viewer_resource(&request, "systemprompt", "<html></html>")
            .expect("viewer resource reads");
    let json = serde_json::to_value(&result).expect("serializes");

    assert_eq!(json["resultType"], "complete");
    assert!(json["ttlMs"].is_u64());
    assert_validates(&json, "ReadResourceResult");
}
