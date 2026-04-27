//! Tests for manifest handling and edge cases.

use systemprompt_template_provider::TemplateProvider;
use systemprompt_templates::CoreTemplateProvider;
use tokio::fs;

mod manifest_tests {
    use super::*;

    #[tokio::test]
    async fn manifest_overrides_inferred_content_types() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("custom.html"), "<html></html>")
            .await
            .expect("failed to write");

        let manifest = r"
templates:
  custom:
    content_types:
      - page
      - article
";
        fs::write(temp_dir.path().join("templates.yaml"), manifest)
            .await
            .expect("failed to write manifest");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        let templates = provider.templates();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "custom");
        assert_eq!(templates[0].content_types, vec!["page", "article"]);
    }

    #[tokio::test]
    async fn manifest_with_single_content_type() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("single.html"), "<html></html>")
            .await
            .expect("failed to write");

        let manifest = r"
templates:
  single:
    content_types:
      - only-one
";
        fs::write(temp_dir.path().join("templates.yaml"), manifest)
            .await
            .expect("failed to write manifest");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert_eq!(provider.templates()[0].content_types, vec!["only-one"]);
    }

    #[tokio::test]
    async fn manifest_with_empty_content_types() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("empty.html"), "<html></html>")
            .await
            .expect("failed to write");

        let manifest = r"
templates:
  empty:
    content_types: []
";
        fs::write(temp_dir.path().join("templates.yaml"), manifest)
            .await
            .expect("failed to write manifest");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert!(provider.templates()[0].content_types.is_empty());
    }

    #[tokio::test]
    async fn invalid_manifest_uses_defaults() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("test-post.html"), "<html></html>")
            .await
            .expect("failed to write");

        fs::write(
            temp_dir.path().join("templates.yaml"),
            "invalid: yaml: content::::",
        )
        .await
        .expect("failed to write manifest");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert_eq!(provider.templates().len(), 1);
        assert_eq!(provider.templates()[0].content_types, vec!["test"]);
    }

    #[tokio::test]
    async fn manifest_for_unlisted_template_uses_inference() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("listed.html"), "<html></html>")
            .await
            .expect("failed to write");
        fs::write(temp_dir.path().join("unlisted-post.html"), "<html></html>")
            .await
            .expect("failed to write");

        let manifest = r"
templates:
  listed:
    content_types:
      - custom
";
        fs::write(temp_dir.path().join("templates.yaml"), manifest)
            .await
            .expect("failed to write manifest");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        let templates = provider.templates();
        assert_eq!(templates.len(), 2);

        let listed = templates.iter().find(|t| t.name == "listed").unwrap();
        let unlisted = templates
            .iter()
            .find(|t| t.name == "unlisted-post")
            .unwrap();

        assert_eq!(listed.content_types, vec!["custom"]);
        assert_eq!(unlisted.content_types, vec!["unlisted"]);
    }
}

mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn handles_hidden_files() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join(".hidden.html"), "<html></html>")
            .await
            .expect("failed to write");
        fs::write(temp_dir.path().join("visible.html"), "<html></html>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert_eq!(provider.templates().len(), 2);
    }

    #[tokio::test]
    async fn handles_template_with_dots_in_name() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("my.template.v2.html"), "<html></html>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert_eq!(provider.templates()[0].name, "my.template.v2");
    }

    #[tokio::test]
    async fn handles_template_with_special_characters() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(
            temp_dir.path().join("template-with-dashes.html"),
            "<html></html>",
        )
        .await
        .expect("failed to write");
        fs::write(
            temp_dir.path().join("template_with_underscores.html"),
            "<html></html>",
        )
        .await
        .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert_eq!(provider.templates().len(), 2);
    }
}
