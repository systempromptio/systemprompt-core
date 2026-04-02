use systemprompt_templates::{EmbeddedDefaultsProvider, TemplateProvider};

#[test]
fn provider_id_returns_embedded_defaults() {
    let provider = EmbeddedDefaultsProvider;
    assert_eq!(provider.provider_id(), "embedded-defaults");
}

#[test]
fn priority_returns_default_constant() {
    let provider = EmbeddedDefaultsProvider;
    assert_eq!(provider.priority(), EmbeddedDefaultsProvider::PRIORITY);
    assert_eq!(provider.priority(), 1000);
}

#[test]
fn templates_returns_homepage() {
    let provider = EmbeddedDefaultsProvider;
    let templates = provider.templates();
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "homepage");
}

#[test]
fn homepage_template_has_correct_content_type() {
    let provider = EmbeddedDefaultsProvider;
    let templates = provider.templates();
    assert!(templates[0].content_types.contains(&"homepage".to_string()));
}

#[test]
fn homepage_template_has_expected_priority() {
    let provider = EmbeddedDefaultsProvider;
    let templates = provider.templates();
    assert_eq!(templates[0].priority, EmbeddedDefaultsProvider::PRIORITY);
}

#[test]
fn default_trait_creates_instance() {
    let provider = EmbeddedDefaultsProvider::default();
    assert_eq!(provider.provider_id(), "embedded-defaults");
}

#[test]
fn debug_impl() {
    let provider = EmbeddedDefaultsProvider;
    let debug = format!("{:?}", provider);
    assert!(debug.contains("EmbeddedDefaultsProvider"));
}

#[test]
fn clone_preserves_identity() {
    let original = EmbeddedDefaultsProvider;
    let cloned = original;
    assert_eq!(original.provider_id(), cloned.provider_id());
    assert_eq!(original.priority(), cloned.priority());
}
