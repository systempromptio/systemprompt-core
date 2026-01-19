use super::ValidationConfigProvider;
use crate::config::RateLimitConfig;
use systemprompt_traits::validation_report::{ValidationReport, ValidationWarning};
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

#[derive(Debug, Default, Clone, Copy)]
pub struct RateLimitsConfigValidator {
    config: Option<RateLimitConfig>,
}

impl RateLimitsConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DomainConfig for RateLimitsConfigValidator {
    fn domain_id(&self) -> &'static str {
        "rate_limits"
    }

    fn priority(&self) -> u32 {
        10
    }

    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError> {
        let provider = config
            .as_any()
            .downcast_ref::<ValidationConfigProvider>()
            .ok_or_else(|| {
                DomainConfigError::LoadError("Expected ValidationConfigProvider".into())
            })?;

        self.config = Some(provider.config().rate_limits);
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("rate_limits");
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| DomainConfigError::ValidationError("Not loaded".into()))?;

        if config.disabled {
            return Ok(report);
        }

        Self::validate_stream_limits(&mut report, config);
        Self::validate_tier_multipliers(&mut report, config);
        Self::validate_agent_limits(&mut report, config);

        Ok(report)
    }
}

impl RateLimitsConfigValidator {
    fn validate_stream_limits(report: &mut ValidationReport, config: &RateLimitConfig) {
        if config.stream_per_second < 10 {
            report.add_warning(
                ValidationWarning::new(
                    "rate_limits.stream_per_second",
                    format!(
                        "stream_per_second={} is restrictive. Users may experience connection \
                         issues.",
                        config.stream_per_second
                    ),
                )
                .with_suggestion("Consider increasing to at least 10 for production use"),
            );
        }
    }

    fn validate_tier_multipliers(report: &mut ValidationReport, config: &RateLimitConfig) {
        if config.tier_multipliers.anon < 0.3 {
            report.add_warning(
                ValidationWarning::new(
                    "rate_limits.tier_multipliers.anon",
                    format!(
                        "tier_multipliers.anon={} severely limits anonymous users.",
                        config.tier_multipliers.anon
                    ),
                )
                .with_suggestion("Consider at least 0.3 for basic anonymous access"),
            );
        }

        if config.tier_multipliers.user < 0.5 {
            report.add_warning(
                ValidationWarning::new(
                    "rate_limits.tier_multipliers.user",
                    format!(
                        "tier_multipliers.user={} restricts authenticated users below baseline.",
                        config.tier_multipliers.user
                    ),
                )
                .with_suggestion("User tier multiplier should typically be 1.0 or higher"),
            );
        }
    }

    fn validate_agent_limits(report: &mut ValidationReport, config: &RateLimitConfig) {
        if config.agents_per_second < 5 {
            report.add_warning(
                ValidationWarning::new(
                    "rate_limits.agents_per_second",
                    format!(
                        "agents_per_second={} may cause agent timeouts under load.",
                        config.agents_per_second
                    ),
                )
                .with_suggestion("Consider at least 10 for stable agent operations"),
            );
        }

        if config.contexts_per_second < 20 {
            report.add_warning(
                ValidationWarning::new(
                    "rate_limits.contexts_per_second",
                    format!(
                        "contexts_per_second={} may slow down conversation operations.",
                        config.contexts_per_second
                    ),
                )
                .with_suggestion("Consider at least 50 for responsive context management"),
            );
        }
    }
}
