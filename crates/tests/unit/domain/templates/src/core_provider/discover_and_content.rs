//! Tests for content type inference, discover method, and template source.

use systemprompt_template_provider::TemplateProvider;
use systemprompt_templates::CoreTemplateProvider;
use tokio::fs;

mod content_type_inference_tests {
    use super::*;

    #[tokio::test]
    async fn infers_content_type_from_post_suffix() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        fs::write(
            temp_dir.path().join("article-post.html"),
            "<article></article>",
        )
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
            },
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
