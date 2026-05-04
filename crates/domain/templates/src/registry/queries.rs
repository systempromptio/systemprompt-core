use serde_json::Value;
use systemprompt_template_provider::{
    DynComponentRenderer, DynPageDataProvider, DynPagePrerenderer, DynTemplateDataExtender,
    TemplateDefinition,
};

use super::TemplateRegistry;
use crate::TemplateError;

impl TemplateRegistry {
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
}
