use std::path::PathBuf;

use systemprompt_template_provider::TemplateProvider;
use systemprompt_templates::CoreTemplateProvider;
use tokio::fs;

mod creation_tests {
    use super::*;

    #[test]
    fn new_creates_provider_with_default_priority() {
        let provider = CoreTemplateProvider::new("/tmp/templates");

        assert_eq!(provider.priority(), CoreTemplateProvider::DEFAULT_PRIORITY);
        assert!(provider.templates().is_empty());
    }

    #[test]
    fn new_accepts_path_buf() {
        let path = PathBuf::from("/var/templates");
        let provider = CoreTemplateProvider::new(path);

        assert_eq!(provider.priority(), CoreTemplateProvider::DEFAULT_PRIORITY);
    }

    #[test]
    fn new_accepts_string() {
        let provider = CoreTemplateProvider::new("/var/templates".to_string());

        assert_eq!(provider.priority(), CoreTemplateProvider::DEFAULT_PRIORITY);
    }

    #[test]
    fn with_priority_creates_provider_with_custom_priority() {
        let provider = CoreTemplateProvider::with_priority("/tmp/templates", 500);

        assert_eq!(provider.priority(), 500);
    }

    #[test]
    fn with_priority_accepts_various_values() {
        let priorities = [0, 1, 100, 500, 1000, u32::MAX];

        for priority in priorities {
            let provider = CoreTemplateProvider::with_priority("/tmp", priority);
            assert_eq!(provider.priority(), priority);
        }
    }
}

mod provider_trait_tests {
    use super::*;

    #[test]
    fn provider_id_returns_core() {
        let provider = CoreTemplateProvider::new("/tmp/templates");
        assert_eq!(provider.provider_id(), "core");
    }

    #[test]
    fn templates_returns_empty_before_discovery() {
        let provider = CoreTemplateProvider::new("/tmp/templates");
        assert!(provider.templates().is_empty());
    }

    #[test]
    fn priority_returns_configured_value() {
        let default_provider = CoreTemplateProvider::new("/tmp");
        assert_eq!(
            default_provider.priority(),
            CoreTemplateProvider::DEFAULT_PRIORITY
        );

        let custom_provider =
            CoreTemplateProvider::with_priority("/tmp", CoreTemplateProvider::EXTENSION_PRIORITY);
        assert_eq!(
            custom_provider.priority(),
            CoreTemplateProvider::EXTENSION_PRIORITY
        );
    }
}

mod constants_tests {
    use super::*;

    #[test]
    fn default_priority_is_1000() {
        assert_eq!(CoreTemplateProvider::DEFAULT_PRIORITY, 1000);
    }

    #[test]
    fn extension_priority_is_500() {
        assert_eq!(CoreTemplateProvider::EXTENSION_PRIORITY, 500);
    }

    #[test]
    fn extension_priority_is_higher_than_default() {
        assert!(CoreTemplateProvider::EXTENSION_PRIORITY < CoreTemplateProvider::DEFAULT_PRIORITY);
    }
}

mod discover_tests {
    use super::*;

    #[tokio::test]
    async fn discover_from_nonexistent_directory_returns_empty() {
        let provider = CoreTemplateProvider::discover_from("/nonexistent/path/that/does/not/exist")
            .await
            .expect("should succeed with empty templates");

        assert!(provider.templates().is_empty());
    }

    #[tokio::test]
    async fn discover_from_empty_directory_returns_empty() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert!(provider.templates().is_empty());
    }

    #[tokio::test]
    async fn discover_finds_html_templates() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("page.html"), "<html></html>")
            .await
            .expect("failed to write template");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert_eq!(provider.templates().len(), 1);
        assert_eq!(provider.templates()[0].name, "page");
    }

    #[tokio::test]
    async fn discover_finds_multiple_templates() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("page1.html"), "<html></html>")
            .await
            .expect("failed to write");
        fs::write(temp_dir.path().join("page2.html"), "<html></html>")
            .await
            .expect("failed to write");
        fs::write(temp_dir.path().join("page3.html"), "<html></html>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert_eq!(provider.templates().len(), 3);
    }

    #[tokio::test]
    async fn discover_ignores_non_html_files() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("template.html"), "<html></html>")
            .await
            .expect("failed to write");
        fs::write(temp_dir.path().join("style.css"), "body {}")
            .await
            .expect("failed to write");
        fs::write(temp_dir.path().join("script.js"), "console.log('hi');")
            .await
            .expect("failed to write");
        fs::write(temp_dir.path().join("readme.md"), "# Readme")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert_eq!(provider.templates().len(), 1);
        assert_eq!(provider.templates()[0].name, "template");
    }

    #[tokio::test]
    async fn discover_uses_configured_priority() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("template.html"), "<html></html>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_with_priority(temp_dir.path(), 250)
            .await
            .expect("failed to discover");

        assert_eq!(provider.priority(), 250);
        assert_eq!(provider.templates()[0].priority, 250);
    }
}

