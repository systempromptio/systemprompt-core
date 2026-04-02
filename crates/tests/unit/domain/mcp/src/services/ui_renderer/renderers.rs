use systemprompt_mcp::services::ui_renderer::templates::{
    ChartRenderer, DashboardRenderer, FormRenderer, ImageRenderer, ListRenderer, TableRenderer,
    TextRenderer,
};
use systemprompt_mcp::services::ui_renderer::{
    CspPolicy, MCP_APP_MIME_TYPE, UiMetadata, UiRenderer, UiResource,
};
use systemprompt_models::{
    ArtifactType, DataPart, Part, TextPart,
    A2aArtifact as Artifact, ArtifactMetadata,
};

fn make_artifact(
    artifact_type: &str,
    name: Option<&str>,
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
        name: name.map(String::from),
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
        }
    };
    Part::Data(DataPart { data: map })
}

fn text_part(text: &str) -> Part {
    Part::Text(TextPart {
        text: text.to_string(),
    })
}

#[test]
fn ui_resource_new_uses_default_csp() {
    let resource = UiResource::new("html".to_string());
    assert_eq!(resource.html, "html");
    assert!(resource.csp.to_header_value().is_empty());
}

#[test]
fn ui_resource_with_csp() {
    let resource = UiResource::new("html".to_string()).with_csp(CspPolicy::strict());
    assert!(resource.csp.to_header_value().contains("default-src"));
}

#[test]
fn ui_resource_mime_type() {
    assert_eq!(UiResource::mime_type(), MCP_APP_MIME_TYPE);
    assert_eq!(UiResource::mime_type(), "text/html;profile=mcp-app");
}

#[test]
fn ui_metadata_for_static_template() {
    let meta = UiMetadata::for_static_template("test-server");
    assert_eq!(meta.resource_uri, "ui://test-server/artifact-viewer");
    assert!(meta.prefers_border);
    assert!(meta.csp.is_none());
}

#[test]
fn ui_metadata_for_tool_definition() {
    let meta = UiMetadata::for_tool_definition("my-server");
    assert_eq!(meta.resource_uri, "ui://my-server/artifact-viewer");
    assert!(meta.prefers_border);
}

#[test]
fn ui_metadata_with_csp() {
    let meta = UiMetadata::for_static_template("s").with_csp(CspPolicy::strict());
    assert!(meta.csp.is_some());
}

#[test]
fn ui_metadata_with_prefers_border_false() {
    let meta = UiMetadata::for_static_template("s").with_prefers_border(false);
    assert!(!meta.prefers_border);
}

#[test]
fn ui_metadata_model_only() {
    let meta = UiMetadata::for_static_template("s").model_only();
    assert_eq!(meta.visibility.len(), 1);
}

#[test]
fn ui_metadata_to_json_contains_resource_uri() {
    let meta = UiMetadata::for_static_template("s");
    let json = meta.to_json();
    assert_eq!(json["resourceUri"], "ui://s/artifact-viewer");
}

#[test]
fn ui_metadata_to_json_includes_csp_when_set() {
    let meta = UiMetadata::for_static_template("s").with_csp(CspPolicy::strict());
    let json = meta.to_json();
    assert!(json.get("csp").is_some());
}

#[test]
fn ui_metadata_to_json_no_csp_when_unset() {
    let meta = UiMetadata::for_static_template("s");
    let json = meta.to_json();
    assert!(json.get("csp").is_none());
}

#[test]
fn ui_metadata_to_tool_meta_wraps_in_ui_key() {
    let meta = UiMetadata::for_static_template("s");
    let tool_meta = meta.to_tool_meta();
    assert!(tool_meta.contains_key("ui"));
}

#[test]
fn table_renderer_artifact_type() {
    let renderer = TableRenderer::new();
    assert_eq!(renderer.artifact_type(), ArtifactType::Table);
}

#[test]
fn table_renderer_supports_table() {
    let renderer = TableRenderer::new();
    assert!(renderer.supports("table"));
}

#[test]
fn table_renderer_does_not_support_chart() {
    let renderer = TableRenderer::new();
    assert!(!renderer.supports("chart"));
}

#[test]
fn table_renderer_csp_is_strict() {
    let renderer = TableRenderer::new();
    let csp = renderer.csp_policy();
    assert_eq!(csp.frame_src, vec!["'none'"]);
}

