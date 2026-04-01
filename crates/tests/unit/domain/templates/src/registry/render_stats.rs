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

        result.unwrap_err();
    }

    #[tokio::test]
    async fn render_succeeds_with_registered_template() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded("greeting", "<h1>Hello, {{name}}!</h1>");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        let data = serde_json::json!({"name": "World"});
        let rendered = registry.render("greeting", &data)
            .expect("should render greeting template");
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
        let rendered = registry.render("optional", &data)
            .expect("should render optional template");
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
        let rendered = registry.render("nested", &data)
            .expect("should render nested template");
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
        let rendered = registry.render("list", &data)
            .expect("should render list template");
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
        let rendered_visible = registry.render("cond", &data_visible)
            .expect("should render conditional with show=true");
        assert!(rendered_visible.contains("Visible"));

        let data_hidden = serde_json::json!({"show": false});
        let rendered_hidden = registry.render("cond", &data_hidden)
            .expect("should render conditional with show=false");
        assert!(!rendered_hidden.contains("Visible"));
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
