use crate::mocks::{component, extender, loader, page_provider, provider};

use systemprompt_templates::TemplateDefinition;

use crate::mocks::{
    MockComponent, MockExtender, MockLoader, MockPageProvider, MockProvider,
};

mod registry_creation_tests {
    use systemprompt_templates::TemplateRegistry;

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
    fn debug_impl_includes_registry_name() {
        let registry = TemplateRegistry::new();
        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("TemplateRegistry"));
    }
}

mod provider_registration_tests {
    use super::*;
    use systemprompt_templates::TemplateRegistry;

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
    use systemprompt_templates::TemplateRegistry;

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
    use systemprompt_templates::TemplateRegistry;

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
    use systemprompt_templates::TemplateRegistry;

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
    use systemprompt_templates::TemplateRegistry;

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
