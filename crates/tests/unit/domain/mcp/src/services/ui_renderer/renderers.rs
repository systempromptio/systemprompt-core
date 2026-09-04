use systemprompt_mcp::services::ui_renderer::templates::{
    ChartRenderer, DashboardRenderer, FormRenderer, ImageRenderer, ListRenderer, TableRenderer,
    TextRenderer,
};
use systemprompt_mcp::services::ui_renderer::{
    CspPolicy, MCP_APP_MIME_TYPE, UiMetadata, UiRenderer, UiResource,
};
use systemprompt_models::artifacts::chart::{ChartArtifact, ChartDataset};
use systemprompt_models::artifacts::dashboard::{
    ChartSectionData, DashboardArtifact, DashboardHints, DashboardSection, ItemList, LayoutMode,
    ListItem, ListSectionData, MetricCard, MetricsCardsData, SectionType, ServiceStatus,
    StatusSectionData, TableSectionData, TextSectionData,
};
use systemprompt_models::artifacts::types::ChartType;
use systemprompt_models::{
    A2aArtifact as Artifact, ArtifactMetadata, ArtifactType, DataPart, Part, TextPart,
};

fn dashboard_artifact(dashboard: &DashboardArtifact) -> Artifact {
    make_artifact(
        "dashboard",
        None,
        None,
        vec![data_part(serde_json::to_value(dashboard).unwrap())],
        None,
    )
}

fn text_section(id: &str, title: &str, text: &str) -> DashboardSection {
    DashboardSection::new(id, title, SectionType::Text)
        .with_data(TextSectionData::new(text))
        .unwrap()
}

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
        title: name.map(String::from),
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
    assert_eq!(
        meta.csp.as_ref().map(|c| c.to_header_value()),
        Some(CspPolicy::strict().to_header_value())
    );
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
    let csp = json.get("csp").expect("csp present when set");
    assert_eq!(
        csp.as_str(),
        Some(CspPolicy::strict().to_header_value().as_str())
    );
    assert!(
        csp.as_str().is_some_and(|s| s.contains("default-src")),
        "serialized csp must carry the header directives: {csp}"
    );
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
        vec![data_part(
            serde_json::json!({"columns": ["a"], "data": [{"a": 1}]}),
        )],
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
        vec![data_part(
            serde_json::json!({"columns": ["a"], "data": [{"a": 1}]}),
        )],
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
fn chart_renderer_csp_is_strict() {
    let renderer = ChartRenderer::new();
    let csp = renderer.csp_policy();
    assert_eq!(csp.script_src, CspPolicy::strict().script_src);
    assert!(!csp.script_src.iter().any(|src| src.contains("://")));
    assert_eq!(csp.frame_src, vec!["'none'"]);
}

#[tokio::test]
async fn chart_renderer_bar_chart() {
    let renderer = ChartRenderer::new();
    let chart = ChartArtifact::new("Sales Chart", ChartType::Bar)
        .with_labels(vec!["Jan".into(), "Feb".into(), "Mar".into()])
        .with_datasets(vec![ChartDataset::new(
            "Revenue",
            vec![100.0, 200.0, 150.0],
        )]);
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::to_value(&chart).unwrap())],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Sales Chart"));
    assert!(result.html.contains("<svg class=\"chart-svg\""));
    assert_eq!(result.html.matches("class=\"chart-bar\"").count(), 3);
    assert!(!result.html.contains("<canvas"));
}

#[tokio::test]
async fn chart_renderer_line_chart() {
    let renderer = ChartRenderer::new();
    let chart = ChartArtifact::new("Trend", ChartType::Line)
        .with_labels(vec!["Q1".into(), "Q2".into()])
        .with_datasets(vec![ChartDataset::new("Growth", vec![10.0, 20.0])]);
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::to_value(&chart).unwrap())],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("Trend"));
    assert!(result.html.contains("class=\"chart-line\""));
    assert_eq!(result.html.matches("class=\"chart-point\"").count(), 2);
}