mod content_type_inference_tests {
    use super::*;

    #[tokio::test]
    async fn infers_content_type_from_post_suffix() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("article-post.html"), "<article></article>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        let templates = provider.templates();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "article-post");
        assert_eq!(templates[0].content_types, vec!["article"]);
    }

    #[tokio::test]
    async fn infers_content_type_from_list_suffix() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("articles-list.html"), "<ul></ul>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        let templates = provider.templates();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "articles-list");
        assert_eq!(templates[0].content_types, vec!["articles-list"]);
    }

    #[tokio::test]
    async fn no_content_type_for_generic_template() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("generic.html"), "<div></div>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        let templates = provider.templates();
        assert_eq!(templates.len(), 1);
        assert!(templates[0].content_types.is_empty());
    }

    #[tokio::test]
    async fn complex_post_name_extracts_prefix() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("blog-entry-post.html"), "<div></div>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        let templates = provider.templates();
        assert_eq!(templates[0].content_types, vec!["blog-entry"]);
    }
}

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
        let unlisted = templates.iter().find(|t| t.name == "unlisted-post").unwrap();

        assert_eq!(listed.content_types, vec!["custom"]);
        assert_eq!(unlisted.content_types, vec!["unlisted"]);
    }
}

mod discover_method_tests {
    use super::*;

    #[tokio::test]
    async fn discover_populates_templates() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("test.html"), "<html></html>")
            .await
            .expect("failed to write");

        let mut provider = CoreTemplateProvider::new(temp_dir.path());
        assert!(provider.templates().is_empty());

        provider.discover().await.expect("failed to discover");

        assert_eq!(provider.templates().len(), 1);
    }

    #[tokio::test]
    async fn discover_can_be_called_multiple_times() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("first.html"), "<html></html>")
            .await
            .expect("failed to write");

        let mut provider = CoreTemplateProvider::new(temp_dir.path());
        provider.discover().await.expect("failed to discover");
        assert_eq!(provider.templates().len(), 1);

        fs::write(temp_dir.path().join("second.html"), "<html></html>")
            .await
            .expect("failed to write");

        provider.discover().await.expect("failed to discover");
        assert_eq!(provider.templates().len(), 2);
    }
}

mod template_source_tests {
    use super::*;

    #[tokio::test]
    async fn templates_have_file_source() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("test.html"), "<html></html>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        let template = &provider.templates()[0];
        match &template.source {
            systemprompt_templates::TemplateSource::File(path) => {
                assert!(path.to_string_lossy().contains("test.html"));
            }
            _ => panic!("Expected File source"),
        }
    }
}

mod debug_tests {
    use super::*;

    #[test]
    fn debug_impl_includes_provider_info() {
        let provider = CoreTemplateProvider::new("/tmp/templates");
        let debug_str = format!("{:?}", provider);

        assert!(debug_str.contains("CoreTemplateProvider"));
    }

    #[tokio::test]
    async fn debug_shows_discovered_templates() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(temp_dir.path().join("test.html"), "<html></html>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        let debug_str = format!("{:?}", provider);
        assert!(debug_str.contains("CoreTemplateProvider"));
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

        fs::write(temp_dir.path().join("template-with-dashes.html"), "<html></html>")
            .await
            .expect("failed to write");
        fs::write(temp_dir.path().join("template_with_underscores.html"), "<html></html>")
            .await
            .expect("failed to write");

        let provider = CoreTemplateProvider::discover_from(temp_dir.path())
            .await
            .expect("failed to discover");

        assert_eq!(provider.templates().len(), 2);
    }
}
