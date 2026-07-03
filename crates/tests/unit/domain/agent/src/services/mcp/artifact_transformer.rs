//! Unit tests for MCP artifact transformer
//!
//! Tests cover:
//! - parse_tool_response — valid JSON, null input, empty object, missing fields
//! - calculate_fingerprint — deterministic hashing, distinct inputs produce
//!   distinct hashes
//! - infer_type — schema x-artifact-type, tabular/form/chart schema, data-level
//!   type, fall-through past the "cli" envelope tag, fallback error
//! - build_metadata — all artifact types with rendering hints, optional fields
//! - build_parts — JSON object input, content array with text/image/resource,
//!   error on invalid

use serde_json::json;
use systemprompt_agent::services::mcp::artifact_transformer::{
    BuildMetadataParams, build_metadata, build_parts, calculate_fingerprint, infer_type,
    parse_tool_response,
};
use systemprompt_models::artifacts::types::ArtifactType;
use systemprompt_models::artifacts::{CliArtifact, TextArtifact};

#[test]
fn parse_tool_response_valid_complete() {
    let input = json!({
        "artifact_id": "art-001",
        "mcp_execution_id": "exec-001",
        "artifact": {"key": "value"},
        "_metadata": {
            "skill_id": "skill-1",
            "skill_name": "test-skill",
            "execution_id": "exec-ref"
        }
    });

    let parsed = parse_tool_response(&input).expect("should parse");
    assert_eq!(parsed.artifact_id.as_str(), "art-001");
    assert_eq!(parsed.mcp_execution_id.as_str(), "exec-001");
    assert_eq!(
        parsed.metadata.skill_id.as_ref().map(|s| s.as_str()),
        Some("skill-1")
    );
    assert_eq!(parsed.metadata.skill_name, Some("test-skill".to_string()));
    assert_eq!(parsed.metadata.execution_id, Some("exec-ref".to_string()));
}

#[test]
fn parse_tool_response_minimal_metadata() {
    let input = json!({
        "artifact_id": "art-002",
        "mcp_execution_id": "exec-002",
        "artifact": {"data": 42},
        "_metadata": {}
    });

    let parsed = parse_tool_response(&input).expect("should parse");
    assert_eq!(parsed.artifact_id.as_str(), "art-002");
    assert!(parsed.metadata.skill_id.is_none());
    assert!(parsed.metadata.skill_name.is_none());
    assert!(parsed.metadata.execution_id.is_none());
}

#[test]
fn parse_tool_response_null_input_returns_error() {
    let result = parse_tool_response(&json!(null));
    assert!(result.is_err());
}

#[test]
fn parse_tool_response_empty_object_returns_error() {
    let result = parse_tool_response(&json!({}));
    assert!(result.is_err());
}

#[test]
fn parse_tool_response_missing_artifact_id_returns_error() {
    let input = json!({
        "mcp_execution_id": "exec-003",
        "artifact": {},
        "_metadata": {}
    });

    let result = parse_tool_response(&input);
    assert!(result.is_err());
}

#[test]
fn parse_tool_response_missing_mcp_execution_id_returns_error() {
    let input = json!({
        "artifact_id": "art-003",
        "artifact": {},
        "_metadata": {}
    });

    let result = parse_tool_response(&input);
    assert!(result.is_err());
}

#[test]
fn parse_tool_response_missing_metadata_returns_error() {
    let input = json!({
        "artifact_id": "art-004",
        "mcp_execution_id": "exec-004",
        "artifact": {}
    });

    let result = parse_tool_response(&input);
    assert!(result.is_err());
}

#[test]
fn parse_tool_response_string_input_returns_error() {
    let result = parse_tool_response(&json!("just a string"));
    assert!(result.is_err());
}

#[test]
fn parse_tool_response_array_input_returns_error() {
    let result = parse_tool_response(&json!([1, 2, 3]));
    assert!(result.is_err());
}