#[tokio::test]
async fn chart_renderer_pie_chart() {
    let renderer = ChartRenderer::new();
    let chart = ChartArtifact::new("Split", ChartType::Pie)
        .with_labels(vec!["A".into(), "B".into()])
        .with_datasets(vec![ChartDataset::new("Share", vec![60.0, 40.0])]);
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::to_value(&chart).unwrap())],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(!result.html.contains("<canvas"));
    assert_eq!(result.html.matches("class=\"chart-slice\"").count(), 2);
    assert!(result.html.contains("A: 60 (60%)"));
}

#[tokio::test]
async fn chart_renderer_with_axis_labels() {
    let renderer = ChartRenderer::new();
    let chart = ChartArtifact::new("Bars", ChartType::Bar)
        .with_labels(vec!["A".into()])
        .with_datasets(vec![ChartDataset::new("Data", vec![1.0])])
        .with_axes("Category", "Value");
    let artifact = make_artifact(
        "chart",
        None,
        None,
        vec![data_part(serde_json::to_value(&chart).unwrap())],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("class=\"chart-axis-label\""));
    assert!(result.html.contains("Category"));
    assert!(result.html.contains("Value"));
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
    let artifact = make_artifact("text", None, None, vec![text_part("Line 1\nLine 2")], None);
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
async fn text_renderer_drops_blank_lines() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        None,
        None,
        vec![text_part("Before\n\n\nAfter")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains("<p>Before</p>"));
    assert!(result.html.contains("<p>After</p>"));
    // A blank line is separation, not content: however many were typed, the gap
    // comes from `p + p` rather than from empty paragraphs.
    assert!(!result.html.contains("&nbsp;"));
}

