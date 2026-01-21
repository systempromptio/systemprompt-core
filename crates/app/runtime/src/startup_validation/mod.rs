mod config_loaders;
mod display;
mod extension_validator;
mod files_validator;
mod mcp_validator;

use systemprompt_logging::services::cli::{
    render_phase_success, render_phase_warning, BrandColors,
};
use systemprompt_logging::{is_startup_mode, CliService};
use systemprompt_models::validators::{
    AgentConfigValidator, AiConfigValidator, ContentConfigValidator, McpConfigValidator,
    RateLimitsConfigValidator, ValidationConfigProvider, WebConfigValidator,
};
use systemprompt_models::{Config, ProfileBootstrap};
use systemprompt_traits::validation_report::ValidationError;
use systemprompt_traits::{DomainConfigRegistry, StartupValidationReport, ValidationReport};

use config_loaders::{create_spinner, load_content_config, load_web_config, load_web_metadata};
use extension_validator::validate_extensions;
use mcp_validator::validate_mcp_manifests;

pub use display::{display_validation_report, display_validation_warnings};
pub use files_validator::FilesConfigValidator;

#[derive(Debug)]
pub struct StartupValidator {
    registry: DomainConfigRegistry,
}

impl StartupValidator {
    pub fn new() -> Self {
        let mut registry = DomainConfigRegistry::new();

        registry.register(Box::new(FilesConfigValidator::new()));
        registry.register(Box::new(RateLimitsConfigValidator::new()));
        registry.register(Box::new(WebConfigValidator::new()));
        registry.register(Box::new(ContentConfigValidator::new()));
        registry.register(Box::new(AgentConfigValidator::new()));
        registry.register(Box::new(McpConfigValidator::new()));
        registry.register(Box::new(AiConfigValidator::new()));

        Self { registry }
    }

    #[allow(clippy::print_stdout)]
    pub fn validate(&mut self, config: &Config) -> StartupValidationReport {
        let mut report = StartupValidationReport::new();
        let verbose = is_startup_mode();

        if let Ok(path) = ProfileBootstrap::get_path() {
            report = report.with_profile_path(path);
        }

        if verbose {
            CliService::section("Validating configuration");
        }

        let Some(validation_provider) = Self::load_configs(config, &mut report, verbose) else {
            return report;
        };

        if self.load_domain_validators(&validation_provider, &mut report, verbose) {
            return report;
        }

        self.run_domain_validations(&mut report, verbose);

        validate_mcp_manifests(config, validation_provider.services_config(), &mut report);

        if report.has_errors() {
            return report;
        }

        validate_extensions(config, &mut report, verbose);

        if verbose {
            println!();
        }

        report
    }

    fn load_configs(
        config: &Config,
        report: &mut StartupValidationReport,
        verbose: bool,
    ) -> Option<ValidationConfigProvider> {
        let spinner = if verbose {
            Some(create_spinner("Loading services config"))
        } else {
            None
        };
        let services_config = match systemprompt_loader::ConfigLoader::load() {
            Ok(cfg) => {
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                if verbose {
                    CliService::phase_success("Services config", Some("includes merged"));
                }
                cfg
            },
            Err(e) => {
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                CliService::error(&format!("Services config: {}", e));
                let mut domain_report = ValidationReport::new("services");
                domain_report.add_error(ValidationError::new(
                    "services_config",
                    format!("Failed to load: {}", e),
                ));
                report.add_domain(domain_report);
                return None;
            },
        };

        let mut provider = ValidationConfigProvider::new(config.clone(), services_config);

        provider = load_content_config(config, provider, verbose);
        provider = load_web_config(config, provider, verbose);
        provider = load_web_metadata(config, provider, verbose);

        Some(provider)
    }

    #[allow(clippy::print_stdout)]
    fn load_domain_validators(
        &mut self,
        provider: &ValidationConfigProvider,
        report: &mut StartupValidationReport,
        verbose: bool,
    ) -> bool {
        if verbose {
            println!();
            println!(
                "{} {}",
                BrandColors::primary("▸"),
                BrandColors::white_bold("Validating domains")
            );
        }

        for validator in self.registry.validators_mut() {
            let domain_id = validator.domain_id();
            let spinner = if verbose {
                Some(create_spinner(&format!("Loading {}", domain_id)))
            } else {
                None
            };

            match validator.load(provider) {
                Ok(()) => {
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }
                },
                Err(e) => {
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }
                    println!("  {} [{}] {}", BrandColors::stopped("✗"), domain_id, e);

                    let mut domain_report = ValidationReport::new(domain_id);
                    domain_report.add_error(ValidationError::new(
                        format!("{}_config", domain_id),
                        format!("Failed to load: {}", e),
                    ));
                    report.add_domain(domain_report);
                },
            }
        }

        report.has_errors()
    }

    #[allow(clippy::print_stdout)]
    fn run_domain_validations(&self, report: &mut StartupValidationReport, verbose: bool) {
        for validator in self.registry.validators_sorted() {
            let domain_id = validator.domain_id();
            let spinner = if verbose {
                Some(create_spinner(&format!("Validating {}", domain_id)))
            } else {
                None
            };

            match validator.validate() {
                Ok(domain_report) => {
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }
                    if verbose {
                        Self::print_domain_result(&domain_report, domain_id);
                    }
                    report.add_domain(domain_report);
                },
                Err(e) => {
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }
                    println!("  {} [{}] {}", BrandColors::stopped("✗"), domain_id, e);

                    let mut domain_report = ValidationReport::new(domain_id);
                    domain_report.add_error(ValidationError::new(
                        format!("{}_validation", domain_id),
                        format!("Validation error: {}", e),
                    ));
                    report.add_domain(domain_report);
                },
            }
        }
    }

    #[allow(clippy::print_stdout)]
    fn print_domain_result(domain_report: &ValidationReport, domain_id: &str) {
        if domain_report.has_errors() {
            println!(
                "  {} [{}] {} error(s)",
                BrandColors::stopped("✗"),
                domain_id,
                domain_report.errors.len()
            );
        } else if domain_report.has_warnings() {
            render_phase_warning(
                &format!("[{}]", domain_id),
                Some(&format!("{} warning(s)", domain_report.warnings.len())),
            );
        } else {
            render_phase_success(&format!("[{}]", domain_id), Some("valid"));
        }
    }
}

impl Default for StartupValidator {
    fn default() -> Self {
        Self::new()
    }
}
