//! Template registry and builder.
//!
//! This module provides [`TemplateRegistry`] for managing templates, loaders, extenders,
//! and component renderers. Use [`TemplateRegistryBuilder`] for convenient construction.

use std::collections::HashMap;

use handlebars::Handlebars;
use serde_json::Value;
use systemprompt_template_provider::{
    DynComponentRenderer, DynPageDataProvider, DynTemplateDataExtender, DynTemplateLoader,
    DynTemplateProvider, TemplateDefinition,
};
use tracing::{debug, info, warn};

use crate::TemplateError;

/// Central registry for managing templates, loaders, extenders, and components.
///
/// The registry coordinates between multiple template providers, resolving conflicts
/// by priority (lower priority values win). Use [`TemplateRegistryBuilder`] for
/// convenient construction.
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
    /// Creates a new empty template registry.
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

    /// Registers a template provider.
    ///
    /// Providers are sorted by priority after registration.
    pub fn register_provider(&mut self, provider: DynTemplateProvider) {
        debug!(
            provider_id = %provider.provider_id(),
            priority = provider.priority(),
            "Registering template provider"
        );
        self.providers.push(provider);
        self.providers.sort_by_key(|p| p.priority());
    }

    /// Registers a template loader.
    pub fn register_loader(&mut self, loader: DynTemplateLoader) {
        self.loaders.push(loader);
    }

    /// Registers a template data extender.
    ///
    /// Extenders are sorted by priority after registration.
    pub fn register_extender(&mut self, extender: DynTemplateDataExtender) {
        debug!(
            extender_id = %extender.extender_id(),
            priority = extender.priority(),
            "Registering template data extender"
        );
        self.extenders.push(extender);
        self.extenders.sort_by_key(|e| e.priority());
    }

    /// Registers a component renderer.
    pub fn register_component(&mut self, component: DynComponentRenderer) {
        debug!(
            component_id = %component.component_id(),
            variable_name = %component.variable_name(),
            "Registering component renderer"
        );
        self.components.push(component);
    }

    /// Registers a page data provider.
    ///
    /// Page providers are sorted by priority after registration.
    pub fn register_page_provider(&mut self, provider: DynPageDataProvider) {
        debug!(
            provider_id = %provider.provider_id(),
            pages = ?provider.applies_to_pages(),
            "Registering page data provider"
        );
        self.page_providers.push(provider);
        self.page_providers.sort_by_key(|p| p.priority());
    }

    /// Initializes the registry by loading and compiling all templates.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError::NotInitialized`] if no loaders are registered.
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

        info!(
            templates = self.resolved_templates.len(),
            "Template registry initialized"
        );

        Ok(())
    }

    async fn load_template(&self, definition: &TemplateDefinition) -> Result<String, TemplateError> {
        for loader in &self.loaders {
            if loader.can_load(&definition.source) {
                return loader.load(&definition.source).await.map_err(|e| {
                    TemplateError::LoadError {
                        name: definition.name.clone(),
                        source: e,
                    }
                });
            }
        }
        Err(TemplateError::NoLoader(definition.name.clone()))
    }

    /// Renders a template with the given data.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError::RenderError`] if rendering fails.
    pub fn render(&self, template_name: &str, data: &Value) -> Result<String, TemplateError> {
        self.handlebars
            .render(template_name, data)
            .map_err(|e| TemplateError::RenderError {
                name: template_name.to_string(),
                source: e.into(),
            })
    }

    /// Returns `true` if a template with the given name is registered.
    #[must_use]
    pub fn has_template(&self, name: &str) -> bool {
        self.resolved_templates.contains_key(name)
    }

    /// Returns the template definition for the given name, if it exists.
    #[must_use]
    pub fn get_template(&self, name: &str) -> Option<&TemplateDefinition> {
        self.resolved_templates.get(name)
    }

    /// Returns the template name that handles the given content type.
    #[must_use]
    pub fn get_template_for_content_type(&self, content_type: &str) -> Option<&str> {
        let content_type_owned = content_type.to_string();
        self.resolved_templates
            .iter()
            .find(|(_, def)| def.content_types.contains(&content_type_owned))
            .map(|(name, _)| name.as_str())
    }

    /// Returns all extenders that apply to the given content type.
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

    /// Returns all component renderers that apply to the given content type.
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

    /// Returns all page data providers that apply to the given page type.
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

    /// Returns the provider ID that registered the given template.
    #[must_use]
    pub fn get_template_provider(&self, name: &str) -> Option<&str> {
        self.template_sources.get(name).map(String::as_str)
    }

    /// Returns the names of all registered templates.
    #[must_use]
    pub fn template_names(&self) -> Vec<&str> {
        self.resolved_templates.keys().map(String::as_str).collect()
    }

    /// Returns statistics about the registry.
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

