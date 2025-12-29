//! AI configuration validator.

use super::ValidationConfigProvider;
use crate::ServicesConfig;
use systemprompt_traits::validation_report::{
    ValidationError, ValidationReport, ValidationWarning,
};
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

#[derive(Debug, Default)]
pub struct AiConfigValidator {
    config: Option<ServicesConfig>,
}

impl AiConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DomainConfig for AiConfigValidator {
    fn domain_id(&self) -> &'static str {
        "ai"
    }

    fn priority(&self) -> u32 {
        50
    }

    fn dependencies(&self) -> &[&'static str] {
        &["mcp"]
    }

    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError> {
        let provider = config
            .as_any()
            .downcast_ref::<ValidationConfigProvider>()
            .ok_or_else(|| {
                DomainConfigError::LoadError(
                    "Expected ValidationConfigProvider with merged ServicesConfig".into(),
                )
            })?;

        self.config = Some(provider.services_config().clone());
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("ai");
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| DomainConfigError::ValidationError("Not loaded".into()))?;
        let ai_config = &config.ai;

        Self::validate_default_provider(&mut report, ai_config);
        Self::validate_enabled_providers(&mut report, ai_config);
        Self::validate_mcp_config(&mut report, ai_config);

        if ai_config.history.retention_days == 0 {
            report.add_warning(ValidationWarning::new(
                "ai.history.retention_days",
                "History retention set to 0 days, history will not be retained",
            ));
        }

        Ok(report)
    }
}

impl AiConfigValidator {
    fn validate_default_provider(report: &mut ValidationReport, ai_config: &crate::AiConfig) {
        if ai_config.default_provider.is_empty() {
            report.add_error(ValidationError::new(
                "ai.default_provider",
                "Default AI provider not configured",
            ));
        } else if !ai_config
            .providers
            .contains_key(&ai_config.default_provider)
        {
            report.add_error(
                ValidationError::new(
                    "ai.default_provider",
                    format!(
                        "Default provider '{}' not found in providers",
                        ai_config.default_provider
                    ),
                )
                .with_suggestion("Add the provider to ai.providers or change default_provider"),
            );
        }
    }

    fn validate_enabled_providers(report: &mut ValidationReport, ai_config: &crate::AiConfig) {
        let enabled: Vec<_> = ai_config
            .providers
            .iter()
            .filter(|(_, c)| c.enabled)
            .collect();

        if enabled.is_empty() {
            report.add_error(
                ValidationError::new("ai.providers", "No AI providers are enabled")
                    .with_suggestion("Enable at least one provider in ai.providers"),
            );
        }

        for (name, cfg) in &enabled {
            if cfg.api_key.is_empty() {
                report.add_warning(
                    ValidationWarning::new(
                        format!("ai.providers.{}", name),
                        format!("Provider '{}' is enabled but has no API key", name),
                    )
                    .with_suggestion(format!(
                        "Set {}_API_KEY environment variable",
                        name.to_uppercase()
                    )),
                );
            }
            if cfg.default_model.is_empty() {
                report.add_error(ValidationError::new(
                    format!("ai.providers.{}.default_model", name),
                    format!("Provider '{}' has no default model specified", name),
                ));
            }
        }
    }

    fn validate_mcp_config(report: &mut ValidationReport, ai_config: &crate::AiConfig) {
        if ai_config.mcp.connect_timeout_ms == 0 {
            report.add_error(ValidationError::new(
                "ai.mcp.connect_timeout_ms",
                "MCP connect timeout must be greater than 0",
            ));
        }
        if ai_config.mcp.execution_timeout_ms == 0 {
            report.add_error(ValidationError::new(
                "ai.mcp.execution_timeout_ms",
                "MCP execution timeout must be greater than 0",
            ));
        }
    }
}