#[tokio::test]
async fn table_renderer_with_columns_and_data() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        Some("Users"),
        None,
        vec![data_part(serde_json::json!({
            "columns": ["name", "age"],
            "data": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25}
            ]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Users"));
    assert!(result.html.contains("data-table"));
}

#[tokio::test]
async fn table_renderer_object_with_columns_and_rows() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        Some("Sales"),
        Some("Monthly sales data"),
        vec![data_part(serde_json::json!({
            "columns": ["month", "revenue"],
            "data": [
                {"month": "Jan", "revenue": 100},
                {"month": "Feb", "revenue": 200}
            ]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Sales"));
    assert!(result.html.contains("Monthly sales data"));
}

#[tokio::test]
async fn table_renderer_empty_data() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact("table", Some("Empty"), None, vec![], None);
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Empty"));
}

#[tokio::test]
async fn table_renderer_with_filterable_hint() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        None,
        None,
        vec![data_part(serde_json::json!({"columns": ["a"], "data": [{"a": 1}]}))],
        Some(serde_json::json!({"filterable": true})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("filter-input"));
}

#[tokio::test]
async fn table_renderer_with_pagination_hint() {
    let renderer = TableRenderer::new();
    let artifact = make_artifact(
        "table",
        None,
        None,
        vec![data_part(serde_json::json!({"columns": ["a"], "data": [{"a": 1}]}))],
        Some(serde_json::json!({"page_size": 10})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("pagination"));
}

#[test]
fn chart_renderer_artifact_type() {
    let renderer = ChartRenderer::new();
    assert_eq!(renderer.artifact_type(), ArtifactType::Chart);
}

#[test]
fn chart_renderer_csp_includes_jsdelivr() {
    let renderer = ChartRenderer::new();
    let csp = renderer.csp_policy();
    assert!(csp.script_src.contains(&"https://cdn.jsdelivr.net".to_string()));
}

#[tokio::test]
async fn chart_renderer_bar_chart() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        Some("Sales Chart"),
        None,
        vec![data_part(serde_json::json!({
            "labels": ["Jan", "Feb", "Mar"],
            "datasets": [{"label": "Revenue", "data": [100, 200, 150]}]
        }))],
        Some(serde_json::json!({"chart_type": "bar"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Sales Chart"));
    assert!(result.html.contains("chart"));
}

#[tokio::test]
async fn chart_renderer_line_chart() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        Some("Trend"),
        None,
        vec![data_part(serde_json::json!({
            "labels": ["Q1", "Q2"],
            "datasets": [{"label": "Growth", "data": [10, 20]}]
        }))],
        Some(serde_json::json!({"chart_type": "line"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Trend"));
}

#[tokio::test]
async fn chart_renderer_pie_chart() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::json!({
            "labels": ["A", "B"],
            "datasets": [{"data": [60, 40]}]
        }))],
        Some(serde_json::json!({"chart_type": "pie"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("canvas"));
}

#[tokio::test]
async fn chart_renderer_with_axis_labels() {
    let renderer = ChartRenderer::new();
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::json!({
            "labels": ["A"],
            "data": [1]
        }))],
        Some(serde_json::json!({
            "chart_type": "bar",
            "x_axis_label": "Category",
            "y_axis_label": "Value"
        })),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("CHART_CONFIG"));
}

#[test]
fn text_renderer_artifact_type() {
    let renderer = TextRenderer::new();
    assert_eq!(renderer.artifact_type(), ArtifactType::Text);
}

#[tokio::test]
async fn text_renderer_simple_text() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        Some("Note"),
        None,
        vec![text_part("Hello, world!")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Hello, world!"));
    assert!(result.html.contains("Note"));
}

#[tokio::test]
async fn text_renderer_multiline_text() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        None,
        None,
        vec![text_part("Line 1\nLine 2")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("<p>Line 1</p>"));
    assert!(result.html.contains("<p>Line 2</p>"));
}

#[tokio::test]
async fn text_renderer_escapes_html() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        None,
        None,
        vec![text_part("<script>alert('xss')</script>")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("&lt;script&gt;"));
    assert!(!result.html.contains("<script>alert"));
}

#[tokio::test]
async fn text_renderer_empty_lines_use_nbsp() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        None,
        None,
        vec![text_part("Before\n\nAfter")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("&nbsp;"));
}

#[tokio::test]
async fn text_renderer_copy_button() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        None,
        None,
        vec![text_part("Copy me")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("copy-btn"));
}

#[test]
fn list_renderer_artifact_type() {
    let renderer = ListRenderer::new();
    assert_eq!(renderer.artifact_type(), ArtifactType::List);
}

