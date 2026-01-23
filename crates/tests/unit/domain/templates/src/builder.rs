use crate::mocks::{component, extender, loader, page_provider, provider};

use systemprompt_templates::{TemplateDefinition, TemplateError, TemplateRegistryBuilder};

use crate::mocks::{MockComponent, MockExtender, MockLoader, MockPageProvider, MockProvider};

mod builder_creation_tests {
    use super::*;

    #[test]
    fn new_creates_empty_builder() {
        let builder = TemplateRegistryBuilder::new();
        let registry = builder.build();

        assert_eq!(registry.stats().providers, 0);
        assert_eq!(registry.stats().loaders, 0);
        assert_eq!(registry.stats().extenders, 0);
        assert_eq!(registry.stats().components, 0);
        assert_eq!(registry.stats().page_providers, 0);
    }

    #[test]
    fn default_creates_empty_builder() {
        let builder = TemplateRegistryBuilder::default();
        let registry = builder.build();

        assert_eq!(registry.stats().providers, 0);
    }

    #[test]
    fn debug_impl_includes_builder_name() {
        let builder = TemplateRegistryBuilder::new();
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("TemplateRegistryBuilder"));
    }
}

mod with_provider_tests {
    use super::*;

    #[test]
    fn with_single_provider() {
        let registry = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::new("test-provider")))
            .build();

        assert_eq!(registry.stats().providers, 1);
    }

    #[test]
    fn with_multiple_providers() {
        let registry = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::new("provider-1")))
            .with_provider(provider(MockProvider::new("provider-2")))
            .with_provider(provider(MockProvider::new("provider-3")))
            .build();

        assert_eq!(registry.stats().providers, 3);
    }

    #[test]
    fn with_provider_returns_self_for_chaining() {
        let _registry = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::new("p1")))
            .with_provider(provider(MockProvider::new("p2")))
            .with_loader(loader(MockLoader::new()))
            .build();
    }
}

mod with_loader_tests {
    use super::*;

    #[test]
    fn with_single_loader() {
        let registry = TemplateRegistryBuilder::new()
            .with_loader(loader(MockLoader::new()))
            .build();

        assert_eq!(registry.stats().loaders, 1);
    }

    #[test]
    fn with_multiple_loaders() {
        let registry = TemplateRegistryBuilder::new()
            .with_loader(loader(MockLoader::new()))
            .with_loader(loader(MockLoader::new()))
            .build();

        assert_eq!(registry.stats().loaders, 2);
    }
}

mod with_extender_tests {
    use super::*;

    #[test]
    fn with_single_extender() {
        let registry = TemplateRegistryBuilder::new()
            .with_extender(extender(MockExtender::new("extender-1")))
            .build();

        assert_eq!(registry.stats().extenders, 1);
    }

    #[test]
    fn with_multiple_extenders() {
        let registry = TemplateRegistryBuilder::new()
            .with_extender(extender(MockExtender::new("extender-1")))
            .with_extender(extender(MockExtender::new("extender-2")))
            .build();

        assert_eq!(registry.stats().extenders, 2);
    }
}

mod with_component_tests {
    use super::*;

    #[test]
    fn with_single_component() {
        let registry = TemplateRegistryBuilder::new()
            .with_component(component(MockComponent::new("sidebar", "sidebar_html")))
            .build();

        assert_eq!(registry.stats().components, 1);
    }

    #[test]
    fn with_multiple_components() {
        let registry = TemplateRegistryBuilder::new()
            .with_component(component(MockComponent::new("header", "header_html")))
            .with_component(component(MockComponent::new("footer", "footer_html")))
            .build();

        assert_eq!(registry.stats().components, 2);
    }
}

mod with_page_provider_tests {
    use super::*;

    #[test]
    fn with_single_page_provider() {
        let registry = TemplateRegistryBuilder::new()
            .with_page_provider(page_provider(MockPageProvider::new("home-data")))
            .build();

        assert_eq!(registry.stats().page_providers, 1);
    }

    #[test]
    fn with_multiple_page_providers() {
        let registry = TemplateRegistryBuilder::new()
            .with_page_provider(page_provider(MockPageProvider::new("home")))
            .with_page_provider(page_provider(MockPageProvider::new("about")))
            .build();

        assert_eq!(registry.stats().page_providers, 2);
    }
}

mod build_tests {
    use super::*;

    #[test]
    fn build_creates_registry_with_all_components() {
        let registry = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::new("p")))
            .with_loader(loader(MockLoader::new()))
            .with_extender(extender(MockExtender::new("e")))
            .with_component(component(MockComponent::new("c", "c_html")))
            .with_page_provider(page_provider(MockPageProvider::new("pp")))
            .build();

        let stats = registry.stats();
        assert_eq!(stats.providers, 1);
        assert_eq!(stats.loaders, 1);
        assert_eq!(stats.extenders, 1);
        assert_eq!(stats.components, 1);
        assert_eq!(stats.page_providers, 1);
    }

    #[test]
    fn build_consumes_builder() {
        let builder = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::new("p")));

        let _registry = builder.build();
    }
}

