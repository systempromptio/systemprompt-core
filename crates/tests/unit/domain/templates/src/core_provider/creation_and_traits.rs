//! Tests for creation, provider trait, constants, and discover methods.

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