#[tokio::test]
async fn list_renderer_simple_string_items() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        Some("Todo"),
        None,
        vec![data_part(serde_json::json!({"items": ["Item 1", "Item 2", "Item 3"]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Item 1"));
    assert!(result.html.contains("3 items"));
}

#[tokio::test]
async fn list_renderer_object_items_with_title() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({
            "items": [
                {"title": "First", "description": "Desc 1"},
                {"title": "Second"}
            ]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("First"));
    assert!(result.html.contains("Desc 1"));
}

#[tokio::test]
async fn list_renderer_ordered_style() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({"items": ["A"]}))],
        Some(serde_json::json!({"style": "ordered"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("<ol"));
    assert!(result.html.contains("ordered-list"));
}

#[tokio::test]
async fn list_renderer_unordered_style_default() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({"items": ["A"]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("<ul"));
    assert!(result.html.contains("unordered-list"));
}

#[tokio::test]
async fn list_renderer_items_with_links() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact(
        "list",
        None,
        None,
        vec![data_part(serde_json::json!({
            "items": [{"title": "Google", "link": "https://google.com"}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("href="));
    assert!(result.html.contains("noopener"));
}

#[tokio::test]
async fn list_renderer_empty_list() {
    let renderer = ListRenderer::new();
    let artifact = make_artifact("list", None, None, vec![], None);
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("0 items"));
}

#[test]
fn form_renderer_artifact_type() {
    let renderer = FormRenderer::new();
    assert_eq!(renderer.artifact_type(), ArtifactType::Form);
}

#[tokio::test]
async fn form_renderer_text_fields() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        Some("Contact"),
        None,
        vec![data_part(serde_json::json!({
            "fields": [
                {"name": "username", "type": "text", "label": "Username", "required": true},
                {"name": "email", "type": "email", "label": "Email"}
            ]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Contact"));
    assert!(result.html.contains("username"));
    assert!(result.html.contains("email"));
    assert!(result.html.contains("required"));
}

#[tokio::test]
async fn form_renderer_select_field() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{
                "name": "country",
                "type": "select",
                "options": [
                    {"value": "us", "label": "United States"},
                    {"value": "uk", "label": "United Kingdom"}
                ]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("<select"));
    assert!(result.html.contains("United States"));
}

#[tokio::test]
async fn form_renderer_checkbox_field() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({
            "fields": [{"name": "agree", "type": "checkbox", "default": true}]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("checkbox"));
    assert!(result.html.contains("checked"));
}

#[tokio::test]
async fn form_renderer_submit_tool_hint() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({"fields": [{"name": "x", "type": "text"}]}))],
        Some(serde_json::json!({"submit_tool": "my_tool"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("FORM_SUBMIT_TOOL"));
    assert!(result.html.contains("my_tool"));
}

#[tokio::test]
async fn form_renderer_has_submit_and_reset_buttons() {
    let renderer = FormRenderer::new();
    let artifact = make_artifact(
        "form",
        None,
        None,
        vec![data_part(serde_json::json!({"fields": [{"name": "x", "type": "text"}]}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("submit-btn"));
    assert!(result.html.contains("reset-btn"));
}

#[test]
fn image_renderer_artifact_type() {
    let renderer = ImageRenderer::new();
    assert_eq!(renderer.artifact_type(), ArtifactType::Image);
}

#[test]
fn image_renderer_csp_allows_https_and_blob() {
    let renderer = ImageRenderer::new();
    let csp = renderer.csp_policy();
    assert!(csp.img_src.contains(&"https:".to_string()));
    assert!(csp.img_src.contains(&"blob:".to_string()));
}

#[tokio::test]
async fn image_renderer_data_uri() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact(
        "image",
        Some("Photo"),
        None,
        vec![data_part(serde_json::json!({
            "src": "https://example.com/image.png",
            "alt": "A photo",
            "caption": "My photo"
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Photo"));
    assert!(result.html.contains("https://example.com/image.png"));
    assert!(result.html.contains("A photo"));
    assert!(result.html.contains("My photo"));
}

#[tokio::test]
async fn image_renderer_with_dimensions() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact(
        "image",
        None,
        None,
        vec![data_part(serde_json::json!({
            "src": "https://example.com/img.png",
            "width": 800,
            "height": 600
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("width=\"800\""));
    assert!(result.html.contains("height=\"600\""));
}

#[tokio::test]
async fn image_renderer_zoom_controls() {
    let renderer = ImageRenderer::new();
    let artifact = make_artifact(
        "image",
        None,
        None,
        vec![data_part(serde_json::json!({"src": "https://example.com/img.png"}))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("zoom-in"));
    assert!(result.html.contains("zoom-out"));
    assert!(result.html.contains("zoom-reset"));
}

#[test]
fn dashboard_renderer_artifact_type() {
    let renderer = DashboardRenderer::new();
    assert_eq!(renderer.artifact_type(), ArtifactType::Dashboard);
}

#[test]
fn dashboard_renderer_csp_includes_jsdelivr() {
    let renderer = DashboardRenderer::new();
    let csp = renderer.csp_policy();
    assert!(csp.script_src.contains(&"https://cdn.jsdelivr.net".to_string()));
}

#[tokio::test]
async fn dashboard_renderer_vertical_layout() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        Some("Overview"),
        None,
        vec![data_part(serde_json::json!({
            "sections": [
                {"type": "text", "title": "Welcome", "text": "Hello"},
            ]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Overview"));
    assert!(result.html.contains("layout-vertical"));
}

#[tokio::test]
async fn dashboard_renderer_grid_layout() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{"type": "text", "title": "S1", "text": "A"}]
        }))],
        Some(serde_json::json!({"layout": "grid"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("layout-grid"));
}

#[tokio::test]
async fn dashboard_renderer_tabs_layout() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [
                {"type": "text", "title": "Tab 1", "id": "tab1", "text": "Content 1"},
                {"type": "text", "title": "Tab 2", "id": "tab2", "text": "Content 2"}
            ]
        }))],
        Some(serde_json::json!({"layout": "tabs"})),
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("layout-tabs"));
    assert!(result.html.contains("tabs-nav"));
    assert!(result.html.contains("Tab 1"));
    assert!(result.html.contains("Tab 2"));
}

#[tokio::test]
async fn dashboard_renderer_metrics_section() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "metrics",
                "title": "KPIs",
                "metrics": [
                    {"label": "Revenue", "value": 1000.50, "unit": "$", "change": 5.2},
                    {"label": "Users", "value": 42}
                ]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("KPIs"));
    assert!(result.html.contains("metric-card"));
    assert!(result.html.contains("Revenue"));
}

#[tokio::test]
async fn dashboard_renderer_status_section() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "status",
                "title": "Services",
                "items": [
                    {"name": "API", "status": "ok"},
                    {"name": "DB", "status": "error"}
                ]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Services"));
    assert!(result.html.contains("status-ok"));
    assert!(result.html.contains("status-error"));
}

#[tokio::test]
async fn dashboard_renderer_table_section() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "table",
                "title": "Data",
                "columns": ["name", "value"],
                "rows": [{"name": "A", "value": "1"}]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("section-table"));
}

#[tokio::test]
async fn dashboard_renderer_chart_section() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "chart",
                "title": "Trend",
                "id": "my-chart",
                "chart_type": "line",
                "labels": ["A", "B"],
                "datasets": [{"label": "D", "data": [1, 2]}]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("chart-container"));
    assert!(result.html.contains("DASHBOARD_CHART_CONFIGS"));
}

#[tokio::test]
async fn dashboard_renderer_list_section() {
    let renderer = DashboardRenderer::new();
    let artifact = make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::json!({
            "sections": [{
                "type": "list",
                "title": "Items",
                "items": ["One", "Two"]
            }]
        }))],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("section-list"));
    assert!(result.html.contains("One"));
}

