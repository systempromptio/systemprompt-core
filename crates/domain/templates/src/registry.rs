use std::collections::HashMap;

use anyhow::{anyhow, Result};
use handlebars::Handlebars;
use serde_json::Value;
use systemprompt_template_provider::{
    DynComponentRenderer, DynPageDataProvider, DynTemplateDataExtender, DynTemplateLoader,
    DynTemplateProvider, TemplateDefinition,
};
use tracing::{debug, info, warn};

pub struct TemplateRegistry {
    providers: Vec<DynTemplateProvider>,
    loaders: Vec<DynTemplateLoader>,
    extenders: Vec<DynTemplateDataExtender>,
    components: Vec<DynComponentRenderer>,
    page_providers: Vec<DynPageDataProvider>,
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

    pub async fn initialize(&mut self) -> Result<()> {
        info!(
            providers = self.providers.len(),
            loaders = self.loaders.len(),
            "Initializing template registry"
        );

        if self.loaders.is_empty() {
            return Err(anyhow!(
                "No template loaders registered. Use register_loader() or \
                 TemplateRegistryBuilder::with_loader() to add a loader."
            ));
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

        info!(
            templates = self.resolved_templates.len(),
            "Template registry initialized"
        );

        Ok(())
    }

    async fn load_template(&self, definition: &TemplateDefinition) -> Result<String> {
        for loader in &self.loaders {
            if loader.can_load(&definition.source) {
                return loader.load(&definition.source).await;
            }
        }
        Err(anyhow!(
            "No loader available for template: {}",
            definition.name
        ))
    }

    pub fn render(&self, template_name: &str, data: &Value) -> Result<String> {
        self.handlebars
            .render(template_name, data)
            .map_err(|e| anyhow!("Template render failed for '{}': {}", template_name, e))
    }

    #[must_use]
    pub fn has_template(&self, name: &str) -> bool {
        self.resolved_templates.contains_key(name)
    }

    #[must_use]
    pub fn get_template(&self, name: &str) -> Option<&TemplateDefinition> {
        self.resolved_templates.get(name)
    }

    #[must_use]
    pub fn get_template_for_content_type(&self, content_type: &str) -> Option<&str> {
        self.resolved_templates
            .iter()
            .find(|(_, def)| def.content_types.contains(&content_type.to_string()))
            .map(|(name, _)| name.as_str())
    }

    #[must_use]
    pub fn extenders_for(&self, content_type: &str) -> Vec<&DynTemplateDataExtender> {
        self.extenders
            .iter()
            .filter(|e| {
                let types = e.applies_to();
                types.is_empty() || types.contains(&content_type.to_string())
            })
            .collect()
    }

    #[must_use]
    pub fn components_for(&self, content_type: &str) -> Vec<&DynComponentRenderer> {
        self.components
            .iter()
            .filter(|c| {
                let types = c.applies_to();
                types.is_empty() || types.contains(&content_type.to_string())
            })
            .collect()
    }

    #[must_use]
    pub fn page_providers_for(&self, page_type: &str) -> Vec<&DynPageDataProvider> {
        self.page_providers
            .iter()
            .filter(|p| {
                let pages = p.applies_to_pages();
                pages.is_empty() || pages.contains(&page_type.to_string())
            })
            .collect()
    }

    #[must_use]
    pub fn get_template_provider(&self, name: &str) -> Option<&str> {
        self.template_sources.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn template_names(&self) -> Vec<&str> {
        self.resolved_templates.keys().map(String::as_str).collect()
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
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
pub struct TemplateRegistryBuilder {
    providers: Vec<DynTemplateProvider>,
    loaders: Vec<DynTemplateLoader>,
    extenders: Vec<DynTemplateDataExtender>,
    components: Vec<DynComponentRenderer>,
    page_providers: Vec<DynPageDataProvider>,
}

impl std::fmt::Debug for TemplateRegistryBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemplateRegistryBuilder")
            .field("providers", &self.providers.len())
            .field("loaders", &self.loaders.len())
            .field("extenders", &self.extenders.len())
            .field("components", &self.components.len())
            .field("page_providers", &self.page_providers.len())
            .finish()
    }
}

impl TemplateRegistryBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<DynTemplateProvider>) -> Self {
        self.providers.push(provider.into());
        self
    }

    #[must_use]
    pub fn with_loader(mut self, loader: impl Into<DynTemplateLoader>) -> Self {
        self.loaders.push(loader.into());
        self
    }

    #[must_use]
    pub fn with_extender(mut self, extender: impl Into<DynTemplateDataExtender>) -> Self {
        self.extenders.push(extender.into());
        self
    }

    #[must_use]
    pub fn with_component(mut self, component: impl Into<DynComponentRenderer>) -> Self {
        self.components.push(component.into());
        self
    }

    #[must_use]
    pub fn with_page_provider(mut self, provider: impl Into<DynPageDataProvider>) -> Self {
        self.page_providers.push(provider.into());
        self
    }

    #[must_use]
    pub fn build(self) -> TemplateRegistry {
        let mut registry = TemplateRegistry::new();

        for provider in self.providers {
            registry.register_provider(provider);
        }
        for loader in self.loaders {
            registry.register_loader(loader);
        }
        for extender in self.extenders {
            registry.register_extender(extender);
        }
        for component in self.components {
            registry.register_component(component);
        }
        for page_provider in self.page_providers {
            registry.register_page_provider(page_provider);
        }

        registry
    }

    pub async fn build_and_init(self) -> Result<TemplateRegistry> {
        let mut registry = self.build();
        registry.initialize().await?;
        Ok(registry)
    }
}
