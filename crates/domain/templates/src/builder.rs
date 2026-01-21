use systemprompt_template_provider::{
    DynComponentRenderer, DynPageDataProvider, DynTemplateDataExtender, DynTemplateLoader,
    DynTemplateProvider,
};

use crate::{TemplateError, TemplateRegistry};

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

    pub async fn build_and_init(self) -> Result<TemplateRegistry, TemplateError> {
        let mut registry = self.build();
        registry.initialize().await?;
        Ok(registry)
    }
}