#[test]
fn parse_tool_response_artifact_can_be_nested_object() {
    let input = json!({
        "artifact_id": "art-005",
        "mcp_execution_id": "exec-005",
        "artifact": {
            "rows": [{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}],
            "total": 2
        },
        "_metadata": {}
    });

    let parsed = parse_tool_response(&input).expect("should parse");
    assert!(parsed.artifact.is_object());
    assert_eq!(parsed.artifact["total"], 2);
}

#[test]
fn calculate_fingerprint_deterministic_same_inputs() {
    let fp1 = calculate_fingerprint("my-tool", Some(&json!({"a": 1})));
    let fp2 = calculate_fingerprint("my-tool", Some(&json!({"a": 1})));
    assert_eq!(fp1, fp2);
}

#[test]
fn calculate_fingerprint_different_tool_names_same_args() {
    let fp1 = calculate_fingerprint("tool-a", Some(&json!({"x": 1})));
    let fp2 = calculate_fingerprint("tool-b", Some(&json!({"x": 1})));
    assert_ne!(fp1, fp2);
}

#[test]
fn calculate_fingerprint_same_tool_different_args() {
    let fp1 = calculate_fingerprint("tool", Some(&json!({"x": 1})));
    let fp2 = calculate_fingerprint("tool", Some(&json!({"x": 2})));
    assert_ne!(fp1, fp2);
}

#[test]
fn calculate_fingerprint_none_arguments() {
    let fp = calculate_fingerprint("tool", None);
    assert!(fp.starts_with("tool-"));
    assert!(fp.len() > 5);
}

#[test]
fn calculate_fingerprint_empty_object_arguments() {
    let fp = calculate_fingerprint("tool", Some(&json!({})));
    assert!(fp.starts_with("tool-"));
}

#[test]
fn calculate_fingerprint_none_vs_empty_object_differ() {
    let fp_none = calculate_fingerprint("tool", None);
    let fp_empty = calculate_fingerprint("tool", Some(&json!({})));
    assert_ne!(fp_none, fp_empty);
}

#[test]
fn calculate_fingerprint_contains_tool_name_prefix() {
    let fp = calculate_fingerprint("lookup-user", Some(&json!({"id": 42})));
    assert!(fp.starts_with("lookup-user-"));
}

#[test]
fn calculate_fingerprint_complex_arguments() {
    let args = json!({
        "query": "SELECT * FROM users",
        "params": [1, 2, 3],
        "nested": {"deep": true}
    });
    let fp = calculate_fingerprint("db-query", Some(&args));
    assert!(fp.starts_with("db-query-"));
    assert!(fp.len() > "db-query-".len());
}

#[test]
fn infer_type_from_schema_x_artifact_type() {
    let schema = json!({"x-artifact-type": "chart"});
    let artifact = json!({"data": []});
    let result = infer_type(&artifact, Some(&schema), "my-tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Chart));
}

#[test]
fn infer_type_from_schema_nested_artifact_property() {
    let schema = json!({
        "properties": {
            "artifact": {
                "x-artifact-type": "form"
            }
        }
    });
    let artifact = json!({"some": "data"});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Form));
}

#[test]
fn infer_type_tabular_schema() {
    let schema = json!({
        "type": "array",
        "items": {"type": "object", "properties": {"id": {"type": "integer"}}}
    });
    let artifact = json!({"rows": []});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Table));
}

#[test]
fn infer_type_form_schema() {
    let schema = json!({
        "properties": {
            "fields": {"type": "array"}
        }
    });
    let artifact = json!({"fields": []});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Form));
}

#[test]
fn infer_type_chart_schema() {
    let schema = json!({
        "properties": {
            "labels": {"type": "array"},
            "datasets": {"type": "array"}
        }
    });
    let artifact = json!({"labels": [], "datasets": []});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Chart));
}

