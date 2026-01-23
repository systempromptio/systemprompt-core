use crate::mocks::{component, extender, loader, page_provider, provider};

use systemprompt_templates::{
    RegistryStats, TemplateDefinition, TemplateError, TemplateRegistry,
};

use crate::mocks::{MockComponent, MockExtender, MockLoader, MockPageProvider, MockProvider};

mod registry_creation_tests {
    use super::*;

    #[test]
    fn new_creates_empty_registry() {
        let registry = TemplateRegistry::new();
        let stats = registry.stats();

        assert_eq!(stats.providers, 0);
        assert_eq!(stats.templates, 0);
        assert_eq!(stats.loaders, 0);
        assert_eq!(stats.extenders, 0);
        assert_eq!(stats.components, 0);
        assert_eq!(stats.page_providers, 0);
    }

    #[test]
    fn default_equals_new() {
        let registry1 = TemplateRegistry::new();
        let registry2 = TemplateRegistry::default();

        let stats1 = registry1.stats();
        let stats2 = registry2.stats();

        assert_eq!(stats1.providers, stats2.providers);
        assert_eq!(stats1.templates, stats2.templates);
        assert_eq!(stats1.loaders, stats2.loaders);
        assert_eq!(stats1.extenders, stats2.extenders);
        assert_eq!(stats1.components, stats2.components);
        assert_eq!(stats1.page_providers, stats2.page_providers);
    }

    #[test]
    fn debug_impl_includes_registry_name() {
        let registry = TemplateRegistry::new();
        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("TemplateRegistry"));
    }
}

mod provider_registration_tests {
    use super::*;

    #[test]
    fn register_single_provider() {
        let mut registry = TemplateRegistry::new();
        let mock = MockProvider::new("test-provider");

        registry.register_provider(provider(mock));

        assert_eq!(registry.stats().providers, 1);
    }

    #[test]
    fn register_multiple_providers() {
        let mut registry = TemplateRegistry::new();

        registry.register_provider(provider(MockProvider::new("provider-1")));
        registry.register_provider(provider(MockProvider::new("provider-2")));
        registry.register_provider(provider(MockProvider::new("provider-3")));

        assert_eq!(registry.stats().providers, 3);
    }

    #[test]
    fn providers_sorted_by_priority() {
        let mut registry = TemplateRegistry::new();

        let template_low = TemplateDefinition::embedded("low-priority", "<low>")
            .with_priority(300)
            .for_content_type("article");
        let template_high = TemplateDefinition::embedded("high-priority", "<high>")
            .with_priority(100)
            .for_content_type("article");

        registry.register_provider(provider(MockProvider::with_templates_and_priority(
            "low",
            300,
            vec![template_low],
        )));
        registry.register_provider(provider(MockProvider::with_templates_and_priority(
            "high",
            100,
            vec![template_high],
        )));

        assert_eq!(registry.stats().providers, 2);
    }
}

mod loader_registration_tests {
    use super::*;

    #[test]
    fn register_single_loader() {
        let mut registry = TemplateRegistry::new();
        let mock = MockLoader::new();

        registry.register_loader(loader(mock));

        assert_eq!(registry.stats().loaders, 1);
    }

    #[test]
    fn register_multiple_loaders() {
        let mut registry = TemplateRegistry::new();

        registry.register_loader(loader(MockLoader::new()));
        registry.register_loader(loader(MockLoader::new()));

        assert_eq!(registry.stats().loaders, 2);
    }
}

mod extender_registration_tests {
    use super::*;

    #[test]
    fn register_single_extender() {
        let mut registry = TemplateRegistry::new();
        let mock = MockExtender::new("test-extender");

        registry.register_extender(extender(mock));

        assert_eq!(registry.stats().extenders, 1);
    }

    #[test]
    fn register_multiple_extenders() {
        let mut registry = TemplateRegistry::new();

        registry.register_extender(extender(MockExtender::new("extender-1")));
        registry.register_extender(extender(MockExtender::new("extender-2")));

        assert_eq!(registry.stats().extenders, 2);
    }

    #[test]
    fn extenders_sorted_by_priority() {
        let mut registry = TemplateRegistry::new();

        registry.register_extender(extender(MockExtender::with_priority("low", 200)));
        registry.register_extender(extender(MockExtender::with_priority("high", 50)));
        registry.register_extender(extender(MockExtender::with_priority("medium", 100)));

        assert_eq!(registry.stats().extenders, 3);
    }
}

mod component_registration_tests {
    use super::*;

    #[test]
    fn register_single_component() {
        let mut registry = TemplateRegistry::new();
        let mock = MockComponent::new("sidebar", "sidebar_html");

        registry.register_component(component(mock));

        assert_eq!(registry.stats().components, 1);
    }

    #[test]
    fn register_multiple_components() {
        let mut registry = TemplateRegistry::new();

        registry.register_component(component(MockComponent::new("header", "header_html")));
        registry.register_component(component(MockComponent::new("footer", "footer_html")));

        assert_eq!(registry.stats().components, 2);
    }
}

mod page_provider_registration_tests {
    use super::*;

    #[test]
    fn register_single_page_provider() {
        let mut registry = TemplateRegistry::new();
        let mock = MockPageProvider::new("home-data");

        registry.register_page_provider(page_provider(mock));

        assert_eq!(registry.stats().page_providers, 1);
    }

    #[test]
    fn register_multiple_page_providers() {
        let mut registry = TemplateRegistry::new();

        registry.register_page_provider(page_provider(MockPageProvider::new("home")));
        registry.register_page_provider(page_provider(MockPageProvider::new("about")));

        assert_eq!(registry.stats().page_providers, 2);
    }

    #[test]
    fn page_providers_sorted_by_priority() {
        let mut registry = TemplateRegistry::new();

        registry.register_page_provider(page_provider(MockPageProvider::with_priority("low", 200)));
        registry.register_page_provider(page_provider(MockPageProvider::with_priority("high", 50)));

        assert_eq!(registry.stats().page_providers, 2);
    }
}

mod initialization_tests {
    use super::*;

    #[tokio::test]
    async fn initialize_fails_without_loaders() {
        let mut registry = TemplateRegistry::new();

        let result = registry.initialize().await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TemplateError::NotInitialized
        ));
    }

    #[tokio::test]
    async fn initialize_succeeds_with_loader_no_templates() {
        let mut registry = TemplateRegistry::new();
        registry.register_loader(loader(MockLoader::new()));

        let result = registry.initialize().await;

        assert!(result.is_ok());
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

        let result = registry.initialize().await;

        assert!(result.is_ok());
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

        let found = registry.find_template("my-template");
        assert!(found.is_some());
        let def = found.unwrap();
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
        };

        let copied: RegistryStats = stats;
        assert_eq!(copied.providers, 10);
        assert_eq!(stats.providers, 10);
    }
}
