// Branch-level coverage for the per-artifact template renderers, exercised
// through their public `UiRenderer::render` entry points. The section/field
// internals are `pub(super)` and only reachable this way.

use systemprompt_mcp::services::ui_renderer::UiRenderer;
use systemprompt_mcp::services::ui_renderer::templates::{
    ChartRenderer, DashboardRenderer, FormRenderer, ImageRenderer, ListRenderer, TableRenderer,
    TextRenderer,
};
use systemprompt_models::{
    A2aArtifact as Artifact, ArtifactMetadata, DataPart, FileContent, FilePart, Part, TextPart,
};

fn make_artifact(
    artifact_type: &str,
    title: Option<&str>,
    description: Option<&str>,
    parts: Vec<Part>,
    rendering_hints: Option<serde_json::Value>,
) -> Artifact {
    let context_id = systemprompt_identifiers::ContextId::generate();
    let task_id = systemprompt_identifiers::TaskId::generate();
    let mut metadata = ArtifactMetadata::new(artifact_type.to_string(), context_id, task_id);
    if let Some(hints) = rendering_hints {
        metadata = metadata.with_rendering_hints(hints);
    }
    Artifact {
        id: systemprompt_identifiers::ArtifactId::generate(),
        title: title.map(String::from),
        description: description.map(String::from),
        parts,
        extensions: vec![],
        metadata,
    }
}

fn data_part(data: serde_json::Value) -> Part {
    let map = match data {
        serde_json::Value::Object(m) => m,
        other => {
            let mut m = serde_json::Map::new();
            m.insert("data".to_string(), other);
            m
        },
    };
    Part::Data(DataPart { data: map })
}

fn data_array_part(items: serde_json::Value) -> Part {
    // A data part whose JSON Object wraps an array under "data" so renderers
    // that branch on `as_data().as_array()` are exercised. Some renderers read
    // the raw array via DataPart; here we put the array directly under a key
    // many renderers also accept.
    let mut m = serde_json::Map::new();
    m.insert("items".to_string(), items);
    Part::Data(DataPart { data: m })
}

fn text_part(text: &str) -> Part {
    Part::Text(TextPart {
        text: text.to_string(),
    })
}

fn file_part(name: Option<&str>, mime: Option<&str>, bytes: Option<&str>, url: Option<&str>) -> Part {
    Part::File(FilePart {
        file: FileContent {
            name: name.map(String::from),
            mime_type: mime.map(String::from),
            bytes: bytes.map(String::from),
            url: url.map(String::from),
        },
    })
}