#[test]
fn infer_type_from_data_x_artifact_type() {
    let artifact = json!({"x-artifact-type": "dashboard", "panels": []});
    let result = infer_type(&artifact, None, "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Dashboard));
}

#[test]
fn infer_type_from_nested_artifact_data() {
    let artifact = json!({
        "artifact": {
            "x-artifact-type": "list"
        }
    });
    let result = infer_type(&artifact, None, "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::List));
}

#[test]
fn infer_type_from_nested_card_data() {
    let artifact = json!({
        "artifact": {
            "card": {
                "x-artifact-type": "presentation_card"
            }
        }
    });
    let result = infer_type(&artifact, None, "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::PresentationCard));
}

#[test]
fn infer_type_tabular_data_array_of_objects() {
    let artifact = json!([
        {"id": 1, "name": "Alice"},
        {"id": 2, "name": "Bob"}
    ]);
    let result = infer_type(&artifact, None, "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Table));
}

#[test]
fn infer_type_returns_error_when_no_type_found() {
    let artifact = json!({"unknown": "structure"});
    let result = infer_type(&artifact, None, "mystery-tool");
    assert!(result.is_err());
}

#[test]
fn infer_type_custom_type_from_schema() {
    let schema = json!({"x-artifact-type": "sparkline"});
    let artifact = json!({"points": [1, 2, 3]});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Custom(ref s) if s == "sparkline"));
}

#[test]
fn infer_type_schema_takes_priority_over_data() {
    let schema = json!({"x-artifact-type": "text"});
    let artifact = json!({"x-artifact-type": "chart"});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Text));
}

#[test]
fn infer_type_empty_array_not_tabular() {
    let artifact = json!([]);
    let result = infer_type(&artifact, None, "tool");
    assert!(result.is_err());
}

#[test]
fn infer_type_array_of_primitives_not_tabular() {
    let artifact = json!([1, 2, 3]);
    let result = infer_type(&artifact, None, "tool");
    assert!(result.is_err());
}

#[test]
fn infer_type_envelope_schema_tag_falls_through_to_data_x_artifact_type() {
    let schema = json!({"x-artifact-type": "cli"});
    let artifact = json!({"artifact_type": "table", "x-artifact-type": "table", "data": []});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Table));
}

#[test]
fn infer_type_envelope_schema_tag_falls_through_to_embedded_variant_tag() {
    let schema = json!({"x-artifact-type": "cli"});
    let artifact = json!({"artifact_type": "list", "items": []});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::List));
}

#[test]
fn infer_type_envelope_schema_tag_resolves_message_variant() {
    let schema = json!({"x-artifact-type": "cli"});
    let artifact =
        json!({"artifact_type": "message", "x-artifact-type": "message", "messages": []});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Message));
}

#[test]
fn infer_type_serialized_cli_envelope_resolves_variant() {
    let cli = CliArtifact::text(TextArtifact::new("hi"));
    let artifact = serde_json::to_value(&cli).expect("should serialize");
    let schema = json!({"x-artifact-type": "cli"});
    let result = infer_type(&artifact, Some(&schema), "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Text));
}

#[test]
fn infer_type_envelope_schema_without_data_tag_errors() {
    let schema = json!({"x-artifact-type": "cli"});
    let artifact = json!({"unknown": "structure"});
    let result = infer_type(&artifact, Some(&schema), "tool");
    assert!(result.is_err());
}

#[test]
fn infer_type_envelope_data_tag_is_never_final() {
    let artifact = json!({"x-artifact-type": "cli"});
    let result = infer_type(&artifact, None, "tool");
    assert!(result.is_err());
}

#[test]
fn infer_type_message_from_data() {
    let artifact = json!({"x-artifact-type": "message", "messages": []});
    let result = infer_type(&artifact, None, "tool").expect("should infer");
    assert!(matches!(result, ArtifactType::Message));
}

#[test]
fn build_metadata_text_type_no_hints() {
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Text,
        schema: None,
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-1",
        tool_name: "summarize",
    });
    let meta = result.expect("text metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("summarize"));
    assert_eq!(meta.rendering_hints, None);
    assert_eq!(meta.mcp_schema, None);
    assert_eq!(meta.mcp_execution_id, None);
}

