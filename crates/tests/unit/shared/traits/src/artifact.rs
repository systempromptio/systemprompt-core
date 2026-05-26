use systemprompt_traits::artifact::{ArtifactSupport, schemas};

struct StubArtifact;

impl ArtifactSupport for StubArtifact {
    fn get_output_schema_for_tool(
        &self,
        _tool_name: &str,
        _arguments: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<serde_json::Value> {
        None
    }
}

#[test]
fn validate_artifact_schema_default_impl() {
    let a = StubArtifact;
    assert!(a.validate_artifact_schema("tool", false, false));
    assert!(a.validate_artifact_schema("tool", false, true));
    assert!(a.validate_artifact_schema("tool", true, true));
    assert!(!a.validate_artifact_schema("tool", true, false));
}

#[test]
fn presentation_card_schema_includes_x_artifact_type_and_theme() {
    let s = schemas::presentation_card(Some("dark"));
    assert_eq!(s["x-artifact-type"], "presentation_card");
    assert_eq!(s["x-presentation-hints"]["theme"], "dark");
    assert!(s["properties"]["sections"]["type"] == "array");
    assert!(s["properties"]["ctas"]["type"] == "array");
}

#[test]
fn presentation_card_schema_no_theme_omits_hints() {
    let s = schemas::presentation_card(None);
    assert!(s.get("x-presentation-hints").is_none());
}

#[test]
fn table_schema_marks_columns_and_rows_required() {
    let s = schemas::table();
    assert_eq!(s["x-artifact-type"], "table");
    let required = s["required"].as_array().unwrap();
    let names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(names.contains(&"columns"));
    assert!(names.contains(&"rows"));
}

#[test]
fn chart_schema_includes_chart_type_and_data_array() {
    let s = schemas::chart("pie");
    assert_eq!(s["x-artifact-type"], "chart");
    assert_eq!(s["x-chart-type"], "pie");
    assert_eq!(s["properties"]["data"]["type"], "array");
}

#[test]
fn code_schema_with_language_sets_default() {
    let s = schemas::code(Some("rust"));
    assert_eq!(s["x-artifact-type"], "code");
    assert_eq!(s["properties"]["language"]["default"], "rust");
    let required = s["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "code"));
}

#[test]
fn code_schema_without_language_omits_default() {
    let s = schemas::code(None);
    assert!(s["properties"]["language"].get("default").is_none());
}

#[test]
fn markdown_schema_requires_content() {
    let s = schemas::markdown();
    assert_eq!(s["x-artifact-type"], "markdown");
    let required = s["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "content"));
}
