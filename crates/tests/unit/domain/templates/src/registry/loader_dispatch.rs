use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_template_provider::{
    DynTemplateLoader, TemplateLoader, TemplateLoaderError, TemplateLoaderResult, TemplateSource,
};
use systemprompt_templates::{TemplateDefinition, TemplateError, TemplateRegistry};

use crate::mocks::{MockLoader, MockProvider, loader, provider};

struct NeverLoadsLoader;

#[async_trait]
impl TemplateLoader for NeverLoadsLoader {
    async fn load(&self, _source: &TemplateSource) -> TemplateLoaderResult<String> {
        Err(TemplateLoaderError::EmbeddedOnly)
    }

    fn can_load(&self, _source: &TemplateSource) -> bool {
        false
    }
}

struct EmbeddedOnlyLoader;

#[async_trait]
impl TemplateLoader for EmbeddedOnlyLoader {
    async fn load(&self, source: &TemplateSource) -> TemplateLoaderResult<String> {
        match source {
            TemplateSource::Embedded(content) => Ok((*content).to_owned()),
            _ => Err(TemplateLoaderError::EmbeddedOnly),
        }
    }

    fn can_load(&self, source: &TemplateSource) -> bool {
        matches!(source, TemplateSource::Embedded(_))
    }
}

fn never_loader() -> DynTemplateLoader {
    Arc::new(NeverLoadsLoader)
}

fn embedded_only_loader() -> DynTemplateLoader {
    Arc::new(EmbeddedOnlyLoader)
}

mod no_loader_error_tests {
    use super::*;

    #[tokio::test]
    async fn initialize_emits_no_loader_error_when_no_loader_can_load() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded("orphan", "<div>Orphan</div>");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));
        registry.register_loader(never_loader());

        registry.initialize().await.expect("should initialize");

        assert!(
            !registry.has_template("orphan"),
            "template with no loader should not be registered"
        );
    }

    #[tokio::test]
    async fn no_loader_path_leaves_other_templates_intact() {
        let mut registry = TemplateRegistry::new();

        let orphan = TemplateDefinition::embedded("orphan", "<div>Orphan</div>");
        let good = TemplateDefinition::embedded("good", "<div>Good</div>");

        registry.register_provider(provider(MockProvider::with_templates(
            "p",
            vec![orphan, good],
        )));

        registry.register_loader(never_loader());
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert!(registry.has_template("good"));
    }

    #[tokio::test]
    async fn embedded_only_loader_skips_file_source_templates() {
        let mut registry = TemplateRegistry::new();

        let file_template =
            TemplateDefinition::file("file-tmpl", "/some/path.html").with_priority(100);
        let embedded_template =
            TemplateDefinition::embedded("embedded-tmpl", "<p>Embedded</p>").with_priority(100);

        registry.register_provider(provider(MockProvider::with_templates(
            "p",
            vec![file_template, embedded_template],
        )));
        registry.register_loader(embedded_only_loader());

        registry.initialize().await.expect("should initialize");

        assert!(
            registry.has_template("embedded-tmpl"),
            "embedded template should be registered"
        );
        assert!(
            !registry.has_template("file-tmpl"),
            "file template should be skipped by embedded-only loader"
        );
    }

    #[tokio::test]
    async fn failing_loader_skips_template_and_continues() {
        let mut registry = TemplateRegistry::new();

        let embedded = TemplateDefinition::embedded("surviving", "<p>Survives</p>");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![embedded])));
        registry.register_loader(loader(MockLoader::failing()));
        registry.register_loader(loader(MockLoader::new()));

        registry.initialize().await.expect("should initialize");

        assert!(
            !registry.has_template("surviving"),
            "first loader fails, second never tried because first can_load returned true"
        );
    }

    #[tokio::test]
    async fn render_unregistered_template_produces_render_error() {
        let registry = TemplateRegistry::new();
        let data = serde_json::json!({});
        let result = registry.render("missing", &data);
        assert!(matches!(
            result.unwrap_err(),
            TemplateError::RenderError { .. }
        ));
    }

    #[tokio::test]
    async fn initialize_with_multiple_providers_and_no_loader_skips_all() {
        let mut registry = TemplateRegistry::new();

        registry.register_provider(provider(MockProvider::with_templates(
            "p1",
            vec![TemplateDefinition::embedded("t1", "<p>1</p>")],
        )));
        registry.register_provider(provider(MockProvider::with_templates(
            "p2",
            vec![TemplateDefinition::embedded("t2", "<p>2</p>")],
        )));
        registry.register_loader(never_loader());

        registry.initialize().await.expect("should initialize");

        assert_eq!(registry.stats().templates, 0);
    }
}

mod loader_priority_tests {
    use super::*;

    #[tokio::test]
    async fn first_registered_loader_that_can_load_is_used() {
        let mut registry = TemplateRegistry::new();

        let template = TemplateDefinition::embedded("tmpl", "<span>{{content}}</span>");
        registry.register_provider(provider(MockProvider::with_templates("p", vec![template])));

        registry.register_loader(embedded_only_loader());

        registry.initialize().await.expect("should initialize");

        assert!(registry.has_template("tmpl"));
    }

    #[tokio::test]
    async fn loader_not_called_when_can_load_returns_false() {
        let mut registry = TemplateRegistry::new();

        let file_template = TemplateDefinition::file("file", "fake.html");
        registry.register_provider(provider(MockProvider::with_templates(
            "p",
            vec![file_template],
        )));

        registry.register_loader(embedded_only_loader());

        registry.initialize().await.expect("should initialize");

        assert!(!registry.has_template("file"));
        assert_eq!(registry.stats().templates, 0);
    }
}
