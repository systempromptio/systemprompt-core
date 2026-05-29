//! Handlebars-backed template registry.
//!
//! [`TemplateRegistry`] aggregates the extension-provided template surface —
//! providers, loaders, data extenders, component renderers, page-data
//! providers, and page prerenderers — resolves templates by priority, and
//! compiles them into a shared Handlebars instance. Registration lives here;
//! initialization and rendering live in the `lifecycle` and `queries`
//! submodules, with aggregate counts exposed via [`RegistryStats`].

mod lifecycle;
mod queries;
mod stats;

use std::collections::HashMap;

use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderError,
    RenderErrorReason,
};
use systemprompt_template_provider::{
    DynComponentRenderer, DynPageDataProvider, DynPagePrerenderer, DynTemplateDataExtender,
    DynTemplateLoader, DynTemplateProvider, TemplateDefinition,
};
use tracing::debug;

pub use stats::RegistryStats;

pub struct TemplateRegistry {
    pub(super) providers: Vec<DynTemplateProvider>,
    pub(super) loaders: Vec<DynTemplateLoader>,
    pub(super) extenders: Vec<DynTemplateDataExtender>,
    pub(super) components: Vec<DynComponentRenderer>,
    pub(super) page_providers: Vec<DynPageDataProvider>,
    pub(super) page_prerenderers: Vec<DynPagePrerenderer>,
    pub(super) resolved_templates: HashMap<String, TemplateDefinition>,
    pub(super) handlebars: Handlebars<'static>,
    pub(super) template_sources: HashMap<String, String>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRegistry {
    #[must_use]
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();
        handlebars.register_helper("json", Box::new(json_helper));
        Self {
            providers: Vec::new(),
            loaders: Vec::new(),
            extenders: Vec::new(),
            components: Vec::new(),
            page_providers: Vec::new(),
            page_prerenderers: Vec::new(),
            resolved_templates: HashMap::new(),
            handlebars,
            template_sources: HashMap::new(),
        }
    }

    pub fn register_provider(&mut self, provider: DynTemplateProvider) {
        debug!(
            provider_id = %provider.provider_id(),
            priority = provider.priority(),
            "Registering template provider"
        );
        self.providers.push(provider);
        self.providers.sort_by_key(|p| p.priority());
    }

    pub fn register_loader(&mut self, loader: DynTemplateLoader) {
        self.loaders.push(loader);
    }

    pub fn register_extender(&mut self, extender: DynTemplateDataExtender) {
        debug!(
            extender_id = %extender.extender_id(),
            priority = extender.priority(),
            "Registering template data extender"
        );
        self.extenders.push(extender);
        self.extenders.sort_by_key(|e| e.priority());
    }

    pub fn register_component(&mut self, component: DynComponentRenderer) {
        debug!(
            component_id = %component.component_id(),
            variable_name = %component.variable_name(),
            priority = component.priority(),
            "Registering component renderer"
        );
        self.components.push(component);
        self.components.sort_by_key(|c| c.priority());
    }

    pub fn register_page_provider(&mut self, provider: DynPageDataProvider) {
        debug!(
            provider_id = %provider.provider_id(),
            pages = ?provider.applies_to_pages(),
            "Registering page data provider"
        );
        self.page_providers.push(provider);
        self.page_providers.sort_by_key(|p| p.priority());
    }

    pub fn register_page_prerenderer(&mut self, prerenderer: DynPagePrerenderer) {
        debug!(
            page_type = %prerenderer.page_type(),
            priority = prerenderer.priority(),
            "Registering page prerenderer"
        );
        self.page_prerenderers.push(prerenderer);
        self.page_prerenderers.sort_by_key(|p| p.priority());
    }
}

fn json_helper(
    h: &Helper<'_>,
    _: &Handlebars<'_>,
    _: &Context,
    _: &mut RenderContext<'_, '_>,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| RenderError::from(RenderErrorReason::ParamNotFoundForIndex("json", 0)))?;
    let serialized = serde_json::to_string(param.value()).map_err(|e| {
        RenderError::from(RenderErrorReason::NestedError(Box::new(
            std::io::Error::other(e.to_string()),
        )))
    })?;
    out.write(&serialized)?;
    Ok(())
}

impl std::fmt::Debug for TemplateRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemplateRegistry")
            .field("providers", &self.providers.len())
            .field(
                "templates",
                &self.resolved_templates.keys().collect::<Vec<_>>(),
            )
            .field("loaders", &self.loaders.len())
            .field("extenders", &self.extenders.len())
            .field("components", &self.components.len())
            .field("page_providers", &self.page_providers.len())
            .field("page_prerenderers", &self.page_prerenderers.len())
            .finish_non_exhaustive()
    }
}
