//! Web configuration validator.

use super::validation_config_provider::{WebConfigRaw, WebMetadataRaw};
use super::ValidationConfigProvider;
use std::path::Path;
use systemprompt_traits::validation_report::{ValidationError, ValidationReport};
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

#[derive(Debug, Default)]
pub struct WebConfigValidator {
    config: Option<WebConfigRaw>,
    metadata: Option<WebMetadataRaw>,
    config_path: Option<String>,
}

impl WebConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DomainConfig for WebConfigValidator {
    fn domain_id(&self) -> &'static str {
        "web"
    }

    fn priority(&self) -> u32 {
        10
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

        self.config = provider.web_config().cloned();
        self.metadata = provider.web_metadata().cloned();
        self.config_path = config.get("web_config_path");
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("web");

        let Some(cfg) = self.config.as_ref() else {
            return Ok(report);
        };

        if let Some(ref base_url) = cfg.base_url {
            if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
                report.add_error(
                    ValidationError::new(
                        "web_config.base_url",
                        format!("Invalid URL format: {}", base_url),
                    )
                    .with_suggestion("URL must start with http:// or https://"),
                );
            }
        }

        if let Some(ref site_name) = cfg.site_name {
            if site_name.is_empty() {
                report.add_error(ValidationError::new(
                    "web_config.site_name",
                    "Site name cannot be empty",
                ));
            }
        }

        if let Some(ref path) = self.config_path {
            let parent = Path::new(path).parent();
            if let Some(dir) = parent {
                if !dir.exists() {
                    report.add_error(
                        ValidationError::new("web_config", "Web config directory does not exist")
                            .with_path(dir),
                    );
                }
            }
        }

        Ok(report)
    }
}
