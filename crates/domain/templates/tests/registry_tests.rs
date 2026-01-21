use systemprompt_templates::{
    RegistryStats, TemplateError, TemplateRegistry, TemplateRegistryBuilder,
};

#[test]
fn test_registry_new_creates_empty_registry() {
    let registry = TemplateRegistry::new();
    let stats = registry.stats();

    assert_eq!(stats.providers, 0);
    assert_eq!(stats.templates, 0);
    assert_eq!(stats.loaders, 0);
    assert_eq!(stats.extenders, 0);
    assert_eq!(stats.components, 0);
    assert_eq!(stats.page_providers, 0);
}

#[test]
fn test_registry_default_equals_new() {
    let registry1 = TemplateRegistry::new();
    let registry2 = TemplateRegistry::default();

    assert_eq!(registry1.stats().providers, registry2.stats().providers);
    assert_eq!(registry1.stats().templates, registry2.stats().templates);
}

#[test]
fn test_builder_creates_empty_registry() {
    let registry = TemplateRegistryBuilder::new().build();
    assert_eq!(registry.stats().providers, 0);
}

#[test]
fn test_has_template_returns_false_for_unregistered() {
    let registry = TemplateRegistry::new();
    assert!(!registry.has_template("nonexistent"));
}

#[test]
fn test_get_template_returns_none_for_unregistered() {
    let registry = TemplateRegistry::new();
    assert!(registry.get_template("nonexistent").is_none());
}

#[test]
fn test_template_names_empty_for_new_registry() {
    let registry = TemplateRegistry::new();
    assert!(registry.template_names().is_empty());
}

#[tokio::test]
async fn test_initialize_fails_without_loaders() {
    let mut registry = TemplateRegistry::new();
    let result = registry.initialize().await;

    assert!(result.is_err());
    assert!(matches!(
        result.expect_err("should fail without loaders"),
        TemplateError::NotInitialized
    ));
}

#[test]
fn test_render_fails_for_unregistered_template() {
    let registry = TemplateRegistry::new();
    let data = serde_json::json!({});
    let result = registry.render("nonexistent", &data);

    assert!(result.is_err());
}

#[test]
fn test_extenders_for_returns_empty_without_extenders() {
    let registry = TemplateRegistry::new();
    assert!(registry.extenders_for("article").is_empty());
}

#[test]
fn test_components_for_returns_empty_without_components() {
    let registry = TemplateRegistry::new();
    assert!(registry.components_for("article").is_empty());
}

#[test]
fn test_page_providers_for_returns_empty_without_providers() {
    let registry = TemplateRegistry::new();
    assert!(registry.page_providers_for("home").is_empty());
}

#[test]
fn test_get_template_provider_returns_none_for_unregistered() {
    let registry = TemplateRegistry::new();
    assert!(registry.get_template_provider("test").is_none());
}

#[test]
fn test_get_template_for_content_type_returns_none_without_templates() {
    let registry = TemplateRegistry::new();
    assert!(registry.get_template_for_content_type("article").is_none());
}

#[test]
fn test_registry_debug_impl() {
    let registry = TemplateRegistry::new();
    let debug_str = format!("{:?}", registry);
    assert!(debug_str.contains("TemplateRegistry"));
}

#[test]
fn test_builder_debug_impl() {
    let builder = TemplateRegistryBuilder::new();
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("TemplateRegistryBuilder"));
}

#[test]
fn test_registry_stats_debug_impl() {
    let stats = RegistryStats {
        providers: 1,
        templates: 2,
        loaders: 3,
        extenders: 4,
        components: 5,
        page_providers: 6,
    };
    let debug_str = format!("{:?}", stats);
    assert!(debug_str.contains("RegistryStats"));
}
