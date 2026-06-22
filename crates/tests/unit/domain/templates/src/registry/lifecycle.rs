//! Tests for the registry initialization lifecycle paths that exercise
//! partial-template loading from files and the no-loader error branch.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_template_provider::{
    ComponentContext, DynComponentRenderer, PartialTemplate, ProviderResult, RenderedComponent,
};
use systemprompt_templates::{ComponentRenderer, TemplateDefinition, TemplateRegistry};
use tokio::fs;

use crate::mocks::{MockLoader, MockProvider, loader, provider};

struct FilePartialComponent {
    id: &'static str,
    variable_name: &'static str,
    partial_name: String,
    partial_path: PathBuf,
}

impl FilePartialComponent {
    fn new(
        id: &'static str,
        variable_name: &'static str,
        partial_name: impl Into<String>,
        partial_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id,
            variable_name,
            partial_name: partial_name.into(),
            partial_path: partial_path.into(),
        }
    }
}

#[async_trait]
impl ComponentRenderer for FilePartialComponent {
    fn component_id(&self) -> &'static str {
        self.id
    }

    fn variable_name(&self) -> &'static str {
        self.variable_name
    }

    fn partial_template(&self) -> Option<PartialTemplate> {
        Some(PartialTemplate::file(
            self.partial_name.clone(),
            self.partial_path.clone(),
        ))
    }

    async fn render(&self, _ctx: &ComponentContext<'_>) -> ProviderResult<RenderedComponent> {
        Ok(RenderedComponent::new(
            self.variable_name,
            format!("<comp>{}</comp>", self.id),
        ))
    }
}

struct EmbeddedPartialComponent {
    id: &'static str,
    variable_name: &'static str,
    partial_name: &'static str,
    partial_content: &'static str,
}

#[async_trait]
impl ComponentRenderer for EmbeddedPartialComponent {
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

mod partial_file_loading_tests {
    use super::*;

    #[tokio::test]
    async fn partial_loaded_from_file_is_registered_and_renders() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let partial_path = temp_dir.path().join("nav.hbs");
        fs::write(&partial_path, "<nav>{{title}}</nav>")
            .await
            .expect("failed to write partial file");

        let mut registry = TemplateRegistry::new();
        registry.register_component(Arc::new(FilePartialComponent::new(
            "nav-comp",
            "nav_html",
            "file-nav",
            &partial_path,
        )) as DynComponentRenderer);
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert!(
            registry.has_partial("file-nav"),
            "file-sourced partial should be registered"
        );

        let data = serde_json::json!({"title": "Welcome"});
        let rendered = registry
            .render_partial("file-nav", &data)
            .expect("file-sourced partial should render");
        assert!(rendered.contains("Welcome"));
        assert!(rendered.contains("<nav>"));
    }

    #[tokio::test]
    async fn missing_partial_file_is_skipped_without_failing_init() {
        let mut registry = TemplateRegistry::new();
        registry.register_component(Arc::new(FilePartialComponent::new(
            "broken-comp",
            "broken_html",
            "broken-partial",
            "/nonexistent/dir/missing-partial.hbs",
        )) as DynComponentRenderer);
        registry.register_loader(loader(MockLoader::new()));

        registry
            .initialize()
            .await
            .expect("init should succeed despite missing partial file");

        assert!(
            !registry.has_partial("broken-partial"),
            "partial whose file is missing must not be registered"
        );
    }

    #[tokio::test]
    async fn invalid_embedded_partial_is_skipped_without_failing_init() {
        let mut registry = TemplateRegistry::new();
        registry.register_component(Arc::new(EmbeddedPartialComponent {
            id: "bad-syntax",
            variable_name: "bad_html",
            partial_name: "bad-partial",
            partial_content: "{{#each items}}unterminated",
        }) as DynComponentRenderer);
        registry.register_loader(loader(MockLoader::new()));

        registry
            .initialize()
            .await
            .expect("init should succeed despite uncompilable partial");

        assert!(
            !registry.has_partial("bad-partial"),
            "partial that fails handlebars compilation must not be registered"
        );
    }

    #[tokio::test]
    async fn mixed_valid_and_missing_partials_register_only_valid() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let good_path = temp_dir.path().join("good.hbs");
        fs::write(&good_path, "<p>{{value}}</p>")
            .await
            .expect("failed to write good partial");

        let mut registry = TemplateRegistry::new();
        registry.register_component(Arc::new(FilePartialComponent::new(
            "good-comp",
            "good_html",
            "good-partial",
            &good_path,
        )) as DynComponentRenderer);
        registry.register_component(Arc::new(FilePartialComponent::new(
            "missing-comp",
            "missing_html",
            "missing-partial",
            temp_dir.path().join("does-not-exist.hbs"),
        )) as DynComponentRenderer);
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert!(registry.has_partial("good-partial"));
        assert!(!registry.has_partial("missing-partial"));
    }
}

mod no_loader_tests {
    use super::*;

    #[tokio::test]
    async fn template_with_no_matching_loader_is_skipped() {
        let mut registry = TemplateRegistry::new();

        let working = TemplateDefinition::embedded("ok", "<p>ok</p>").with_priority(50);
        let orphan = TemplateDefinition::file("orphan", "/some/path.html").with_priority(100);

        registry.register_provider(provider(MockProvider::with_templates(
            "p",
            vec![working, orphan],
        )));
        registry.register_loader(loader(MockLoader::selective()));

        registry
            .initialize()
            .await
            .expect("init should succeed even when a template has no loader");

        assert!(registry.has_template("ok"));
        assert!(
            !registry.has_template("orphan"),
            "template with no matching loader must be skipped (NoLoader)"
        );
    }
}
