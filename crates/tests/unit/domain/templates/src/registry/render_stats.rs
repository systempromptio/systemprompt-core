use crate::mocks::{component, extender, loader, page_provider, provider};

use systemprompt_templates::{RegistryStats, TemplateDefinition, TemplateRegistry};

use crate::mocks::{
    MockComponent, MockExtender, MockLoader, MockPageProvider, MockProvider,
};

mod render_tests {
    use super::*;

    #[test]
    fn render_fails_for_unregistered_template() {
        let registry = TemplateRegistry::new();
        let data = serde_json::json!({});

        let result = registry.render("nonexistent", &data);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn render_succeeds_with_registered_template() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded("greeting", "<h1>Hello, {{name}}!</h1>");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let data = serde_json::json!({"name": "World"});
        let result = registry.render("greeting", &data);

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("Hello, World!"));
    }

    #[tokio::test]
    async fn render_with_missing_variable_uses_empty() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded("optional", "<p>Value: {{value}}</p>");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let data = serde_json::json!({});
        let result = registry.render("optional", &data);

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("Value:"));
    }

    #[tokio::test]
    async fn render_with_nested_data() {
        let mut registry = TemplateRegistry::new();

        let template =
            TemplateDefinition::embedded("nested", "<p>{{user.name}} - {{user.email}}</p>");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let data = serde_json::json!({
            "user": {
                "name": "Alice",
                "email": "alice@example.com"
            }
        });
        let result = registry.render("nested", &data);

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("Alice"));
        assert!(rendered.contains("alice@example.com"));
    }

    #[tokio::test]
    async fn render_with_array_iteration() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded(
            "list",
            "<ul>{{#each items}}<li>{{this}}</li>{{/each}}</ul>",
        );
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let data = serde_json::json!({
            "items": ["one", "two", "three"]
        });
        let result = registry.render("list", &data);

        assert!(result.is_ok());
        let rendered = result.unwrap();
        assert!(rendered.contains("<li>one</li>"));
        assert!(rendered.contains("<li>two</li>"));
        assert!(rendered.contains("<li>three</li>"));
    }

    #[tokio::test]
    async fn render_with_conditional() {
        let mut registry = TemplateRegistry::new();

        let template =
            TemplateDefinition::embedded("cond", "{{#if show}}<p>Visible</p>{{/if}}");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let data_visible = serde_json::json!({"show": true});
        let result = registry.render("cond", &data_visible);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Visible"));

        let data_hidden = serde_json::json!({"show": false});
        let result = registry.render("cond", &data_hidden);
        assert!(result.is_ok());
        assert!(!result.unwrap().contains("Visible"));
    }
}

mod stats_tests {
    use super::*;

    #[test]
    fn stats_reflects_registry_state() {
        let mut registry = TemplateRegistry::new();

        registry.register_provider(provider(MockProvider::new("p1")));
        registry.register_provider(provider(MockProvider::new("p2")));
        registry.register_loader(loader(MockLoader::new()));
        registry.register_extender(extender(MockExtender::new("e1")));
        registry.register_component(component(MockComponent::new("c1", "var")));
        registry.register_page_provider(page_provider(MockPageProvider::new("pp1")));
        registry.register_page_provider(page_provider(MockPageProvider::new("pp2")));
        registry.register_page_provider(page_provider(MockPageProvider::new("pp3")));

        let stats = registry.stats();
        assert_eq!(stats.providers, 2);
        assert_eq!(stats.loaders, 1);
        assert_eq!(stats.extenders, 1);
        assert_eq!(stats.components, 1);
        assert_eq!(stats.page_providers, 3);
    }

    #[test]
    fn registry_stats_debug_impl() {
        let stats = RegistryStats {
            providers: 1,
            templates: 2,
            loaders: 3,
            extenders: 4,
            components: 5,
            page_providers: 6,
            page_prerenderers: 7,
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("RegistryStats"));
        assert!(debug_str.contains("providers"));
        assert!(debug_str.contains("templates"));
    }

    #[test]
    fn registry_stats_clone() {
        let stats = RegistryStats {
            providers: 1,
            templates: 2,
            loaders: 3,
            extenders: 4,
            components: 5,
            page_providers: 6,
            page_prerenderers: 7,
        };

        let cloned = stats;
        assert_eq!(cloned.providers, 1);
        assert_eq!(cloned.templates, 2);
    }

    #[test]
    fn registry_stats_copy() {
        let stats = RegistryStats {
            providers: 10,
            templates: 20,
            loaders: 30,
            extenders: 40,
            components: 50,
            page_providers: 60,
            page_prerenderers: 70,
        };

        let copied: RegistryStats = stats;
        assert_eq!(copied.providers, 10);
        assert_eq!(stats.providers, 10);
    }
}

mod template_provider_lookup_tests {
    use super::*;

    #[test]
    fn find_template_provider_returns_none_for_unregistered() {
        let registry = TemplateRegistry::new();
        assert!(registry.find_template_provider("test").is_none());
    }

    #[tokio::test]
    async fn find_template_provider_returns_provider_id() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded("my-template", "<div></div>");
        registry.register_provider(provider(MockProvider::with_templates(
            "my-provider",
            vec![template],
        )));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let provider = registry.find_template_provider("my-template");
        assert_eq!(provider, Some("my-provider"));
    }
}
