use crate::mocks::{component, extender, loader, page_provider, provider};

use systemprompt_templates::{TemplateDefinition, TemplateRegistry};

use crate::mocks::{
    MockComponent, MockExtender, MockLoader, MockPageProvider, MockProvider,
};

mod template_lookup_tests {
    use super::*;

    #[test]
    fn has_template_returns_false_for_unregistered() {
        let registry = TemplateRegistry::new();
        assert!(!registry.has_template("nonexistent"));
    }

    #[tokio::test]
    async fn has_template_returns_true_for_registered() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded("existing", "<div></div>");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        assert!(registry.has_template("existing"));
    }

    #[test]
    fn find_template_returns_none_for_unregistered() {
        let registry = TemplateRegistry::new();
        assert!(registry.find_template("nonexistent").is_none());
    }

    #[tokio::test]
    async fn find_template_returns_definition_for_registered() {
        let mut registry = TemplateRegistry::new();

        let template =
            TemplateDefinition::embedded("my-template", "<div></div>").for_content_type("article");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let def = registry.find_template("my-template")
            .expect("should find registered template");
        assert_eq!(def.name, "my-template");
        assert!(def.content_types.contains(&"article".to_string()));
    }

    #[test]
    fn template_names_empty_for_new_registry() {
        let registry = TemplateRegistry::new();
        assert!(registry.template_names().is_empty());
    }

    #[tokio::test]
    async fn template_names_returns_all_registered() {
        let mut registry = TemplateRegistry::new();

        let templates = vec![
            TemplateDefinition::embedded("alpha", "<a>"),
            TemplateDefinition::embedded("beta", "<b>"),
            TemplateDefinition::embedded("gamma", "<g>"),
        ];
        registry.register_provider(provider(MockProvider::with_templates("p", templates)));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let names = registry.template_names();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
        assert!(names.contains(&"gamma"));
    }
}

mod content_type_lookup_tests {
    use super::*;

    #[test]
    fn find_template_for_content_type_returns_none_without_templates() {
        let registry = TemplateRegistry::new();
        assert!(registry.find_template_for_content_type("article").is_none());
    }

    #[tokio::test]
    async fn find_template_for_content_type_finds_matching() {
        let mut registry = TemplateRegistry::new();

        let template =
            TemplateDefinition::embedded("article-post", "<article>").for_content_type("article");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let found = registry.find_template_for_content_type("article");
        assert_eq!(found, Some("article-post"));
    }

    #[tokio::test]
    async fn find_template_for_content_type_returns_none_for_non_matching() {
        let mut registry = TemplateRegistry::new();

        let template =
            TemplateDefinition::embedded("article-post", "<article>").for_content_type("article");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let found = registry.find_template_for_content_type("blog");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn find_template_for_content_type_with_multiple_content_types() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded("multi-post", "<multi>")
            .for_content_type("article")
            .for_content_type("blog");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        assert_eq!(
            registry.find_template_for_content_type("article"),
            Some("multi-post")
        );
        assert_eq!(
            registry.find_template_for_content_type("blog"),
            Some("multi-post")
        );
    }
}

mod extender_lookup_tests {
    use super::*;

    #[test]
    fn extenders_for_returns_empty_without_extenders() {
        let registry = TemplateRegistry::new();
        assert!(registry.extenders_for("article").is_empty());
    }

    #[test]
    fn extenders_for_returns_matching_extenders() {
        let mut registry = TemplateRegistry::new();

        registry.register_extender(extender(MockExtender::with_applies_to(
            "article-ext",
            vec!["article".to_string()],
        )));
        registry.register_extender(extender(MockExtender::with_applies_to(
            "blog-ext",
            vec!["blog".to_string()],
        )));

        let article_extenders = registry.extenders_for("article");
        assert_eq!(article_extenders.len(), 1);
        assert_eq!(article_extenders[0].extender_id(), "article-ext");
    }

    #[test]
    fn extenders_for_includes_global_extenders() {
        let mut registry = TemplateRegistry::new();

        registry.register_extender(extender(MockExtender::new("global-ext")));
        registry.register_extender(extender(MockExtender::with_applies_to(
            "article-ext",
            vec!["article".to_string()],
        )));

        let article_extenders = registry.extenders_for("article");
        assert_eq!(article_extenders.len(), 2);
    }

    #[test]
    fn extenders_for_excludes_non_matching() {
        let mut registry = TemplateRegistry::new();

        registry.register_extender(extender(MockExtender::with_applies_to(
            "blog-only",
            vec!["blog".to_string()],
        )));

        let article_extenders = registry.extenders_for("article");
        assert!(article_extenders.is_empty());
    }
}

mod component_lookup_tests {
    use super::*;

    #[test]
    fn components_for_returns_empty_without_components() {
        let registry = TemplateRegistry::new();
        assert!(registry.components_for("article").is_empty());
    }

    #[test]
    fn components_for_returns_matching_components() {
        let mut registry = TemplateRegistry::new();

        registry.register_component(component(MockComponent::with_applies_to(
            "article-sidebar",
            "sidebar",
            vec!["article".to_string()],
        )));
        registry.register_component(component(MockComponent::with_applies_to(
            "blog-widget",
            "widget",
            vec!["blog".to_string()],
        )));

        let article_components = registry.components_for("article");
        assert_eq!(article_components.len(), 1);
        assert_eq!(article_components[0].component_id(), "article-sidebar");
    }

    #[test]
    fn components_for_includes_global_components() {
        let mut registry = TemplateRegistry::new();

        registry.register_component(component(MockComponent::new("global-nav", "nav")));
        registry.register_component(component(MockComponent::with_applies_to(
            "article-comp",
            "comp",
            vec!["article".to_string()],
        )));

        let article_components = registry.components_for("article");
        assert_eq!(article_components.len(), 2);
    }
}

mod page_provider_lookup_tests {
    use super::*;

    #[test]
    fn page_providers_for_returns_empty_without_providers() {
        let registry = TemplateRegistry::new();
        assert!(registry.page_providers_for("home").is_empty());
    }

    #[test]
    fn page_providers_for_returns_matching_providers() {
        let mut registry = TemplateRegistry::new();

        registry.register_page_provider(page_provider(MockPageProvider::with_applies_to(
            "home-data",
            vec!["home".to_string()],
        )));
        registry.register_page_provider(page_provider(MockPageProvider::with_applies_to(
            "about-data",
            vec!["about".to_string()],
        )));

        let home_providers = registry.page_providers_for("home");
        assert_eq!(home_providers.len(), 1);
        assert_eq!(home_providers[0].provider_id(), "home-data");
    }

    #[test]
    fn page_providers_for_includes_global_providers() {
        let mut registry = TemplateRegistry::new();

        registry.register_page_provider(page_provider(MockPageProvider::new("global-data")));
        registry.register_page_provider(page_provider(MockPageProvider::with_applies_to(
            "home-data",
            vec!["home".to_string()],
        )));

        let home_providers = registry.page_providers_for("home");
        assert_eq!(home_providers.len(), 2);
    }
}

