// `build_metadata` across every artifact type it derives rendering hints for,
// plus the explicit `x-*-hints` overrides and the validation failure. The
// transformer suite only ever reaches it through a full tool-result transform,
// which pins one artifact type per call.

use serde_json::json;
use systemprompt_agent::services::mcp::artifact_transformer::{
    BuildMetadataParams, build_metadata,
};
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::artifacts::types::ArtifactType;

fn params<'a>(
    artifact_type: &'a ArtifactType,
    schema: Option<&'a serde_json::Value>,
    context_id: &'a str,
    task_id: &'a str,
) -> BuildMetadataParams<'a> {
    BuildMetadataParams {
        artifact_type,
        schema,
        mcp_execution_id: None,
        context_id,
        task_id,
        tool_name: "list_users",
    }
}

fn ids() -> (String, String) {
    (
        ContextId::generate().to_string(),
        TaskId::generate().to_string(),
    )
}

#[test]
fn a_table_schema_yields_column_hints_derived_from_its_item_properties() {
    let (ctx, task) = ids();
    let schema = json!({
        "items": {"properties": {"name": {"type": "string"}, "age": {"type": "integer"}}}
    });

    let metadata = build_metadata(params(&ArtifactType::Table, Some(&schema), &ctx, &task))
        .expect("metadata builds");

    let hints = metadata.rendering_hints.expect("table hints derived");
    let columns = hints["columns"].as_array().expect("columns");
    assert_eq!(columns.len(), 2);
    assert!(columns.iter().any(|c| c == "name"));
    assert_eq!(hints["filterable"], json!(true));
    assert_eq!(hints["page_size"], json!(25));
    assert_eq!(
        hints["sortable_columns"], hints["columns"],
        "every derived column is sortable by default"
    );
}

#[test]
fn an_explicit_table_hint_overrides_the_derived_one() {
    let (ctx, task) = ids();
    let schema = json!({
        "x-table-hints": {"columns": ["only_this"], "page_size": 5},
        "items": {"properties": {"ignored": {"type": "string"}}}
    });

    let metadata = build_metadata(params(&ArtifactType::Table, Some(&schema), &ctx, &task))
        .expect("metadata builds");

    let hints = metadata.rendering_hints.expect("hints present");
    assert_eq!(hints["columns"], json!(["only_this"]));
    assert_eq!(
        hints["page_size"],
        json!(5),
        "the author's hint wins over the derived default"
    );
}

#[test]
fn a_form_schema_yields_field_hints_and_a_default_layout() {
    let (ctx, task) = ids();
    let schema = json!({
        "properties": {"email": {"type": "string"}, "age": {"type": "integer"}}
    });

    let metadata = build_metadata(params(&ArtifactType::Form, Some(&schema), &ctx, &task))
        .expect("metadata builds");

    let hints = metadata.rendering_hints.expect("form hints derived");
    assert_eq!(hints["layout"], json!("vertical"));
    assert_eq!(
        hints["fields"].as_array().map(Vec::len),
        Some(2),
        "one field per schema property: {hints}"
    );
}

#[test]
fn an_explicit_form_hint_overrides_the_derived_one() {
    let (ctx, task) = ids();
    let schema = json!({
        "x-form-hints": {"layout": "grid"},
        "properties": {"ignored": {"type": "string"}}
    });

    let metadata = build_metadata(params(&ArtifactType::Form, Some(&schema), &ctx, &task))
        .expect("metadata builds");

    assert_eq!(
        metadata.rendering_hints.expect("hints")["layout"],
        json!("grid")
    );
}

#[test]
fn a_chart_carries_no_rendering_hints_because_presentation_rides_in_the_payload() {
    let (ctx, task) = ids();
    let schema = json!({"properties": {"x": {"type": "number"}}});

    let metadata = build_metadata(params(&ArtifactType::Chart, Some(&schema), &ctx, &task))
        .expect("metadata builds");

    assert!(
        metadata.rendering_hints.is_none(),
        "chart presentation travels in the artifact, not the metadata"
    );
}

#[test]
fn the_tool_name_and_schema_are_recorded_on_the_metadata() {
    let (ctx, task) = ids();
    let schema = json!({"type": "object"});

    let metadata = build_metadata(params(&ArtifactType::Text, Some(&schema), &ctx, &task))
        .expect("metadata builds");

    assert_eq!(metadata.tool_name.as_deref(), Some("list_users"));
    assert_eq!(
        metadata.artifact_type, "text",
        "the artifact type is recorded as its wire name"
    );
}

#[test]
fn an_execution_id_is_carried_through_when_supplied() {
    let (ctx, task) = ids();
    let artifact_type = ArtifactType::Text;
    let metadata = build_metadata(BuildMetadataParams {
        artifact_type: &artifact_type,
        schema: None,
        mcp_execution_id: Some("exec-1234".to_owned()),
        context_id: &ctx,
        task_id: &task,
        tool_name: "list_users",
    })
    .expect("metadata builds");

    let rendered = serde_json::to_string(&metadata).expect("serialize");
    assert!(
        rendered.contains("exec-1234"),
        "the originating execution is traceable from the artifact: {rendered}"
    );
}

#[test]
fn a_blank_context_id_is_rejected_rather_than_producing_untraceable_metadata() {
    let artifact_type = ArtifactType::Text;
    let err = build_metadata(BuildMetadataParams {
        artifact_type: &artifact_type,
        schema: None,
        mcp_execution_id: None,
        context_id: "",
        task_id: TaskId::generate().as_ref(),
        tool_name: "list_users",
    })
    .expect_err("an empty context id cannot identify a conversation");

    assert!(
        !err.to_string().is_empty(),
        "the validation failure carries a reason"
    );
}

#[test]
fn a_table_schema_without_item_properties_yields_empty_hints() {
    let (ctx, task) = ids();
    let schema = json!({"type": "array"});

    let metadata = build_metadata(params(&ArtifactType::Table, Some(&schema), &ctx, &task))
        .expect("metadata builds");

    assert!(
        metadata
            .rendering_hints
            .as_ref()
            .and_then(|h| h.as_object())
            .is_some_and(serde_json::Map::is_empty),
        "nothing to derive columns from means no column hints"
    );
}