#[test]
fn build_metadata_table_with_schema_hints() {
    let schema = json!({
        "x-table-hints": {"columns": ["id", "name"], "sortable_columns": ["id"]}
    });
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Table,
        schema: Some(&schema),
        mcp_execution_id: Some("exec-1".to_string()),
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-2",
        tool_name: "list-users",
    });
    let meta = result.expect("table metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("list-users"));
    assert_eq!(meta.mcp_execution_id.as_deref(), Some("exec-1"));
    assert_eq!(
        meta.rendering_hints,
        Some(json!({"columns": ["id", "name"], "sortable_columns": ["id"]}))
    );
}

#[test]
fn build_metadata_table_infers_hints_from_items() {
    let schema = json!({
        "items": {
            "type": "object",
            "properties": {
                "id": {"type": "integer"},
                "name": {"type": "string"}
            }
        }
    });
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Table,
        schema: Some(&schema),
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-3",
        tool_name: "query-table",
    });
    let meta = result.expect("table metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("query-table"));
    assert_eq!(
        meta.rendering_hints,
        Some(json!({
            "columns": ["id", "name"],
            "sortable_columns": ["id", "name"],
            "filterable": true,
            "page_size": 25,
        }))
    );
}

#[test]
fn build_metadata_form_with_schema_hints() {
    let schema = json!({
        "x-form-hints": {"layout": "horizontal", "fields": []}
    });
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Form,
        schema: Some(&schema),
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-4",
        tool_name: "create-form",
    });
    let meta = result.expect("form metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("create-form"));
    assert_eq!(
        meta.rendering_hints,
        Some(json!({"layout": "horizontal", "fields": []}))
    );
}

#[test]
fn build_metadata_form_infers_fields_from_properties() {
    let schema = json!({
        "properties": {
            "email": {"type": "string", "format": "email"},
            "age": {"type": "integer"}
        }
    });
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Form,
        schema: Some(&schema),
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-5",
        tool_name: "edit-profile",
    });
    let meta = result.expect("form metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("edit-profile"));
    let hints = meta.rendering_hints.expect("form hints present");
    assert_eq!(hints["layout"], "vertical");
    assert_eq!(hints["fields"].as_array().map(|a| a.len()), Some(2));
}

#[test]
fn build_metadata_chart_default_hints() {
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Chart,
        schema: None,
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-6",
        tool_name: "chart-tool",
    });
    let meta = result.expect("chart metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("chart-tool"));
    assert_eq!(meta.rendering_hints, Some(json!({"chart_type": "bar"})));
}

#[test]
fn build_metadata_presentation_card_default_hints() {
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::PresentationCard,
        schema: None,
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-7",
        tool_name: "card-tool",
    });
    let meta = result.expect("presentation card metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("card-tool"));
    assert_eq!(meta.rendering_hints, Some(json!({"theme": "default"})));
}

#[test]
fn build_metadata_dashboard_default_hints() {
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Dashboard,
        schema: None,
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-8",
        tool_name: "dashboard-tool",
    });
    let meta = result.expect("dashboard metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("dashboard-tool"));
    assert_eq!(meta.rendering_hints, Some(json!({"layout": "vertical"})));
}

#[test]
fn build_metadata_with_mcp_execution_id() {
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Text,
        schema: None,
        mcp_execution_id: Some("exec-abc".to_string()),
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-9",
        tool_name: "text-tool",
    })
    .expect("should build");
    assert!(format!("{result:?}").contains("exec-abc"));
}

#[test]
fn build_metadata_with_schema_attaches_mcp_schema() {
    let schema = json!({"x-artifact-type": "text", "description": "A text output"});
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Text,
        schema: Some(&schema),
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-10",
        tool_name: "text-tool",
    });
    let meta = result.expect("text metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("text-tool"));
    assert_eq!(
        meta.mcp_schema,
        Some(json!({"x-artifact-type": "text", "description": "A text output"}))
    );
    assert_eq!(meta.rendering_hints, None);
}

