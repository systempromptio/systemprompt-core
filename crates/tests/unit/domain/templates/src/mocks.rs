use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use systemprompt_template_provider::{
    ComponentContext, ComponentRenderer, DynComponentRenderer, DynPageDataProvider,
    DynTemplateDataExtender, DynTemplateLoader, DynTemplateProvider, ExtenderContext, PageContext,
    PageDataProvider, RenderedComponent, TemplateDataExtender, TemplateDefinition, TemplateLoader,
    TemplateProvider, TemplateSource,
};

// A DEBUG-level subscriber installed *globally* (not thread-locally): the
// registry/lifecycle/discovery `debug!`/`warn!` field expressions are only
// evaluated when a subscriber enables the callsite, and several of those
// callsites fire from tokio worker threads where a thread-local `set_default`
// subscriber would not apply. A global default also survives tracing's
// per-callsite interest cache across every test in the process, so the field
// arguments are exercised on every registration rather than the handful of
// tests that opt in.
static DEBUG_LOGGING: LazyLock<()> = LazyLock::new(|| {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
});

pub fn init_debug_logging() {
    LazyLock::force(&DEBUG_LOGGING);
}

pub fn provider(p: MockProvider) -> DynTemplateProvider {
    init_debug_logging();
    Arc::new(p)
}

pub fn loader(l: MockLoader) -> DynTemplateLoader {
    init_debug_logging();
    Arc::new(l)
}

pub fn extender(e: MockExtender) -> DynTemplateDataExtender {
    init_debug_logging();
    Arc::new(e)
}

pub fn component(c: MockComponent) -> DynComponentRenderer {
    init_debug_logging();
    Arc::new(c)
}

pub fn page_provider(p: MockPageProvider) -> DynPageDataProvider {
    init_debug_logging();
    Arc::new(p)
}

pub struct MockProvider {
    id: &'static str,
    priority: u32,
    templates: Vec<TemplateDefinition>,
}

impl MockProvider {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            priority: 100,
            templates: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn with_priority(id: &'static str, priority: u32) -> Self {
        Self {
            id,
            priority,
            templates: Vec::new(),
        }
    }

    pub fn with_templates(id: &'static str, templates: Vec<TemplateDefinition>) -> Self {
        Self {
            id,
            priority: 100,
            templates,
        }
    }

    pub fn with_templates_and_priority(
        id: &'static str,
        priority: u32,
        templates: Vec<TemplateDefinition>,
    ) -> Self {
        Self {
            id,
            priority,
            templates,
        }
    }
}

impl TemplateProvider for MockProvider {
    fn provider_id(&self) -> &'static str {
        self.id
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    fn templates(&self) -> Vec<TemplateDefinition> {
        self.templates.clone()
    }
}

pub struct MockLoader {
    load_count: AtomicUsize,
    fail_on_load: bool,
    embedded_only: bool,
}

impl Default for MockLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLoader {
    pub fn new() -> Self {
        Self {
            load_count: AtomicUsize::new(0),
            fail_on_load: false,
            embedded_only: false,
        }
    }

    #[allow(dead_code)]
    pub fn failing() -> Self {
        Self {
            load_count: AtomicUsize::new(0),
            fail_on_load: true,
            embedded_only: false,
        }
    }

    #[allow(dead_code)]
    pub fn selective() -> Self {
        Self {
            load_count: AtomicUsize::new(0),
            fail_on_load: false,
            embedded_only: true,
        }
    }

