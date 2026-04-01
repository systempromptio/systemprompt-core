use crate::mocks::{loader, provider};

use systemprompt_templates::{TemplateDefinition, TemplateError, TemplateRegistry};

use crate::mocks::{MockLoader, MockProvider};

mod initialization_tests {
    use super::*;

    #[tokio::test]
    async fn initialize_fails_without_loaders() {
        let mut registry = TemplateRegistry::new();

        let result = registry.initialize().await;

        let err = result.unwrap_err();
        assert!(matches!(err, TemplateError::NotInitialized));
    }

    #[tokio::test]
    async fn initialize_succeeds_with_loader_no_templates() {
        let mut registry = TemplateRegistry::new();
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize with loader and no templates");
        assert_eq!(registry.stats().templates, 0);
    }

    #[tokio::test]
    async fn initialize_loads_templates_from_providers() {
        let mut registry = TemplateRegistry::new();

        let templates = vec![
            TemplateDefinition::embedded("template-1", "<h1>Template 1</h1>"),
            TemplateDefinition::embedded("template-2", "<h1>Template 2</h1>"),
        ];
        registry.register_provider(provider(MockProvider::with_templates("test", templates)));
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize with templates");
        assert_eq!(registry.stats().templates, 2);
        assert!(registry.has_template("template-1"));
        assert!(registry.has_template("template-2"));
    }

    #[tokio::test]
    async fn initialize_respects_template_priority() {
        let mut registry = TemplateRegistry::new();

        let high_priority = TemplateDefinition::embedded("shared", "<high>").with_priority(50);
        let low_priority = TemplateDefinition::embedded("shared", "<low>").with_priority(200);

        registry.register_provider(provider(MockProvider::with_templates(
            "low-provider",
            vec![low_priority],
        )));
        registry.register_provider(provider(MockProvider::with_templates(
            "high-provider",
            vec![high_priority],
        )));
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert_eq!(registry.stats().templates, 1);
        let provider = registry.find_template_provider("shared");
        assert_eq!(provider, Some("high-provider"));
    }

    #[tokio::test]
    async fn initialize_tracks_template_sources() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded("my-template", "<div>content</div>");
        registry.register_provider(provider(MockProvider::with_templates(
            "source-provider",
            vec![template],
        )));
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        let source = registry.find_template_provider("my-template");
        assert_eq!(source, Some("source-provider"));
    }

    #[tokio::test]
    async fn initialize_skips_duplicate_template_names() {
        let mut registry = TemplateRegistry::new();

        let template1 = TemplateDefinition::embedded("duplicate", "<first>").with_priority(100);
        let template2 = TemplateDefinition::embedded("duplicate", "<second>").with_priority(100);

        registry.register_provider(provider(MockProvider::with_templates(
            "first-provider",
            vec![template1],
        )));
        registry.register_provider(provider(MockProvider::with_templates(
            "second-provider",
            vec![template2],
        )));
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert_eq!(registry.stats().templates, 1);
    }

    #[tokio::test]
    async fn initialize_continues_on_loader_failure() {
        let mut registry = TemplateRegistry::new();

        let file_template =
            TemplateDefinition::file("file-based", "/nonexistent/path.html").with_priority(100);
        let embedded_template =
            TemplateDefinition::embedded("embedded", "<works>").with_priority(100);

        registry.register_provider(provider(MockProvider::with_templates(
            "provider",
            vec![file_template, embedded_template],
        )));
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert!(registry.has_template("embedded"));
    }
}
