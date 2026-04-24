use systemprompt_mcp::services::ui_renderer::UiRendererRegistry;
use systemprompt_mcp::services::ui_renderer::registry::create_default_registry;

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