#[tokio::test]
async fn text_renderer_formats_bullets_and_emphasis() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        None,
        None,
        vec![text_part("- **[23] Follow up** with `crm.lead`\n- Second item")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(result.html.contains(r#"<ul class="text-list">"#));
    assert!(result.html.contains("<strong>[23] Follow up</strong>"));
    assert!(result.html.contains("<code>crm.lead</code>"));
    // Counted over the rendered body rather than the whole document: the
    // stylesheet is inlined into the same string and its comments name the very
    // tags being counted.
    let body = result
        .html
        .split(r#"id="text-content""#)
        .nth(1)
        .expect("the rendered body");
    // One list, not one per item.
    assert_eq!(body.matches("<ul").count(), 1);
    assert_eq!(body.matches("<li>").count(), 2);
}

#[tokio::test]
async fn text_renderer_leaves_unpaired_markers_alone() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact(
        "text",
        None,
        None,
        vec![text_part("2 * 3 * 4 is a product, not **emphasis")],
        None,
    );
    let result = renderer.render(&artifact).await.unwrap();
    assert!(!result.html.contains("<strong>"));
}

#[tokio::test]
async fn text_renderer_copy_button() {
    let renderer = TextRenderer::new();
    let artifact = make_artifact("text", None, None, vec![text_part("Copy me")], None);
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
        vec![data_part(
            serde_json::json!({"items": ["Item 1", "Item 2", "Item 3"]}),
        )],
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
        vec![data_part(
            serde_json::json!({"fields": [{"name": "x", "type": "text"}]}),
        )],
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
        vec![data_part(
            serde_json::json!({"fields": [{"name": "x", "type": "text"}]}),
        )],
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
        vec![data_part(
            serde_json::json!({"src": "https://example.com/img.png"}),
        )],
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
fn dashboard_renderer_csp_is_strict() {
    let renderer = DashboardRenderer::new();
    let csp = renderer.csp_policy();
    assert_eq!(csp.script_src, CspPolicy::strict().script_src);
    assert!(!csp.script_src.iter().any(|src| src.contains("://")));
    assert_eq!(csp.frame_src, vec!["'none'"]);
}

#[tokio::test]
async fn dashboard_renderer_vertical_layout() {
    let renderer = DashboardRenderer::new();
    let dashboard =
        DashboardArtifact::new("Overview").add_section(text_section("welcome", "Welcome", "Hello"));
    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .unwrap();
    assert!(result.html.contains("Overview"));
    assert!(result.html.contains("layout-vertical"));
    assert!(result.html.contains("Hello"));
}

#[tokio::test]
async fn dashboard_renderer_grid_layout() {
    let renderer = DashboardRenderer::new();
    let dashboard = DashboardArtifact::new("Grid")
        .add_section(text_section("s1", "S1", "A"))
        .with_hints(DashboardHints::new().with_layout(LayoutMode::Grid));
    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .unwrap();
    assert!(result.html.contains("layout-grid"));
}

#[tokio::test]
async fn dashboard_renderer_tabs_layout() {
    let renderer = DashboardRenderer::new();
    let dashboard = DashboardArtifact::new("Tabs")
        .add_section(text_section("tab1", "Tab 1", "Content 1"))
        .add_section(text_section("tab2", "Tab 2", "Content 2"))
        .with_hints(DashboardHints::new().with_layout(LayoutMode::Tabs));
    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .unwrap();
    assert!(result.html.contains("layout-tabs"));
    assert!(result.html.contains("tabs-nav"));
    assert!(result.html.contains("Tab 1"));
    assert!(result.html.contains("Tab 2"));
}

#[tokio::test]
async fn dashboard_renderer_metrics_section() {
    let renderer = DashboardRenderer::new();
    let dashboard = DashboardArtifact::new("KPIs").add_section(
        DashboardSection::new("kpis", "KPIs", SectionType::MetricsCards)
            .with_data(MetricsCardsData::new(vec![
                MetricCard::new("Revenue", "$1000.50").with_subtitle("+5.2%"),
                MetricCard::new("Users", "42"),
            ]))
            .unwrap(),
    );
    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .unwrap();
    assert!(result.html.contains("KPIs"));
    assert!(result.html.contains("metric-card"));
    assert!(result.html.contains("Revenue"));
    assert!(result.html.contains("+5.2%"));
}

#[tokio::test]
async fn dashboard_renderer_status_section() {
    let renderer = DashboardRenderer::new();
    let dashboard = DashboardArtifact::new("Health").add_section(
        DashboardSection::new("svcs", "Services", SectionType::Status)
            .with_data(StatusSectionData::new(vec![
                ServiceStatus::new("API", "ok"),
                ServiceStatus::new("DB", "error"),
            ]))
            .unwrap(),
    );
    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .unwrap();
    assert!(result.html.contains("Services"));
    assert!(result.html.contains("status-ok"));
    assert!(result.html.contains("status-error"));
}

#[tokio::test]
async fn dashboard_renderer_table_section() {
    let renderer = DashboardRenderer::new();
    let dashboard = DashboardArtifact::new("Data").add_section(
        DashboardSection::new("data", "Data", SectionType::Table)
            .with_data(TableSectionData::new(
                vec!["name".into(), "value".into()],
                vec![serde_json::json!({"name": "A", "value": "1"})],
            ))
            .unwrap(),
    );
    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .unwrap();
    assert!(result.html.contains("section-table"));
    assert!(result.html.contains("<td>A</td>"));
}

#[tokio::test]
async fn dashboard_renderer_chart_section() {
    let renderer = DashboardRenderer::new();
    let dashboard = DashboardArtifact::new("Trends").add_section(
        DashboardSection::new("my-chart", "Trend", SectionType::Chart)
            .with_data(ChartSectionData::new(
                "line",
                vec!["A".into(), "B".into()],
                vec![ChartDataset::new("D", vec![1.0, 2.0])],
            ))
            .unwrap(),
    );
    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .unwrap();
    assert!(result.html.contains("chart-container"));
    assert!(result.html.contains("<svg class=\"chart-svg\""));
    assert!(result.html.contains("class=\"chart-line\""));
    assert!(!result.html.contains("<canvas"));
}

#[tokio::test]
async fn dashboard_renderer_list_section() {
    let renderer = DashboardRenderer::new();
    let dashboard = DashboardArtifact::new("Lists").add_section(
        DashboardSection::new("items", "Items", SectionType::List)
            .with_data(ListSectionData::new(vec![ItemList::new(
                "Top",
                vec![
                    ListItem::new(1, "One", "10"),
                    ListItem::new(2, "Two", "20").with_badge("new"),
                ],
            )]))
            .unwrap(),
    );
    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .unwrap();
    assert!(result.html.contains("section-list"));
    assert!(result.html.contains("One"));
    assert!(result.html.contains("list-badge"));
}

#[tokio::test]
async fn dashboard_renderer_sections_sorted_by_order() {
    let renderer = DashboardRenderer::new();
    let dashboard = DashboardArtifact::new("Ordered")
        .add_section(text_section("second", "Second", "b").with_order(1))
        .add_section(text_section("first", "First", "a").with_order(0));
    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .unwrap();
    let first = result.html.find("First").unwrap();
    let second = result.html.find("Second").unwrap();
    assert!(first < second);
}

#[tokio::test]
async fn dashboard_renderer_isolates_mismatched_section_data() {
    let renderer = DashboardRenderer::new();
    let dashboard = DashboardArtifact::new("Broken")
        .add_section(
            DashboardSection::new("m", "Metrics", SectionType::MetricsCards)
                .with_data(serde_json::json!({"metrics": [{"label": "x", "value": 1}]}))
                .unwrap(),
        )
        .add_section(text_section("ok", "Still Here", "readable"));

    let result = renderer
        .render(&dashboard_artifact(&dashboard))
        .await
        .expect("a bad section must not fail the whole dashboard");

    // The bad section reports itself in place...
    assert!(result.html.contains("error-message"));
    assert!(result.html.contains("Metrics"));
    // ...and every good section beside it still renders. Failing the whole
    // render meant one malformed payload blanked the entire dashboard.
    assert!(result.html.contains("Still Here"));
    assert!(result.html.contains("readable"));
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
    let chart_payload = ChartArtifact::new("C", ChartType::Bar)
        .with_labels(vec!["A".into()])
        .with_datasets(vec![ChartDataset::new("D", vec![1.0])]);
    let dashboard_payload = DashboardArtifact::new("D").add_section(text_section("s", "S", "test"));

    let artifacts = vec![
        make_artifact("table", None, None, simple_data.clone(), None),
        make_artifact(
            "chart",
            None,
            None,
            vec![data_part(serde_json::to_value(&chart_payload).unwrap())],
            None,
        ),
        make_artifact("text", None, None, text_data, None),
        make_artifact("list", None, None, simple_data.clone(), None),
        make_artifact("form", None, None, simple_data.clone(), None),
        make_artifact("image", None, None, simple_data, None),
        dashboard_artifact(&dashboard_payload),
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

// Table sections beyond the plain case.
//
// The existing table test renders two string columns and stops, which leaves
// the parts that decide what the operator actually sees — server-side sorting,
// and how each JSON type becomes a cell — unexercised.

fn table_dashboard(data: TableSectionData) -> Artifact {
    dashboard_artifact(
        &DashboardArtifact::new("Data").add_section(
            DashboardSection::new("data", "Data", SectionType::Table)
                .with_data(data)
                .unwrap(),
        ),
    )
}

async fn table_html(data: TableSectionData) -> String {
    DashboardRenderer::new()
        .render(&table_dashboard(data))
        .await
        .expect("a table section should render")
        .html
}

fn positions(html: &str, needles: &[&str]) -> Vec<usize> {
    needles
        .iter()
        .map(|n| {
            html.find(n)
                .unwrap_or_else(|| panic!("{n} missing from rendered table:\n{html}"))
        })
        .collect()
}

fn sorted(
    columns: &[&str],
    rows: Vec<serde_json::Value>,
    column: &str,
    order: &str,
) -> TableSectionData {
    let mut data = TableSectionData::new(columns.iter().map(|c| (*c).to_owned()).collect(), rows);
    data.default_sort = Some(systemprompt_models::artifacts::dashboard::SortConfig {
        column: column.to_owned(),
        order: order.to_owned(),
    });
    data
}

// Why: the source records that `default_sort` was declared by the model and
// never applied, and that sorting server-side is what keeps the no-JS rendering
// correct. A test that only checks the cells are present cannot tell sorted
// output from unsorted, so these assert on the order rows appear in the HTML.
#[tokio::test]
async fn a_declared_ascending_sort_orders_the_rendered_rows() {
    let html = table_html(sorted(
        &["name", "value"],
        vec![
            serde_json::json!({"name": "charlie", "value": 3}),
            serde_json::json!({"name": "alpha", "value": 1}),
            serde_json::json!({"name": "bravo", "value": 2}),
        ],
        "name",
        "asc",
    ))
    .await;

    let p = positions(&html, &["alpha", "bravo", "charlie"]);
    assert!(
        p[0] < p[1] && p[1] < p[2],
        "rows must be emitted in ascending order, got positions {p:?}"
    );
}

#[tokio::test]
async fn a_declared_descending_sort_reverses_the_rendered_rows() {
    let html = table_html(sorted(
        &["name", "value"],
        vec![
            serde_json::json!({"name": "alpha", "value": 1}),
            serde_json::json!({"name": "charlie", "value": 3}),
            serde_json::json!({"name": "bravo", "value": 2}),
        ],
        "name",
        "desc",
    ))
    .await;

    let p = positions(&html, &["charlie", "bravo", "alpha"]);
    assert!(
        p[0] < p[1] && p[1] < p[2],
        "desc must reverse the ordering, got positions {p:?}"
    );
}

// Why: cells are formatted before comparison, so a numeric column sorted as
// text would place 10 before 9. The parse-as-f64 branch exists to prevent
// exactly that, and only a value crossing a digit boundary can detect it.
#[tokio::test]
async fn a_numeric_column_sorts_by_magnitude_rather_than_as_text() {
    let html = table_html(sorted(
        &["n"],
        vec![
            serde_json::json!({"n": 9}),
            serde_json::json!({"n": 10}),
            serde_json::json!({"n": 100}),
        ],
        "n",
        "asc",
    ))
    .await;

    let p = positions(&html, &["<td>9</td>", "<td>10</td>", "<td>100</td>"]);
    assert!(
        p[0] < p[1] && p[1] < p[2],
        "9 must precede 10 and 100; text ordering would put 10 and 100 first: {p:?}"
    );
}

// Why: a sort naming a column that is not in the table must render rather than
// panic or drop rows — the model chose the column name and can get it wrong.
#[tokio::test]
async fn a_sort_on_an_unknown_column_still_renders_every_row() {
    let html = table_html(sorted(
        &["name"],
        vec![
            serde_json::json!({"name": "alpha"}),
            serde_json::json!({"name": "bravo"}),
        ],
        "column-that-does-not-exist",
        "asc",
    ))
    .await;

    assert!(html.contains("alpha") && html.contains("bravo"), "{html}");
}

// Why: the renderer has to produce a table with no JavaScript, so the sortable
// affordance is markup. Without the ARIA role and tabindex a keyboard user
// cannot reach the control at all.
#[tokio::test]
async fn a_sortable_table_marks_its_headers_as_reachable_controls() {
    let mut data =
        TableSectionData::new(vec!["name".into()], vec![serde_json::json!({"name": "a"})]);
    data.sortable = Some(true);

    let html = table_html(data).await;

    assert!(html.contains("sortable"), "{html}");
    assert!(
        html.contains(r#"tabindex="0""#) && html.contains(r#"role="button""#),
        "a sortable header must be keyboard-reachable: {html}"
    );
}

#[tokio::test]
async fn a_table_that_is_not_sortable_does_not_advertise_the_control() {
    let html = table_html(TableSectionData::new(
        vec!["name".into()],
        vec![serde_json::json!({"name": "a"})],
    ))
    .await;

    assert!(
        !html.contains(r#"role="button""#),
        "an unsortable header must not look like a control: {html}"
    );
}

// Why: rows arrive as untyped JSON, so every variant reaches the formatter. A
// bare `true` or a null rendered as Rust's Debug output is what an operator
// would end up reading.
#[tokio::test]
async fn each_json_cell_type_is_rendered_for_a_human() {
    let html = table_html(TableSectionData::new(
        vec!["b".into(), "empty".into(), "n".into()],
        vec![serde_json::json!({"b": true, "empty": serde_json::Value::Null, "n": 1234567})],
    ))
    .await;

    assert!(
        html.contains("<td>Yes</td>"),
        "a boolean reads as Yes/No: {html}"
    );
    assert!(
        html.contains("<td></td>"),
        "a null is an empty cell: {html}"
    );
    assert!(
        html.contains("1,234,567"),
        "a large whole number is digit-grouped so it can be read at a glance: {html}"
    );
}

#[tokio::test]
async fn a_table_with_no_rows_says_so_rather_than_rendering_an_empty_grid() {
    let html = table_html(TableSectionData::new(vec!["name".into()], vec![])).await;

    assert!(
        html.contains("No rows to show."),
        "an empty table must explain itself: {html}"
    );
}

// Presentation-card section bodies.
//
// A card section's `content` is untyped JSON, so the renderer branches on the
// shape the model produced. Nothing exercised those branches, which means the
// list and key/value renderings — the two an operator is most likely to see —
// were unverified.

fn card_section(
    heading: &str,
    content: serde_json::Value,
) -> systemprompt_models::artifacts::card::CardSection {
    systemprompt_models::artifacts::card::CardSection {
        heading: heading.to_owned(),
        content,
        icon: None,
    }
}

async fn card_html(card: systemprompt_models::artifacts::card::PresentationCardArtifact) -> String {
    systemprompt_mcp::services::ui_renderer::templates::PresentationCardRenderer::new()
        .render(&make_artifact(
            "presentation_card",
            None,
            None,
            vec![data_part(serde_json::to_value(&card).unwrap())],
            None,
        ))
        .await
        .expect("a presentation card should render")
        .html
}

fn card_with(
    section: systemprompt_models::artifacts::card::CardSection,
) -> systemprompt_models::artifacts::card::PresentationCardArtifact {
    systemprompt_models::artifacts::card::PresentationCardArtifact::new("Report")
        .add_section(section)
}

#[tokio::test]
async fn a_card_section_holding_an_array_renders_as_a_list() {
    let html = card_html(card_with(card_section(
        "Findings",
        serde_json::json!(["first finding", "second finding"]),
    )))
    .await;

    assert!(html.contains("card-section-list"), "{html}");
    assert!(
        html.contains("<li>first finding</li>") && html.contains("<li>second finding</li>"),
        "every array element must become its own item: {html}"
    );
}

#[tokio::test]
async fn a_card_section_holding_an_object_renders_as_labelled_pairs() {
    let html = card_html(card_with(card_section(
        "Details",
        serde_json::json!({"status": "green", "owner": "platform"}),
    )))
    .await;

    assert!(html.contains("card-section-pairs"), "{html}");
    assert!(
        html.contains("status") && html.contains("green"),
        "a key and its value must both survive: {html}"
    );
}

// Why: card content comes from a model, so it can carry text that looks like
// markup. Rendering it unescaped would let generated output inject nodes into
// the operator's page.
#[tokio::test]
async fn card_content_is_escaped_rather_than_rendered_as_markup() {
    let html = card_html(card_with(card_section(
        "Findings",
        serde_json::json!(["<script>alert(1)</script>"]),
    )))
    .await;

    assert!(
        !html.contains("<script>alert(1)</script>"),
        "model-authored content must never reach the page as live markup: {html}"
    );
    assert!(
        html.contains("&lt;script&gt;"),
        "it should appear escaped: {html}"
    );
}

#[tokio::test]
async fn an_object_key_is_escaped_as_well_as_its_value() {
    let html = card_html(card_with(card_section(
        "Details",
        serde_json::json!({"<b>key</b>": "<i>value</i>"}),
    )))
    .await;

    assert!(
        !html.contains("<b>key</b>") && !html.contains("<i>value</i>"),
        "both halves of a pair are model-authored: {html}"
    );
}

// Why: an empty array and an empty object both fall past the two branches
// above. Rendering an empty <ul> would leave a card section that looks broken
// rather than one that says it has nothing to show.
#[tokio::test]
async fn a_card_with_no_sections_says_it_has_nothing_to_show() {
    let html =
        card_html(systemprompt_models::artifacts::card::PresentationCardArtifact::new("Empty"))
            .await;

    assert!(html.contains("card-empty"), "{html}");
}
