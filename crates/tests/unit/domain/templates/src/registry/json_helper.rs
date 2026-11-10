use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_template_provider::{
    ComponentContext, DynComponentRenderer, PartialTemplate, ProviderResult, RenderedComponent,
};
use systemprompt_templates::{ComponentRenderer, TemplateRegistry};

use crate::mocks::{MockLoader, loader};

struct JsonLdComponent;

#[async_trait]
impl ComponentRenderer for JsonLdComponent {
    fn component_id(&self) -> &'static str {
        "json-ld"
    }

    fn variable_name(&self) -> &'static str {
        "json_ld"
    }

    fn partial_template(&self) -> Option<PartialTemplate> {
        Some(PartialTemplate::embedded(
            "json-ld-partial",
            r#"<script type="application/ld+json">{"description": {{{json description}}}}</script>"#,
        ))
    }

    async fn render(&self, _ctx: &ComponentContext<'_>) -> ProviderResult<RenderedComponent> {
        Ok(RenderedComponent::new("json_ld", String::new()))
    }
}

async fn render_description(description: &str) -> String {
    let mut registry = TemplateRegistry::new();
    registry.register_component(Arc::new(JsonLdComponent) as DynComponentRenderer);
    registry.register_loader(loader(MockLoader::new()));
    registry.initialize().await.expect("should initialize");
    registry
        .render_partial(
            "json-ld-partial",
            &serde_json::json!({ "description": description }),
        )
        .expect("render_partial should succeed")
}

fn script_body(rendered: &str) -> &str {
    let open = r#"<script type="application/ld+json">"#;
    let start = rendered.find(open).expect("opening tag") + open.len();
    let end = rendered.rfind("</script>").expect("closing tag");
    &rendered[start..end]
}

#[tokio::test]
async fn json_helper_value_cannot_close_the_script_element() {
    let payload = "</script><script>steal()</script><!-- x";
    let rendered = render_description(payload).await;

    assert_eq!(rendered.matches("</script").count(), 1, "got: {rendered}");
    assert!(!rendered.contains("<script>steal"), "got: {rendered}");
    assert!(!rendered.contains("<!--"), "got: {rendered}");
}

#[tokio::test]
async fn json_helper_escapes_line_separators_and_ampersand() {
    let rendered = render_description("a\u{2028}b\u{2029}c&d").await;
    let body = script_body(&rendered);

    assert!(!body.contains('\u{2028}') && !body.contains('\u{2029}'));
    assert!(!body.contains('&'), "got: {body}");
    assert!(body.contains(r"\u2028") && body.contains(r"\u2029"));
}

#[tokio::test]
async fn json_helper_output_parses_back_to_the_original_value() {
    let payload = "</script><!-- \u{2028} & <b>\"quoted\"</b>";
    let rendered = render_description(payload).await;
    let parsed: serde_json::Value =
        serde_json::from_str(script_body(&rendered)).expect("valid json");

    assert_eq!(parsed["description"], payload);
}
