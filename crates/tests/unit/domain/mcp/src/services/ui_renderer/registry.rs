use systemprompt_mcp::services::ui_renderer::UiRendererRegistry;
use systemprompt_mcp::services::ui_renderer::registry::{
    create_default_registry, resolve_artifact_type,
};
use systemprompt_models::{A2aArtifact as Artifact, ArtifactMetadata, DataPart, Part};

fn make_artifact(artifact_type: &str, parts: Vec<Part>) -> Artifact {
    let metadata = ArtifactMetadata::new(
        artifact_type.to_string(),
        systemprompt_identifiers::ContextId::generate(),
        systemprompt_identifiers::TaskId::generate(),
    );
    Artifact {
        id: systemprompt_identifiers::ArtifactId::generate(),
        title: None,
        description: None,
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

#[test]
fn registry_registers_default_artifact_types() {
    let registry = create_default_registry();
    assert!(registry.supports("table"));
    assert!(registry.supports("chart"));
    assert!(registry.supports("text"));
}

#[test]
fn empty_registry_rejects_unknown_type() {
    let registry = UiRendererRegistry::new();
    assert!(!registry.supports("unknown_type"));
    assert!(registry.get("unknown_type").is_none());
}

#[test]
fn resolve_artifact_type_passes_through_concrete_type() {
    let artifact = make_artifact("table", vec![]);
    assert_eq!(resolve_artifact_type(&artifact), "table");
}

#[test]
fn resolve_artifact_type_falls_through_envelope_to_embedded_variant_tag() {
    let artifact = make_artifact(
        "cli",
        vec![data_part(
            serde_json::json!({"artifact_type": "table", "data": []}),
        )],
    );
    assert_eq!(resolve_artifact_type(&artifact), "table");
}

#[test]
fn resolve_artifact_type_falls_through_envelope_to_x_artifact_type() {
    let artifact = make_artifact(
        "cli",
        vec![data_part(
            serde_json::json!({"x-artifact-type": "list", "items": []}),
        )],
    );
    assert_eq!(resolve_artifact_type(&artifact), "list");
}

#[test]
fn resolve_artifact_type_envelope_without_data_tag_stays_envelope() {
    let artifact = make_artifact("cli", vec![]);
    assert_eq!(resolve_artifact_type(&artifact), "cli");
}

#[tokio::test]
async fn registry_renders_enveloped_table_artifact() {
    let registry = create_default_registry();
    let artifact = make_artifact(
        "cli",
        vec![data_part(serde_json::json!({
            "artifact_type": "table",
            "columns": ["name"],
            "data": [{"name": "Alice"}]
        }))],
    );
    let result = registry.render(&artifact).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn registry_render_unresolved_envelope_errors() {
    let registry = create_default_registry();
    let artifact = make_artifact("cli", vec![]);
    let result = registry.render(&artifact).await;
    assert!(result.is_err());
}
