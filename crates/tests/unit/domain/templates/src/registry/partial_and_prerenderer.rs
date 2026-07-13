use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_template_provider::{
    ComponentContext, DynComponentRenderer, DynPagePrerenderer, PagePrepareContext,
    PagePrerenderer, PageRenderSpec, PartialTemplate, ProviderResult, RenderedComponent,
};
use systemprompt_templates::{ComponentRenderer, TemplateRegistry, TemplateRegistryBuilder};

use crate::mocks::{MockComponent, MockLoader, component, loader};

struct PartialEmittingComponent {
    id: &'static str,
    variable_name: &'static str,
    partial_name: &'static str,
    partial_content: &'static str,
}

impl PartialEmittingComponent {
    fn new(
        id: &'static str,
        variable_name: &'static str,
        partial_name: &'static str,
        partial_content: &'static str,
    ) -> Self {
        Self {
            id,
            variable_name,
            partial_name,
            partial_content,
        }
    }
}

#[async_trait]
impl ComponentRenderer for PartialEmittingComponent {
    fn component_id(&self) -> &'static str {
        self.id
    }

    fn variable_name(&self) -> &'static str {
        self.variable_name
    }

    fn partial_template(&self) -> Option<PartialTemplate> {
        Some(PartialTemplate::embedded(
            self.partial_name,
            self.partial_content,
        ))
    }

    async fn render(&self, _ctx: &ComponentContext<'_>) -> ProviderResult<RenderedComponent> {
        Ok(RenderedComponent::new(
            self.variable_name,
            format!("<comp>{}</comp>", self.id),
        ))
    }
}

struct StubPrerenderer {
    page_type: &'static str,
    priority: u32,
}

impl StubPrerenderer {
    fn new(page_type: &'static str) -> Self {
        Self {
            page_type,
            priority: 100,
        }
    }

    fn with_priority(page_type: &'static str, priority: u32) -> Self {
        Self {
            page_type,
            priority,
        }
    }
}

#[async_trait]
impl PagePrerenderer for StubPrerenderer {
    fn page_type(&self) -> &str {
        self.page_type
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn prepare(
        &self,
        _ctx: &PagePrepareContext<'_>,
    ) -> ProviderResult<Option<PageRenderSpec>> {
        Ok(None)
    }
}

fn dyn_prerenderer(p: StubPrerenderer) -> DynPagePrerenderer {
    Arc::new(p)
}

mod partial_registration_tests {
    use super::*;

    #[tokio::test]
    async fn has_partial_true_after_component_registration() {
        let mut registry = TemplateRegistry::new();

        registry.register_component(Arc::new(PartialEmittingComponent::new(
            "my-comp",
            "my_var",
            "my-partial",
            "<nav>{{item}}</nav>",
        )) as DynComponentRenderer);
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert!(
            registry.has_partial("my-partial"),
            "partial should be registered via component"
        );
    }

    #[tokio::test]
    async fn render_partial_succeeds_for_registered_partial() {
        let mut registry = TemplateRegistry::new();

        registry.register_component(Arc::new(PartialEmittingComponent::new(
            "nav-comp",
            "nav_html",
            "nav-partial",
            "<nav>{{title}}</nav>",
        )) as DynComponentRenderer);
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        let data = serde_json::json!({"title": "Home"});
        let rendered = registry
            .render_partial("nav-partial", &data)
            .expect("render_partial should succeed");

        assert!(rendered.contains("Home"));
        assert!(rendered.contains("<nav>"));
    }

    #[tokio::test]
    async fn has_partial_false_for_unregistered_name() {
        let mut registry = TemplateRegistry::new();
        registry.register_loader(loader(MockLoader::new()));
        registry.initialize().await.expect("should initialize");

        assert!(!registry.has_partial("nonexistent-partial"));
    }

    #[tokio::test]
    async fn multiple_partials_from_multiple_components() {
        let mut registry = TemplateRegistry::new();

        registry.register_component(Arc::new(PartialEmittingComponent::new(
            "c1",
            "v1",
            "partial-one",
            "<p>One</p>",
        )) as DynComponentRenderer);
        registry.register_component(Arc::new(PartialEmittingComponent::new(
            "c2",
            "v2",
            "partial-two",
            "<p>Two</p>",
        )) as DynComponentRenderer);
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert!(registry.has_partial("partial-one"));
        assert!(registry.has_partial("partial-two"));
        assert!(!registry.has_partial("partial-three"));
    }

    #[tokio::test]
    async fn component_without_partial_template_registers_no_partial() {
        let mut registry = TemplateRegistry::new();

        registry.register_component(component(MockComponent::new("plain", "plain_var")));
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert!(!registry.has_partial("plain"));
        assert!(!registry.has_partial("plain_var"));
    }