/// Statistics about a [`TemplateRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct RegistryStats {
    /// Number of registered providers.
    pub providers: usize,
    /// Number of resolved templates.
    pub templates: usize,
    /// Number of registered loaders.
    pub loaders: usize,
    /// Number of registered extenders.
    pub extenders: usize,
    /// Number of registered components.
    pub components: usize,
    /// Number of registered page data providers.
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

/// Builder for constructing a [`TemplateRegistry`].
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use systemprompt_templates::{TemplateRegistryBuilder, CoreTemplateProvider, FileSystemLoader};
///
/// async fn example() -> Result<(), systemprompt_templates::TemplateError> {
///     let provider = CoreTemplateProvider::discover_from("./templates").await.unwrap();
///     let loader = FileSystemLoader::new(vec!["./templates".into()]);
///     let registry = TemplateRegistryBuilder::new()
///         .with_provider(Arc::new(provider))
///         .with_loader(Arc::new(loader))
///         .build_and_init()
///         .await?;
///     Ok(())
/// }
/// ```
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
    /// Creates a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a template provider.
    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<DynTemplateProvider>) -> Self {
        self.providers.push(provider.into());
        self
    }

    /// Adds a template loader.
    #[must_use]
    pub fn with_loader(mut self, loader: impl Into<DynTemplateLoader>) -> Self {
        self.loaders.push(loader.into());
        self
    }

    /// Adds a template data extender.
    #[must_use]
    pub fn with_extender(mut self, extender: impl Into<DynTemplateDataExtender>) -> Self {
        self.extenders.push(extender.into());
        self
    }

    /// Adds a component renderer.
    #[must_use]
    pub fn with_component(mut self, component: impl Into<DynComponentRenderer>) -> Self {
        self.components.push(component.into());
        self
    }

    /// Adds a page data provider.
    #[must_use]
    pub fn with_page_provider(mut self, provider: impl Into<DynPageDataProvider>) -> Self {
        self.page_providers.push(provider.into());
        self
    }

    /// Builds the registry without initializing it.
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

    /// Builds and initializes the registry.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError`] if initialization fails.
    pub async fn build_and_init(self) -> Result<TemplateRegistry, TemplateError> {
        let mut registry = self.build();
        registry.initialize().await?;
        Ok(registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_creates_empty_registry() {
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
    fn test_registry_default_equals_new() {
        let registry1 = TemplateRegistry::new();
        let registry2 = TemplateRegistry::default();

        assert_eq!(registry1.stats().providers, registry2.stats().providers);
        assert_eq!(registry1.stats().templates, registry2.stats().templates);
    }

    #[test]
    fn test_builder_creates_empty_registry() {
        let registry = TemplateRegistryBuilder::new().build();
        assert_eq!(registry.stats().providers, 0);
    }

    #[test]
    fn test_has_template_returns_false_for_unregistered() {
        let registry = TemplateRegistry::new();
        assert!(!registry.has_template("nonexistent"));
    }

    #[test]
    fn test_get_template_returns_none_for_unregistered() {
        let registry = TemplateRegistry::new();
        assert!(registry.get_template("nonexistent").is_none());
    }

    #[test]
    fn test_template_names_empty_for_new_registry() {
        let registry = TemplateRegistry::new();
        assert!(registry.template_names().is_empty());
    }

    #[tokio::test]
    async fn test_initialize_fails_without_loaders() {
        let mut registry = TemplateRegistry::new();
        let result = registry.initialize().await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TemplateError::NotInitialized));
    }

    #[test]
    fn test_render_fails_for_unregistered_template() {
        let registry = TemplateRegistry::new();
        let data = serde_json::json!({});
        let result = registry.render("nonexistent", &data);

        assert!(result.is_err());
    }

    #[test]
    fn test_extenders_for_returns_empty_without_extenders() {
        let registry = TemplateRegistry::new();
        assert!(registry.extenders_for("article").is_empty());
    }

    #[test]
    fn test_components_for_returns_empty_without_components() {
        let registry = TemplateRegistry::new();
        assert!(registry.components_for("article").is_empty());
    }

    #[test]
    fn test_page_providers_for_returns_empty_without_providers() {
        let registry = TemplateRegistry::new();
        assert!(registry.page_providers_for("home").is_empty());
    }

    #[test]
    fn test_get_template_provider_returns_none_for_unregistered() {
        let registry = TemplateRegistry::new();
        assert!(registry.get_template_provider("test").is_none());
    }

    #[test]
    fn test_get_template_for_content_type_returns_none_without_templates() {
        let registry = TemplateRegistry::new();
        assert!(registry.get_template_for_content_type("article").is_none());
    }

    #[test]
    fn test_registry_debug_impl() {
        let registry = TemplateRegistry::new();
        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("TemplateRegistry"));
    }

    #[test]
    fn test_builder_debug_impl() {
        let builder = TemplateRegistryBuilder::new();
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("TemplateRegistryBuilder"));
    }

    #[test]
    fn test_registry_stats_debug_impl() {
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
    }
}