#[test]
fn build_metadata_image_type_no_special_hints() {
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &ArtifactType::Image,
        schema: None,
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-11",
        tool_name: "gen-image",
    });
    let meta = result.expect("image metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("gen-image"));
    assert_eq!(meta.rendering_hints, None);
}

#[test]
fn build_metadata_custom_type() {
    let custom = ArtifactType::Custom("heatmap".to_string());
    let result = build_metadata(BuildMetadataParams {
        artifact_type: &custom,
        schema: None,
        mcp_execution_id: None,
        context_id: "00000000-0000-4000-8000-000000000001",
        task_id: "task-12",
        tool_name: "heatmap-tool",
    });
    let meta = result.expect("custom metadata builds");
    assert_eq!(meta.tool_name.as_deref(), Some("heatmap-tool"));
    assert_eq!(meta.artifact_type, "heatmap");
    assert_eq!(meta.rendering_hints, None);
}

#[test]
fn build_parts_from_json_object() {
    let artifact = json!({"id": 1, "name": "Alice"});
    let parts = build_parts(&artifact).expect("should build parts");
    assert_eq!(parts.len(), 1);
}

#[test]
fn build_parts_from_nested_object() {
    let artifact = json!({
        "user": {"id": 1},
        "meta": {"created": "2026-01-01"}
    });
    let parts = build_parts(&artifact).expect("should build parts");
    assert_eq!(parts.len(), 1);
}

#[test]
fn build_parts_from_empty_object() {
    let artifact = json!({});
    let parts = build_parts(&artifact).expect("should build parts");
    assert_eq!(parts.len(), 1);
}

#[test]
fn build_parts_object_with_content_key_returns_data_part() {
    let artifact = json!({
        "content": [
            {"type": "text", "text": "Hello, world!"}
        ]
    });
    let parts = build_parts(&artifact).expect("should build parts");
    assert_eq!(parts.len(), 1);
}

#[test]
fn build_parts_object_with_nested_arrays() {
    let artifact = json!({
        "rows": [{"id": 1}, {"id": 2}],
        "total": 2
    });
    let parts = build_parts(&artifact).expect("should build parts");
    assert_eq!(parts.len(), 1);
}

#[test]
fn build_parts_object_with_many_keys() {
    let artifact = json!({
        "a": 1, "b": 2, "c": 3, "d": 4, "e": 5
    });
    let parts = build_parts(&artifact).expect("should build parts");
    assert_eq!(parts.len(), 1);
}

#[test]
fn build_parts_string_value_returns_error() {
    let artifact = json!("just a string");
    let result = build_parts(&artifact);
    assert!(result.is_err());
}

#[test]
fn build_parts_number_value_returns_error() {
    let artifact = json!(42);
    let result = build_parts(&artifact);
    assert!(result.is_err());
}

#[test]
fn build_parts_null_value_returns_error() {
    let artifact = json!(null);
    let result = build_parts(&artifact);
    assert!(result.is_err());
}

#[test]
fn build_parts_array_value_returns_error() {
    let artifact = json!([1, 2, 3]);
    let result = build_parts(&artifact);
    assert!(result.is_err());
}

#[test]
fn build_parts_boolean_value_returns_error() {
    let artifact = json!(true);
    let result = build_parts(&artifact);
    assert!(result.is_err());
}

#[test]
fn build_parts_array_of_objects_returns_error() {
    let artifact = json!([{"id": 1}, {"id": 2}]);
    let result = build_parts(&artifact);
    assert!(result.is_err());
}

#[test]
fn build_parts_empty_array_returns_error() {
    let artifact = json!([]);
    let result = build_parts(&artifact);
    assert!(result.is_err());
}

#[test]
fn build_parts_float_value_returns_error() {
    let artifact = json!(3.14);
    let result = build_parts(&artifact);
    assert!(result.is_err());
}
