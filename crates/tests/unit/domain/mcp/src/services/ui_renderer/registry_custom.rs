// The mutation surface of `UiRendererRegistry` — `new`, `register`,
// `register_arc`, `get`, `supported_types`. The existing suite only ever reads
// the pre-populated default registry, so a deployment registering a renderer
// for its own artifact type exercises none of these.

use std::sync::Arc;

use systemprompt_mcp::McpDomainResult;
use systemprompt_mcp::services::ui_renderer::{UiRenderer, UiRendererRegistry, UiResource};
use systemprompt_models::artifacts::types::ArtifactType;
use systemprompt_models::{A2aArtifact as Artifact, ArtifactMetadata, Part, TextPart};

const CUSTOM_TYPE: &str = "acme_invoice";

struct CustomRenderer {
    marker: &'static str,
}

#[async_trait::async_trait]
impl UiRenderer for CustomRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Custom(CUSTOM_TYPE.to_owned())
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        Ok(UiResource::new(format!(
            "<p data-marker=\"{}\">{}</p>",
            self.marker, artifact.metadata.artifact_type
        )))
    }
}

fn artifact(artifact_type: &str) -> Artifact {
    let metadata = ArtifactMetadata::new(
        artifact_type.to_owned(),
        systemprompt_identifiers::ContextId::generate(),
        systemprompt_identifiers::TaskId::generate(),
    );
    Artifact {
        id: systemprompt_identifiers::ArtifactId::generate(),
        title: None,
        description: None,
        parts: vec![Part::Text(TextPart {
            text: "body".to_owned(),
        })],
        extensions: vec![],
        metadata,
    }
}

#[test]
fn a_new_registry_supports_nothing() {
    let registry = UiRendererRegistry::new();

    assert!(registry.supported_types().is_empty());
    assert!(!registry.supports(CUSTOM_TYPE));
    assert!(registry.get(CUSTOM_TYPE).is_none());
}

#[test]
fn a_default_registry_starts_empty_like_a_new_one() {
    assert_eq!(
        UiRendererRegistry::default().supported_types().len(),
        UiRendererRegistry::new().supported_types().len()
    );
}

#[test]
fn registering_a_renderer_makes_its_artifact_type_supported() {
    let mut registry = UiRendererRegistry::new();
    registry.register(CustomRenderer { marker: "owned" });

    assert!(registry.supports(CUSTOM_TYPE));
    assert_eq!(registry.supported_types(), vec![CUSTOM_TYPE]);
    assert!(
        registry.get(CUSTOM_TYPE).is_some(),
        "the registered renderer is retrievable by type"
    );
    assert!(
        registry.get("table").is_none(),
        "registering one type does not pull in the defaults"
    );
}

#[tokio::test]
async fn a_registered_renderer_is_the_one_that_renders_its_type() {
    let mut registry = UiRendererRegistry::new();
    registry.register(CustomRenderer { marker: "owned" });

    let resource = registry
        .render(&artifact(CUSTOM_TYPE))
        .await
        .expect("the registered renderer handles its own type");

    assert!(
        resource.html.contains("data-marker=\"owned\""),
        "the registry dispatched to our renderer: {}",
        resource.html
    );
}

#[tokio::test]
async fn register_arc_shares_one_renderer_instance() {
    let shared: Arc<dyn UiRenderer> = Arc::new(CustomRenderer { marker: "shared" });
    let mut registry = UiRendererRegistry::new();
    registry.register_arc(Arc::clone(&shared));

    let resource = registry
        .render(&artifact(CUSTOM_TYPE))
        .await
        .expect("the shared renderer handles its type");

    assert!(resource.html.contains("data-marker=\"shared\""));
    assert_eq!(
        Arc::strong_count(&shared),
        2,
        "the registry holds the same Arc rather than a copy of the renderer"
    );
}

#[test]
fn re_registering_an_artifact_type_replaces_the_previous_renderer() {
    let mut registry = UiRendererRegistry::new();
    registry.register(CustomRenderer { marker: "first" });
    registry.register(CustomRenderer { marker: "second" });

    assert_eq!(
        registry.supported_types().len(),
        1,
        "the type is registered once, not twice"
    );
}

#[tokio::test]
async fn re_registering_makes_the_later_renderer_win() {
    let mut registry = UiRendererRegistry::new();
    registry.register(CustomRenderer { marker: "first" });
    registry.register(CustomRenderer { marker: "second" });

    let resource = registry
        .render(&artifact(CUSTOM_TYPE))
        .await
        .expect("render");

    assert!(
        resource.html.contains("data-marker=\"second\""),
        "the last registration for a type wins: {}",
        resource.html
    );
}

#[test]
fn the_registry_debug_surface_lists_what_it_can_render() {
    let mut registry = UiRendererRegistry::new();
    registry.register(CustomRenderer { marker: "owned" });

    let debug = format!("{registry:?}");
    assert!(
        debug.contains(CUSTOM_TYPE),
        "an operator inspecting the registry sees its registered types: {debug}"
    );
}