    #[allow(dead_code)]
    pub fn load_count(&self) -> usize {
        self.load_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl TemplateLoader for MockLoader {
    async fn load(
        &self,
        source: &TemplateSource,
    ) -> systemprompt_template_provider::TemplateLoaderResult<String> {
        self.load_count.fetch_add(1, Ordering::SeqCst);

        if self.fail_on_load {
            return Err(
                systemprompt_template_provider::TemplateLoaderError::NotFound(
                    std::path::PathBuf::from("mock-failure"),
                ),
            );
        }

        match source {
            TemplateSource::Embedded(content) => Ok((*content).to_string()),
            TemplateSource::File(path) => Ok(format!("<!-- content from {} -->", path.display())),
            TemplateSource::Directory(path) => Ok(format!("<!-- directory {} -->", path.display())),
        }
    }

    fn can_load(&self, source: &TemplateSource) -> bool {
        if self.embedded_only {
            return matches!(source, TemplateSource::Embedded(_));
        }
        true
    }
}

pub struct MockExtender {
    id: String,
    applies_to: Vec<String>,
    priority: u32,
}

impl MockExtender {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            applies_to: Vec::new(),
            priority: 100,
        }
    }

    pub fn with_applies_to(id: &str, applies_to: Vec<String>) -> Self {
        Self {
            id: id.to_string(),
            applies_to,
            priority: 100,
        }
    }

    pub fn with_priority(id: &str, priority: u32) -> Self {
        Self {
            id: id.to_string(),
            applies_to: Vec::new(),
            priority,
        }
    }
}

#[async_trait]
impl TemplateDataExtender for MockExtender {
    fn extender_id(&self) -> &str {
        &self.id
    }

    fn applies_to(&self) -> Vec<String> {
        self.applies_to.clone()
    }

    async fn extend(
        &self,
        _ctx: &ExtenderContext<'_>,
        data: &mut serde_json::Value,
    ) -> systemprompt_template_provider::ProviderResult<()> {
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                format!("extended_by_{}", self.id),
                serde_json::Value::Bool(true),
            );
        }
        Ok(())
    }

    fn priority(&self) -> u32 {
        self.priority
    }
}

pub struct MockComponent {
    id: &'static str,
    variable_name: &'static str,
    applies_to: Vec<String>,
}

impl MockComponent {
    pub fn new(id: &'static str, variable_name: &'static str) -> Self {
        Self {
            id,
            variable_name,
            applies_to: Vec::new(),
        }
    }

    pub fn with_applies_to(
        id: &'static str,
        variable_name: &'static str,
        applies_to: Vec<String>,
    ) -> Self {
        Self {
            id,
            variable_name,
            applies_to,
        }
    }
}

#[async_trait]
impl ComponentRenderer for MockComponent {
    fn component_id(&self) -> &'static str {
        self.id
    }

    fn variable_name(&self) -> &'static str {
        self.variable_name
    }

    fn applies_to(&self) -> Vec<String> {
        self.applies_to.clone()
    }

    async fn render(
        &self,
        _ctx: &ComponentContext<'_>,
    ) -> systemprompt_template_provider::ProviderResult<RenderedComponent> {
        Ok(RenderedComponent::new(
            self.variable_name,
            format!("<div>Mock component {}</div>", self.id),
        ))
    }
}

pub struct MockPageProvider {
    id: &'static str,
    applies_to_pages: Vec<String>,
    priority: u32,
}

impl MockPageProvider {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            applies_to_pages: Vec::new(),
            priority: 100,
        }
    }

    pub fn with_applies_to(id: &'static str, applies_to_pages: Vec<String>) -> Self {
        Self {
            id,
            applies_to_pages,
            priority: 100,
        }
    }

    pub fn with_priority(id: &'static str, priority: u32) -> Self {
        Self {
            id,
            applies_to_pages: Vec::new(),
            priority,
        }
    }
}

#[async_trait]
impl PageDataProvider for MockPageProvider {
    fn provider_id(&self) -> &'static str {
        self.id
    }

    fn applies_to_pages(&self) -> Vec<String> {
        self.applies_to_pages.clone()
    }

    async fn provide_page_data(
        &self,
        _ctx: &PageContext<'_>,
    ) -> systemprompt_template_provider::ProviderResult<serde_json::Value> {
        Ok(serde_json::json!({
            "provider_id": self.id,
        }))
    }

    fn priority(&self) -> u32 {
        self.priority
    }
}