    #[tokio::test]
    async fn partial_renders_handlebars_template_with_data() {
        let mut registry = TemplateRegistry::new();

        registry.register_component(Arc::new(PartialEmittingComponent::new(
            "footer",
            "footer_html",
            "footer-partial",
            "<footer>{{#each links}}<a href=\"{{url}}\">{{label}}</a>{{/each}}</footer>",
        )) as DynComponentRenderer);
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        let data = serde_json::json!({
            "links": [
                {"url": "/about", "label": "About"},
                {"url": "/contact", "label": "Contact"}
            ]
        });
        let rendered = registry
            .render_partial("footer-partial", &data)
            .expect("should render partial with iteration");

        assert!(rendered.contains("About"));
        assert!(rendered.contains("Contact"));
        assert!(rendered.contains("/about"));
    }
}

mod page_prerenderer_tests {
    use super::*;

    #[test]
    fn page_prerenderers_empty_on_new_registry() {
        let registry = TemplateRegistry::new();
        assert!(registry.page_prerenderers().is_empty());
    }

    #[test]
    fn register_single_page_prerenderer_in_stats() {
        let mut registry = TemplateRegistry::new();
        registry.register_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("home")));

        assert_eq!(registry.stats().page_prerenderers, 1);
    }

    #[test]
    fn register_multiple_page_prerenderers_in_stats() {
        let mut registry = TemplateRegistry::new();
        registry.register_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("home")));
        registry.register_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("about")));
        registry.register_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("blog")));

        assert_eq!(registry.stats().page_prerenderers, 3);
        assert_eq!(registry.page_prerenderers().len(), 3);
    }

    #[test]
    fn page_prerenderers_sorted_by_priority() {
        let mut registry = TemplateRegistry::new();
        registry
            .register_page_prerenderer(dyn_prerenderer(StubPrerenderer::with_priority("low", 300)));
        registry
            .register_page_prerenderer(dyn_prerenderer(StubPrerenderer::with_priority("high", 50)));
        registry
            .register_page_prerenderer(dyn_prerenderer(StubPrerenderer::with_priority("mid", 150)));

        let prerenderers = registry.page_prerenderers();
        assert_eq!(prerenderers.len(), 3);
        assert_eq!(prerenderers[0].page_type(), "high");
        assert_eq!(prerenderers[1].page_type(), "mid");
        assert_eq!(prerenderers[2].page_type(), "low");
    }

    #[test]
    fn register_page_prerenderer_debug_logging_preserves_registration() {
        crate::mocks::init_debug_logging();

        let mut registry = TemplateRegistry::new();
        registry.register_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("logged-page")));

        assert_eq!(registry.stats().page_prerenderers, 1);
        assert_eq!(registry.page_prerenderers()[0].page_type(), "logged-page");
    }

    #[test]
    fn page_prerenderer_page_type_accessible_from_slice() {
        let mut registry = TemplateRegistry::new();
        registry.register_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("article")));

        let slices = registry.page_prerenderers();
        assert_eq!(slices[0].page_type(), "article");
        assert_eq!(slices[0].priority(), 100);
    }
}

mod builder_page_prerenderer_tests {
    use super::*;

    #[test]
    fn with_page_prerenderer_registers_in_registry() {
        let registry = TemplateRegistryBuilder::new()
            .with_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("home")))
            .build();

        assert_eq!(registry.stats().page_prerenderers, 1);
        assert_eq!(registry.page_prerenderers().len(), 1);
    }

    #[test]
    fn with_multiple_page_prerenderers_via_builder() {
        let registry = TemplateRegistryBuilder::new()
            .with_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("home")))
            .with_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("about")))
            .with_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("blog")))
            .build();

        assert_eq!(registry.stats().page_prerenderers, 3);
    }

    #[tokio::test]
    async fn build_and_init_preserves_page_prerenderers() {
        let registry = TemplateRegistryBuilder::new()
            .with_loader(loader(MockLoader::new()))
            .with_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("home")))
            .with_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("docs")))
            .build_and_init()
            .await
            .expect("should initialize");

        assert_eq!(registry.stats().page_prerenderers, 2);
        assert_eq!(registry.page_prerenderers().len(), 2);
    }

    #[test]
    fn builder_with_page_prerenderer_then_build_shows_in_debug() {
        let registry = TemplateRegistryBuilder::new()
            .with_page_prerenderer(dyn_prerenderer(StubPrerenderer::new("home")))
            .build();

        let debug = format!("{:?}", registry);
        assert!(debug.contains("page_prerenderers"));
    }
}