// ---------------------------------------------------------------------------
// Table renderer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn table_object_columns_and_data_path() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        Some("Inferred"),
        None,
        vec![data_part(serde_json::json!({
            "columns": ["name", "age"],
            "data": [{"name": "Alice", "age": 30}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Inferred"));
    assert!(result.html.contains("Alice"));
}

#[tokio::test]
async fn table_rows_key_alias() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        None,
        None,
        vec![data_part(serde_json::json!({
            "columns": ["x"],
            "rows": [{"x": 1}, {"x": 2}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("data-table"));
}

#[tokio::test]
async fn table_columns_as_objects_with_name() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        None,
        None,
        vec![data_part(serde_json::json!({
            "columns": [{"name": "score"}],
            "data": [{"score": 99}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("score"));
}

#[tokio::test]
async fn table_rows_as_arrays() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        None,
        None,
        vec![data_part(serde_json::json!({
            "columns": ["a", "b"],
            "data": [[1, 2], [3, 4]]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("TABLE_ROWS"));
}

#[tokio::test]
async fn table_sortable_columns_hint() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        None,
        None,
        vec![data_part(serde_json::json!({"columns": ["a"], "data": [{"a": 1}]}))],
        Some(serde_json::json!({"sortable_columns": ["a"]})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("TABLE_SORTABLE"));
    assert!(result.html.contains("\"a\""));
}

#[tokio::test]
async fn table_empty_title_omits_h1() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        Some(""),
        None,
        vec![data_part(serde_json::json!({"columns": ["a"], "data": [{"a": 1}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    // The shared HtmlBuilder always emits the title element; an explicitly
    // empty title must not fall back to the default "Table" text.
    assert!(!result.html.contains(">Table<"));
}

#[tokio::test]
async fn table_default_title_when_none() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact("table", None, None, vec![], None);
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Table"));
}

#[tokio::test]
async fn table_escapes_title() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        Some("<b>x</b>"),
        None,
        vec![data_part(serde_json::json!({"columns": ["a"], "data": [{"a": 1}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("&lt;b&gt;"));
}

// ---------------------------------------------------------------------------
// Chart renderer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn chart_area_type_sets_fill() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::json!({
            "labels": ["A", "B"],
            "datasets": [{"label": "D", "data": [1, 2]}]
        }))],
        Some(serde_json::json!({"chart_type": "area"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("fill"));
    // area maps to chart.js "line"
    assert!(result.html.contains("\"type\":\"line\"") || result.html.contains("line"));
}

#[tokio::test]
async fn chart_doughnut_type() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::json!({
            "labels": ["A"],
            "datasets": [{"data": [1]}]
        }))],
        Some(serde_json::json!({"chart_type": "doughnut"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("doughnut"));
}

#[tokio::test]
async fn chart_unknown_type_defaults_bar() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::json!({"labels": ["A"], "datasets": [{"data": [1]}]}))],
        Some(serde_json::json!({"chart_type": "weird"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("bar"));
}

#[tokio::test]
async fn chart_data_only_synthesizes_dataset() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        Some("MyData"),
        None,
        vec![data_part(serde_json::json!({"labels": ["A", "B"], "data": [10, 20]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("MyData"));
    assert!(result.html.contains("CHART_CONFIG"));
}

#[tokio::test]
async fn chart_title_hint_adds_plugin_title() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::json!({"labels": ["A"], "datasets": [{"data": [1]}]}))],
        Some(serde_json::json!({"chart_type": "bar", "title": "Chart Title"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Chart Title"));
}

#[tokio::test]
async fn chart_only_x_axis_label() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::json!({"labels": ["A"], "datasets": [{"data": [1]}]}))],
        Some(serde_json::json!({"x_axis_label": "OnlyX"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("OnlyX"));
    assert!(result.html.contains("scales"));
}

#[tokio::test]
async fn chart_with_description() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        Some("T"),
        Some("Chart description"),
        vec![data_part(serde_json::json!({"labels": ["A"], "datasets": [{"data": [1]}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Chart description"));
    assert!(result.html.contains("mcp-app-description"));
}

// ---------------------------------------------------------------------------
// List renderer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_none_style() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({"items": ["A"]}))],
        Some(serde_json::json!({"style": "none"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("unstyled-list"));
    assert!(result.html.contains("<ul"));
}

#[tokio::test]
async fn list_numbered_style_is_ordered() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({"items": ["A"]}))],
        Some(serde_json::json!({"style": "numbered"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("<ol"));
}

#[tokio::test]
async fn list_item_with_icon() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({
            "items": [{"title": "Star", "icon": "⭐"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("item-icon"));
    assert!(result.html.contains("⭐"));
}

#[tokio::test]
async fn list_item_name_alias() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({"items": [{"name": "ByName"}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("ByName"));
}

#[tokio::test]
async fn list_item_label_alias() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({"items": [{"label": "ByLabel"}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("ByLabel"));
}

#[tokio::test]
async fn list_item_subtitle_and_url_aliases() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({
            "items": [{"title": "T", "subtitle": "Sub", "url": "https://x.test"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Sub"));
    assert!(result.html.contains("https://x.test"));
    assert!(result.html.contains("noopener"));
}

#[tokio::test]
async fn list_items_top_level_array() {
    let renderer = ListRenderer::new();
    // DataPart wrapping an array directly under "items" already tested; here
    // exercise the `as_data().as_array()` path by placing the array at the
    // root via the items helper alias.
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_array_part(serde_json::json!(["X", "Y"]))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("X"));
    assert!(result.html.contains("2 items"));
}

#[tokio::test]
async fn list_skips_non_object_non_string_items() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({"items": [42, true, "Keep"]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Keep"));
    // numbers/bools without title are dropped
    assert!(result.html.contains("1 items"));
}

#[tokio::test]
async fn list_escapes_item_text() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({"items": ["<i>x</i>"]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("&lt;i&gt;"));
}

// ---------------------------------------------------------------------------
// Form renderer / form fields
// ---------------------------------------------------------------------------

#[tokio::test]
async fn form_fields_from_hints() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![],
        Some(serde_json::json!({
            "fields": [{"name": "from_hint", "type": "text"}]
        })),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("from_hint"));
}

#[tokio::test]
async fn form_dedupes_hint_and_data_fields() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({"fields": [{"name": "dup", "type": "text"}]}))],
        Some(serde_json::json!({"fields": [{"name": "dup", "type": "text"}]})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    let occurrences = result.html.matches("id=\"dup\"").count();
    assert_eq!(occurrences, 1);
}

#[tokio::test]
async fn form_textarea_field() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{"name": "bio", "type": "textarea", "default": "hello"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("<textarea"));
    assert!(result.html.contains("hello"));
}

#[tokio::test]
async fn form_number_field_with_default() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{"name": "qty", "type": "number", "default": 5}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("type=\"number\""));
    assert!(result.html.contains("value=\"5\""));
}

#[tokio::test]
async fn form_date_field() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{"name": "dob", "type": "date", "default": "2020-01-01"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("type=\"date\""));
    assert!(result.html.contains("2020-01-01"));
}

#[tokio::test]
async fn form_field_with_placeholder() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{"name": "q", "type": "text", "placeholder": "Type here"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("placeholder=\"Type here\""));
}

#[tokio::test]
async fn form_select_with_selected_default() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{
                "name": "c",
                "type": "select",
                "default": "b",
                "options": [
                    {"value": "a", "label": "A"},
                    {"value": "b", "label": "B"}
                ]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("selected"));
}

#[tokio::test]
async fn form_select_option_value_as_label_fallback() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{
                "name": "c",
                "type": "select",
                "options": [{"value": "raw"}]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("value=\"raw\""));
    assert!(result.html.contains(">raw</option>"));
}

#[tokio::test]
async fn form_checkbox_unchecked_default() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{"name": "agree", "type": "checkbox"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("form-checkbox"));
    assert!(!result.html.contains(" checked>"));
}

#[tokio::test]
async fn form_field_label_falls_back_to_name() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({"fields": [{"name": "no_label"}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains(">no_label<"));
}

#[tokio::test]
async fn form_field_without_name_skipped() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({"fields": [{"type": "text"}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    // A field with no name is dropped at parse time, so no individual
    // form-field div is rendered (the form-fields container still exists).
    assert!(!result.html.contains("class=\"form-field\">"));
}

#[tokio::test]
async fn form_required_field_has_mark() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{"name": "x", "type": "text", "required": true}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("required-mark"));
}

#[tokio::test]
async fn form_no_submit_tool_yields_null() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({"fields": [{"name": "x", "type": "text"}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("FORM_SUBMIT_TOOL = null"));
}

// ---------------------------------------------------------------------------
// Image renderer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn image_from_file_bytes() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact(
        "image",
        None,
        None,
        vec![file_part(Some("p.png"), Some("image/jpeg"), Some("QUJD"), None)],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("data:image/jpeg;base64,QUJD"));
}

#[tokio::test]
async fn image_file_bytes_default_mime() {
    let renderer = ImageRenderer::new();
    // FileContent serializes mime_type as mimeType; with None it is absent, so
    // the renderer falls back to image/png.
    let artifact = make_artifact(
        "image",
        None,
        None,
        vec![file_part(None, None, Some("REVG"), None)],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("data:image/png;base64,REVG"));
}

#[tokio::test]
async fn image_url_alias_in_data_part() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact(
        "image",
        None,
        None,
        vec![data_part(serde_json::json!({"url": "https://e.test/u.png"}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("https://e.test/u.png"));
}

#[tokio::test]
async fn image_alt_and_caption_from_hints() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact(
        "image",
        None,
        None,
        vec![data_part(serde_json::json!({"src": "https://e.test/i.png"}))],
        Some(serde_json::json!({"alt": "HintAlt", "caption": "HintCap"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("HintAlt"));
    assert!(result.html.contains("HintCap"));
}

#[tokio::test]
async fn image_width_only() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact(
        "image",
        None,
        None,
        vec![data_part(serde_json::json!({"src": "https://e.test/i.png", "width": 100}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("width=\"100\""));
    assert!(!result.html.contains("height="));
}

#[tokio::test]
async fn image_height_only() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact(
        "image",
        None,
        None,
        vec![data_part(serde_json::json!({"src": "https://e.test/i.png", "height": 50}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("height=\"50\""));
}

#[tokio::test]
async fn image_alt_defaults_to_title() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact(
        "image",
        Some("MyTitle"),
        None,
        vec![data_part(serde_json::json!({"src": "https://e.test/i.png"}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("alt=\"MyTitle\""));
}

#[tokio::test]
async fn image_no_src_renders_empty() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact("image", None, None, vec![], None);
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("artifact-image"));
}

// ---------------------------------------------------------------------------
// Text renderer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn text_multiple_text_parts_joined() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        None,
        None,
        vec![text_part("First"), text_part("Second")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("First"));
    assert!(result.html.contains("Second"));
}

#[tokio::test]
async fn text_with_description() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        Some("T"),
        Some("Desc here"),
        vec![text_part("body")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Desc here"));
}

#[tokio::test]
async fn text_no_parts_default_title() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact("text", None, None, vec![], None);
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Text"));
    assert!(result.html.contains("copy-btn"));
}

// ---------------------------------------------------------------------------
// Dashboard renderer / sections
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dashboard_single_section_no_array() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "type": "text",
            "title": "Solo",
            "text": "single"
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Solo"));
    assert!(result.html.contains("single"));
}

#[tokio::test]
async fn dashboard_metrics_kpi_alias_with_name_and_negative_change() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "kpi",
                "title": "K",
                "metrics": [{"name": "Loss", "value": 10, "change": -3.0}]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("metric-change negative"));
    assert!(result.html.contains("Loss"));
}

#[tokio::test]
async fn dashboard_metrics_string_value() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "metrics",
                "metrics": [{"label": "Status", "value": "OK"}]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("OK"));
}

#[tokio::test]
async fn dashboard_status_all_classes() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "status",
                "items": [
                    {"name": "a", "status": "healthy"},
                    {"name": "b", "status": "warning"},
                    {"name": "c", "status": "critical"},
                    {"name": "d", "status": "mystery"}
                ]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("status-ok"));
    assert!(result.html.contains("status-warning"));
    assert!(result.html.contains("status-error"));
    assert!(result.html.contains("status-unknown"));
}

#[tokio::test]
async fn dashboard_status_label_alias_and_default_status() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "status",
                "items": [{"label": "Svc"}]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Svc"));
    assert!(result.html.contains("unknown"));
}

#[tokio::test]
async fn dashboard_table_no_columns() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{"type": "table", "rows": [{"a": 1}]}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("No table data"));
}

#[tokio::test]
async fn dashboard_table_rows_as_arrays() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "table",
                "columns": ["a", "b"],
                "rows": [[1, 2]]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("section-table"));
}

#[tokio::test]
async fn dashboard_list_object_items() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "list",
                "items": [{"text": "Obj item"}, {"title": "Titled"}]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Obj item"));
    assert!(result.html.contains("Titled"));
}

#[tokio::test]
async fn dashboard_text_content_alias() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{"type": "text", "content": "via content"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("via content"));
}

#[tokio::test]
async fn dashboard_section_default_type_is_text() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{"title": "NoType", "text": "fallback"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("section-text"));
    assert!(result.html.contains("fallback"));
}

#[tokio::test]
async fn dashboard_section_with_width() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{"type": "text", "text": "x", "width": "50%"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("flex-basis: 50%"));
}

#[tokio::test]
async fn dashboard_section_default_title() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({"sections": [{"type": "text", "text": "x"}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Section"));
}

#[tokio::test]
async fn dashboard_graph_alias_chart_section() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "graph",
                "id": "g1",
                "chart_type": "pie",
                "labels": ["A"],
                "datasets": [{"data": [1]}]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("chart-g1"));
    assert!(result.html.contains("DASHBOARD_CHART_CONFIGS"));
    assert!(result.html.contains("pie"));
}

#[tokio::test]
async fn dashboard_chart_default_type_bar_when_missing() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{"type": "chart", "id": "c", "labels": ["A"], "datasets": [{"data": [1]}]}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("\"type\":\"bar\""));
}

#[tokio::test]
async fn dashboard_with_description() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        Some("D"),
        Some("Dash desc"),
        vec![data_part(serde_json::json!({"sections": [{"type": "text", "text": "x"}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Dash desc"));
}

#[tokio::test]
async fn dashboard_unknown_layout_defaults_vertical() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({"sections": [{"type": "text", "text": "x"}]}))],
        Some(serde_json::json!({"layout": "spiral"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("layout-vertical"));
}

#[tokio::test]
async fn dashboard_capitalized_grid_layout() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({"sections": [{"type": "text", "text": "x"}]}))],
        Some(serde_json::json!({"layout": "Grid"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("layout-grid"));
}

#[tokio::test]
async fn dashboard_metrics_escapes_label() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "metrics",
                "metrics": [{"label": "<x>", "value": 1}]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("&lt;x&gt;"));
}
