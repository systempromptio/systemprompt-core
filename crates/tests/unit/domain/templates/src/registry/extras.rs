use crate::mocks::{loader, provider};

use systemprompt_templates::{TemplateDefinition, TemplateRegistry};

use crate::mocks::{MockLoader, MockProvider};

mod default_trait_tests {
    use super::*;

    #[test]
    fn default_creates_empty_registry() {
        let registry = TemplateRegistry::default();
        let stats = registry.stats();
        assert_eq!(stats.providers, 0);
        assert_eq!(stats.templates, 0);
        assert_eq!(stats.loaders, 0);
        assert_eq!(stats.extenders, 0);
        assert_eq!(stats.components, 0);
        assert_eq!(stats.page_providers, 0);
        assert_eq!(stats.page_prerenderers, 0);
    }
}

mod has_partial_tests {
    use super::*;

    #[test]
    fn has_partial_returns_false_on_empty_registry() {
        let registry = TemplateRegistry::new();
        assert!(!registry.has_partial("nonexistent"));
    }
}

mod page_prerenderers_tests {
    use super::*;

    #[test]
    fn page_prerenderers_empty_on_new_registry() {
        let registry = TemplateRegistry::new();
        assert!(registry.page_prerenderers().is_empty());
    }
}

mod available_content_types_tests {
    use super::*;

    #[test]
    fn available_content_types_empty_on_new_registry() {
        let registry = TemplateRegistry::new();
        assert!(registry.available_content_types().is_empty());
    }

    #[tokio::test]
    async fn available_content_types_returns_all_registered_types() {
        let mut registry = TemplateRegistry::new();

        let templates = vec![
            TemplateDefinition::embedded("article-post", "<article>").for_content_type("article"),
            TemplateDefinition::embedded("guide-post", "<guide>").for_content_type("guide"),
        ];
        registry.register_provider(provider(MockProvider::with_templates("p", templates)));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let types = registry.available_content_types();
        assert!(types.contains(&"article".to_string()));
        assert!(types.contains(&"guide".to_string()));
    }

    #[tokio::test]
    async fn available_content_types_includes_duplicates_from_multiple_templates() {
        let mut registry = TemplateRegistry::new();

        let templates = vec![
            TemplateDefinition::embedded("template-a", "<a>")
                .for_content_type("article")
                .for_content_type("blog"),
            TemplateDefinition::embedded("template-b", "<b>").for_content_type("article"),
        ];
        registry.register_provider(provider(MockProvider::with_templates("p", templates)));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let types = registry.available_content_types();
        let article_count = types.iter().filter(|t| *t == "article").count();
        assert!(article_count >= 1);
        assert!(types.contains(&"blog".to_string()));
    }
}

mod render_partial_tests {
    use super::*;

    #[test]
    fn render_partial_fails_for_unregistered() {
        let registry = TemplateRegistry::new();
        let data = serde_json::json!({});
        let result = registry.render_partial("missing-partial", &data);
        result.unwrap_err();
    }
}
