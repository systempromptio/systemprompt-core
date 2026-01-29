use std::collections::HashMap;
use std::path::Path;

use handlebars::Handlebars;
use serde_json::Value;
use systemprompt_template_provider::{
    DynComponentRenderer, DynPageDataProvider, DynPagePrerenderer, DynTemplateDataExtender,
    DynTemplateLoader, DynTemplateProvider, PartialSource, TemplateDefinition,
};
use tracing::{debug, info, warn};

use crate::TemplateError;

pub struct TemplateRegistry {
    providers: Vec<DynTemplateProvider>,
    loaders: Vec<DynTemplateLoader>,
    extenders: Vec<DynTemplateDataExtender>,
    components: Vec<DynComponentRenderer>,
    page_providers: Vec<DynPageDataProvider>,
    page_prerenderers: Vec<DynPagePrerenderer>,
    resolved_templates: HashMap<String, TemplateDefinition>,
    handlebars: Handlebars<'static>,
    template_sources: HashMap<String, String>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            loaders: Vec::new(),
            extenders: Vec::new(),
            components: Vec::new(),
            page_providers: Vec::new(),
            page_prerenderers: Vec::new(),
            resolved_templates: HashMap::new(),
            handlebars: Handlebars::new(),
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
            "Registering component renderer"
        );
        self.components.push(component);
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

    pub async fn initialize(&mut self) -> Result<(), TemplateError> {
        info!(
            providers = self.providers.len(),
            loaders = self.loaders.len(),
            "Initializing template registry"
        );

        if self.loaders.is_empty() {
            return Err(TemplateError::NotInitialized);
        }

        let mut all_templates: Vec<(TemplateDefinition, &str)> = Vec::new();

        for provider in &self.providers {
            for template in provider.templates() {
                all_templates.push((template, provider.provider_id()));
            }
        }

        all_templates.sort_by(|a, b| a.0.priority.cmp(&b.0.priority));

        for (template, provider_id) in all_templates {
            if self.resolved_templates.contains_key(&template.name) {
                debug!(
                    template = %template.name,
                    provider = %provider_id,
                    "Template already registered, skipping"
                );
                continue;
            }

            debug!(
                template = %template.name,
                provider = %provider_id,
                priority = template.priority,
                "Registering template"
            );

            match self.load_template(&template).await {
                Ok(content) => {
                    if let Err(e) = self
                        .handlebars
                        .register_template_string(&template.name, content)
                    {
                        warn!(
                            template = %template.name,
                            error = %e,
                            "Failed to compile template"
                        );
                        continue;
                    }
                    self.template_sources
                        .insert(template.name.clone(), provider_id.to_string());
                    self.resolved_templates
                        .insert(template.name.clone(), template);
                },
                Err(e) => {
                    warn!(
                        template = %template.name,
                        error = %e,
                        "Failed to load template"
                    );
                },
            }
        }

        self.register_partial_templates().await;

        info!(
            templates = self.resolved_templates.len(),
            "Template registry initialized"
        );

        Ok(())
    }

    async fn register_partial_templates(&mut self) {
        for component in &self.components {
            let Some(partial) = component.partial_template() else {
                continue;
            };

            let content = match &partial.source {
                PartialSource::Embedded(s) => (*s).to_string(),
                PartialSource::File(path) => match self.load_partial_file(path).await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(
                            component_id = %component.component_id(),
                            path = %path.display(),
                            error = %e,
                            "Failed to load partial template file"
                        );
                        continue;
                    },
                },
            };

            debug!(
                component_id = %component.component_id(),
                partial_name = %partial.name,
                "Registering partial template"
            );

            if let Err(e) = self
                .handlebars
                .register_template_string(&partial.name, content)
            {
                warn!(
                    component_id = %component.component_id(),
                    partial_name = %partial.name,
                    error = %e,
                    "Failed to compile partial template"
                );
            }
        }
    }

    async fn load_partial_file(&self, path: &Path) -> Result<String, TemplateError> {
        tokio::fs::read_to_string(path)
            .await
            .map_err(|e| TemplateError::LoadError {
                name: path.display().to_string(),
                source: e.into(),
            })
    }

    async fn load_template(
        &self,
        definition: &TemplateDefinition,
    ) -> Result<String, TemplateError> {
        for loader in &self.loaders {
            if loader.can_load(&definition.source) {
                return loader.load(&definition.source).await.map_err(|e| {
                    TemplateError::LoadError {
                        name: definition.name.clone(),
                        source: e.into(),
                    }
                });
            }
        }
        Err(TemplateError::NoLoader(definition.name.clone()))
    }

    pub fn render(&self, template_name: &str, data: &Value) -> Result<String, TemplateError> {
        self.handlebars
            .render(template_name, data)
            .map_err(|e| TemplateError::RenderError {
                name: template_name.to_string(),
                source: e.into(),
            })
    }

    pub fn render_partial(
        &self,
        partial_name: &str,
        data: &Value,
    ) -> Result<String, TemplateError> {
        self.handlebars
            .render(partial_name, data)
            .map_err(|e| TemplateError::RenderError {
                name: partial_name.to_string(),
                source: e.into(),
            })
    }

    #[must_use]
    pub fn has_partial(&self, partial_name: &str) -> bool {
        self.handlebars.has_template(partial_name)
    }

    #[must_use]
    pub fn has_template(&self, name: &str) -> bool {
        self.resolved_templates.contains_key(name)
    }

    #[must_use]
    pub fn find_template(&self, name: &str) -> Option<&TemplateDefinition> {
        self.resolved_templates.get(name)
    }

    #[must_use]
    pub fn find_template_for_content_type(&self, content_type: &str) -> Option<&str> {
        let content_type_owned = content_type.to_string();
        self.resolved_templates
            .iter()
            .find(|(_, def)| def.content_types.contains(&content_type_owned))
            .map(|(name, _)| name.as_str())
    }

    #[must_use]
    pub fn extenders_for(&self, content_type: &str) -> Vec<&DynTemplateDataExtender> {
        let content_type_owned = content_type.to_string();
        self.extenders
            .iter()
            .filter(|e| {
                let types = e.applies_to();
                types.is_empty() || types.contains(&content_type_owned)
            })
            .collect()
    }

    #[must_use]
    pub fn components_for(&self, content_type: &str) -> Vec<&DynComponentRenderer> {
        let content_type_owned = content_type.to_string();
        self.components
            .iter()
            .filter(|c| {
                let types = c.applies_to();
                types.is_empty() || types.contains(&content_type_owned)
            })
            .collect()
    }

    #[must_use]
    pub fn page_providers_for(&self, page_type: &str) -> Vec<&DynPageDataProvider> {
        let page_type_owned = page_type.to_string();
        self.page_providers
            .iter()
            .filter(|p| {
                let pages = p.applies_to_pages();
                pages.is_empty() || pages.contains(&page_type_owned)
            })
            .collect()
    }

    #[must_use]
    pub fn page_prerenderers(&self) -> &[DynPagePrerenderer] {
        &self.page_prerenderers
    }

    #[must_use]
    pub fn find_template_provider(&self, name: &str) -> Option<&str> {
        self.template_sources.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn template_names(&self) -> Vec<&str> {
        self.resolved_templates.keys().map(String::as_str).collect()
    }

    #[must_use]
    pub fn available_content_types(&self) -> Vec<String> {
        self.resolved_templates
            .values()
            .flat_map(|def| def.content_types.iter().cloned())
            .collect()
    }

    #[must_use]
    pub fn stats(&self) -> RegistryStats {
        RegistryStats {
            providers: self.providers.len(),
            templates: self.resolved_templates.len(),
            loaders: self.loaders.len(),
            extenders: self.extenders.len(),
            components: self.components.len(),
            page_providers: self.page_providers.len(),
            page_prerenderers: self.page_prerenderers.len(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RegistryStats {
    pub providers: usize,
    pub templates: usize,
    pub loaders: usize,
    pub extenders: usize,
    pub components: usize,
    pub page_providers: usize,
    pub page_prerenderers: usize,
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
