//! Template-registry statistics accessors.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::TemplateRegistry;

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

impl TemplateRegistry {
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
