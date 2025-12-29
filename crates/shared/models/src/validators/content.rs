//! Content configuration validator.

use super::ValidationConfigProvider;
use crate::ContentConfigRaw;
use std::path::Path;
use systemprompt_traits::validation_report::{ValidationError, ValidationReport};
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

#[derive(Debug, Default)]
pub struct ContentConfigValidator {
    config: Option<ContentConfigRaw>,
}

impl ContentConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DomainConfig for ContentConfigValidator {
    fn domain_id(&self) -> &'static str {
        "content"
    }

    fn priority(&self) -> u32 {
        20
    }

    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError> {
        let provider = config
            .as_any()
            .downcast_ref::<ValidationConfigProvider>()
            .ok_or_else(|| {
                DomainConfigError::LoadError(
                    "Expected ValidationConfigProvider with pre-loaded configs".into(),
                )
            })?;

        self.config = provider.content_config().cloned();
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("content");

        let Some(config) = self.config.as_ref() else {
            return Ok(report);
        };

        for (name, source) in &config.content_sources {
            let source_path = Path::new(&source.path);
            if !source_path.exists() {
                report.add_error(
                    ValidationError::new(
                        format!("content_sources.{}", name),
                        "Content source directory does not exist",
                    )
                    .with_path(&source.path)
                    .with_suggestion("Create the directory or remove the source"),
                );
            }

            if source.source_id.as_str().is_empty() {
                report.add_error(ValidationError::new(
                    format!("content_sources.{}.source_id", name),
                    "Source ID cannot be empty",
                ));
            }

            if source.category_id.as_str().is_empty() {
                report.add_error(ValidationError::new(
                    format!("content_sources.{}.category_id", name),
                    "Category ID cannot be empty",
                ));
            }
        }

        for (name, source) in &config.content_sources {
            if !config.categories.contains_key(source.category_id.as_str()) {
                report.add_error(
                    ValidationError::new(
                        format!("content_sources.{}.category_id", name),
                        format!(
                            "Referenced category '{}' not found in categories",
                            source.category_id
                        ),
                    )
                    .with_suggestion("Add the category to the categories section"),
                );
            }
        }

        Ok(report)
    }
}