mod build_and_init_tests {
    use super::*;

    #[tokio::test]
    async fn build_and_init_fails_without_loaders() {
        let result = TemplateRegistryBuilder::new().build_and_init().await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TemplateError::NotInitialized
        ));
    }

    #[tokio::test]
    async fn build_and_init_succeeds_with_loader() {
        let result = TemplateRegistryBuilder::new()
            .with_loader(loader(MockLoader::new()))
            .build_and_init()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn build_and_init_initializes_templates() {
        let templates = vec![
            TemplateDefinition::embedded("template-1", "<h1>One</h1>"),
            TemplateDefinition::embedded("template-2", "<h1>Two</h1>"),
        ];

        let result = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::with_templates("test", templates)))
            .with_loader(loader(MockLoader::new()))
            .build_and_init()
            .await;

        assert!(result.is_ok());
        let registry = result.unwrap();
        assert_eq!(registry.stats().templates, 2);
        assert!(registry.has_template("template-1"));
        assert!(registry.has_template("template-2"));
    }

    #[tokio::test]
    async fn build_and_init_registers_extenders() {
        let registry = TemplateRegistryBuilder::new()
            .with_loader(loader(MockLoader::new()))
            .with_extender(extender(MockExtender::new("ext-1")))
            .with_extender(extender(MockExtender::new("ext-2")))
            .build_and_init()
            .await
            .expect("should initialize");

        assert_eq!(registry.stats().extenders, 2);
    }

    #[tokio::test]
    async fn build_and_init_registers_components() {
        let registry = TemplateRegistryBuilder::new()
            .with_loader(loader(MockLoader::new()))
            .with_component(component(MockComponent::new("c1", "c1_html")))
            .build_and_init()
            .await
            .expect("should initialize");

        assert_eq!(registry.stats().components, 1);
    }

    #[tokio::test]
    async fn build_and_init_registers_page_providers() {
        let registry = TemplateRegistryBuilder::new()
            .with_loader(loader(MockLoader::new()))
            .with_page_provider(page_provider(MockPageProvider::new("pp1")))
            .build_and_init()
            .await
            .expect("should initialize");

        assert_eq!(registry.stats().page_providers, 1);
    }
}

mod chaining_tests {
    use super::*;

    #[test]
    fn all_methods_can_be_chained() {
        let _registry = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::new("p1")))
            .with_provider(provider(MockProvider::new("p2")))
            .with_loader(loader(MockLoader::new()))
            .with_loader(loader(MockLoader::new()))
            .with_extender(extender(MockExtender::new("e1")))
            .with_extender(extender(MockExtender::new("e2")))
            .with_component(component(MockComponent::new("c1", "var1")))
            .with_component(component(MockComponent::new("c2", "var2")))
            .with_page_provider(page_provider(MockPageProvider::new("pp1")))
            .with_page_provider(page_provider(MockPageProvider::new("pp2")))
            .build();
    }

    #[test]
    fn methods_can_be_called_in_any_order() {
        let registry1 = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::new("p")))
            .with_loader(loader(MockLoader::new()))
            .with_extender(extender(MockExtender::new("e")))
            .build();

        let registry2 = TemplateRegistryBuilder::new()
            .with_extender(extender(MockExtender::new("e")))
            .with_loader(loader(MockLoader::new()))
            .with_provider(provider(MockProvider::new("p")))
            .build();

        assert_eq!(registry1.stats().providers, registry2.stats().providers);
        assert_eq!(registry1.stats().loaders, registry2.stats().loaders);
        assert_eq!(registry1.stats().extenders, registry2.stats().extenders);
    }
}

mod builder_debug_tests {
    use super::*;

    #[test]
    fn debug_shows_counts() {
        let builder = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::new("p")))
            .with_loader(loader(MockLoader::new()));

        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("providers"));
        assert!(debug_str.contains("loaders"));
    }

    #[test]
    fn debug_with_all_components() {
        let builder = TemplateRegistryBuilder::new()
            .with_provider(provider(MockProvider::new("p")))
            .with_loader(loader(MockLoader::new()))
            .with_extender(extender(MockExtender::new("e")))
            .with_component(component(MockComponent::new("c", "var")))
            .with_page_provider(page_provider(MockPageProvider::new("pp")));

        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("TemplateRegistryBuilder"));
        assert!(debug_str.contains("providers"));
        assert!(debug_str.contains("loaders"));
        assert!(debug_str.contains("extenders"));
        assert!(debug_str.contains("components"));
        assert!(debug_str.contains("page_providers"));
    }
}
