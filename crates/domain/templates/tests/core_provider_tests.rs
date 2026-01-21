use systemprompt_template_provider::TemplateProvider;
use systemprompt_templates::CoreTemplateProvider;
use tokio::fs;

#[test]
fn test_new_creates_provider_with_default_priority() {
    let provider = CoreTemplateProvider::new("/tmp/templates");
    assert_eq!(provider.priority(), CoreTemplateProvider::DEFAULT_PRIORITY);
    assert!(provider.templates().is_empty());
}

#[test]
fn test_with_priority_creates_provider_with_custom_priority() {
    let provider = CoreTemplateProvider::with_priority("/tmp/templates", 500);
    assert_eq!(provider.priority(), 500);
}

#[test]
fn test_provider_id_returns_core() {
    let provider = CoreTemplateProvider::new("/tmp/templates");
    assert_eq!(provider.provider_id(), "core");
}

#[tokio::test]
async fn test_discover_from_nonexistent_directory() {
    let provider = CoreTemplateProvider::discover_from("/nonexistent/path")
        .await
        .expect("should succeed with empty templates for nonexistent directory");
    assert!(provider.templates().is_empty());
}

#[tokio::test]
async fn test_discover_templates_from_temp_directory() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let template_path = temp_dir.path().join("test-post.html");
    fs::write(&template_path, "<html></html>")
        .await
        .expect("failed to write template");

    let provider = CoreTemplateProvider::discover_from(temp_dir.path())
        .await
        .expect("failed to discover templates");
    let templates = provider.templates();

    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "test-post");
    assert_eq!(templates[0].content_types, vec!["test"]);
}

#[tokio::test]
async fn test_discover_with_manifest() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

    let template_path = temp_dir.path().join("custom.html");
    fs::write(&template_path, "<html></html>")
        .await
        .expect("failed to write template");

    let manifest = r#"
templates:
  custom:
    content_types:
      - page
      - article
"#;
    let manifest_path = temp_dir.path().join("templates.yaml");
    fs::write(&manifest_path, manifest)
        .await
        .expect("failed to write manifest");

    let provider = CoreTemplateProvider::discover_from(temp_dir.path())
        .await
        .expect("failed to discover templates");
    let templates = provider.templates();

    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "custom");
    assert_eq!(templates[0].content_types, vec!["page", "article"]);
}
