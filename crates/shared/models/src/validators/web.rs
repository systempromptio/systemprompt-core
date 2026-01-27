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

        self.validate_branding(&mut report);

        Ok(report)
    }
}

impl WebConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }

    fn validate_branding(&self, report: &mut ValidationReport) {
        let Some(cfg) = self.config.as_ref() else {
            return;
        };

        let Some(branding) = cfg.branding.as_ref() else {
            report.add_error(
                ValidationError::new(
                    "web_config.branding",
                    "Missing 'branding' section in web.yaml",
                )
                .with_suggestion("Add a 'branding' section with copyright, logo, favicon, twitter_handle, and display_sitename"),
            );
            return;
        };

        if branding.copyright.as_ref().map_or(true, |s| s.is_empty()) {
            report.add_error(
                ValidationError::new(
                    "web_config.branding.copyright",
                    "Missing required field 'copyright'",
                )
                .with_suggestion("Add 'copyright: \"© 2024 Your Company\"' under branding"),
            );
        }

        if branding.twitter_handle.as_ref().map_or(true, |s| s.is_empty()) {
            report.add_error(
                ValidationError::new(
                    "web_config.branding.twitter_handle",
                    "Missing required field 'twitter_handle'",
                )
                .with_suggestion("Add 'twitter_handle: \"@yourhandle\"' under branding"),
            );
        }

        if branding.display_sitename.is_none() {
            report.add_error(
                ValidationError::new(
                    "web_config.branding.display_sitename",
                    "Missing required field 'display_sitename'",
                )
                .with_suggestion("Add 'display_sitename: true' under branding"),
            );
        }

        if branding.favicon.as_ref().map_or(true, |s| s.is_empty()) {
            report.add_error(
                ValidationError::new(
                    "web_config.branding.favicon",
                    "Missing required field 'favicon'",
                )
                .with_suggestion("Add 'favicon: \"/favicon.ico\"' under branding"),
            );
        }

        let logo_svg = branding
            .logo
            .as_ref()
            .and_then(|l| l.primary.as_ref())
            .and_then(|p| p.svg.as_ref());

        if logo_svg.map_or(true, |s| s.is_empty()) {
            report.add_error(
                ValidationError::new(
                    "web_config.branding.logo.primary.svg",
                    "Missing required field 'logo.primary.svg'",
                )
                .with_suggestion("Add 'logo: { primary: { svg: \"/logo.svg\" } }' under branding"),
            );
        }
    }
}
