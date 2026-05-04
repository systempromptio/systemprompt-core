use std::path::Path;

use systemprompt_template_provider::{PartialSource, TemplateDefinition};
use tracing::{debug, info, warn};

use super::TemplateRegistry;
use crate::TemplateError;

impl TemplateRegistry {
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

        all_templates.sort_by_key(|a| a.0.priority);

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
                message: e.to_string(),
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
                        message: e.to_string(),
                    }
                });
            }
        }
        Err(TemplateError::NoLoader(definition.name.clone()))
    }
}