#[tokio::test]
async fn all_renderers_produce_valid_html_structure() {
    let table = TableRenderer::new();
    let chart = ChartRenderer::new();
    let text = TextRenderer::new();
    let list = ListRenderer::new();
    let form = FormRenderer::new();
    let image = ImageRenderer::new();
    let dashboard = DashboardRenderer::new();

    let simple_data = vec![data_part(serde_json::json!({"text": "test"}))];
    let text_data = vec![text_part("test")];

    let artifacts = vec![
        make_artifact("table", None, None, simple_data.clone(), None),
        make_artifact("chart", None, None, simple_data.clone(), None),
        make_artifact("text", None, None, text_data, None),
        make_artifact("list", None, None, simple_data.clone(), None),
        make_artifact("form", None, None, simple_data.clone(), None),
        make_artifact("image", None, None, simple_data.clone(), None),
        make_artifact("dashboard", None, None, simple_data, None),
    ];

    let renderers: Vec<Box<dyn UiRenderer>> = vec![
        Box::new(table),
        Box::new(chart),
        Box::new(text),
        Box::new(list),
        Box::new(form),
        Box::new(image),
        Box::new(dashboard),
    ];

    for (renderer, artifact) in renderers.iter().zip(artifacts.iter()) {
        let result = renderer.render(artifact).await.unwrap();
        assert!(result.html.contains("<!DOCTYPE html>"));
        assert!(result.html.contains("</html>"));
    }
}
