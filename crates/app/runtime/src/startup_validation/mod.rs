//! Startup configuration and extension validation.
//!
//! [`StartupValidator`] composes per-domain validators via
//! [`DomainConfigRegistry`], aggregates results into a
//! [`StartupValidationReport`], and is consumed by `systemprompt`'s
//! boot sequence to gate startup on configuration health.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config_loaders;
mod display;
mod extension_validator;
mod mcp_validator;

use systemprompt_config::SkillConfigValidator;
use systemprompt_config::paths::AppPaths;
use systemprompt_extension::BuiltState;
use systemprompt_loader::BootContext;
use systemprompt_logging::services::cli::{
    BrandColors, render_phase_success, render_phase_warning,
};
use systemprompt_logging::{CliService, ExportCapability, OutputState};
use systemprompt_manifest::Config;
use systemprompt_manifest::validators::validation_report::ValidationIssue;
use systemprompt_manifest::validators::{
    DomainConfigRegistry, RateLimitsConfigValidator, StartupValidationReport,
    ValidationConfigProvider, ValidationReport, WebConfigValidator,
};

use crate::error::RuntimeResult;
use config_loaders::{create_spinner, load_content_config, load_web_config, load_web_metadata};
use extension_validator::validate_extensions;
use mcp_validator::validate_mcp_manifests;

pub use display::{display_validation_report, display_validation_warnings};
pub use mcp_validator::{collect_manifest_errors, merge_mcp_errors};

#[derive(Debug)]
pub struct StartupValidator {
    boot: BootContext,
    services: std::path::PathBuf,
    registry: DomainConfigRegistry,
}

impl StartupValidator {
    pub fn new(boot: BootContext) -> RuntimeResult<Self> {
        let mut registry = DomainConfigRegistry::new();

        let profile = boot.profile();
        let app_paths = AppPaths::from_profile(
            &profile.paths,
            profile.path_resolution(),
            boot.services_root().map(|root| root.path.as_path()),
        )?;
        registry.register(Box::new(RateLimitsConfigValidator::new()));
        registry.register(Box::new(WebConfigValidator::new()));
        registry.register(Box::new(SkillConfigValidator::new()));

        Ok(Self {
            boot,
            services: app_paths.system().services().to_path_buf(),
            registry,
        })
    }

    pub fn validate(&mut self, config: &Config, built: &BuiltState) -> StartupValidationReport {
        let mut report = StartupValidationReport::new();
        let verbose = OutputState::current().is_startup_mode();

        report = report.with_profile_path(self.boot.profile_path());
        if let Err(error) =
            ExportCapability::COMPILED.admit(self.boot.profile().observability.otlp())
        {
            let mut observability = ValidationReport::new("observability");
            observability.add_error(ValidationIssue::new(
                "observability.otlp",
                error.to_string(),
            ));
            report.add_domain(observability);
            return report;
        }
        for member in built.members() {
            for validator in member.sink.domain_configs() {
                self.registry.register(validator(&self.services));
            }
        }

        if verbose {
            CliService::section("Validating configuration");
        }

        let Some(validation_provider) = self.load_configs(config, &mut report, verbose) else {
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

        validate_extensions(built, &self.boot, config, &mut report, verbose);

        if verbose {
            CliService::output("");
        }

        report
    }

    fn load_configs(
        &self,
        config: &Config,
        report: &mut StartupValidationReport,
        verbose: bool,
    ) -> Option<ValidationConfigProvider> {
        let spinner = if verbose {
            Some(create_spinner("Loading services config"))
        } else {
            None
        };
        let services_config = match self.boot.load_services_config() {
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
                domain_report.add_error(ValidationIssue::new(
                    "services_config",
                    format!("Failed to load: {}", e),
                ));
                report.add_domain(domain_report);
                return None;
            },
        };

        let mut provider = ValidationConfigProvider::new(config.clone(), services_config)
            .with_secrets_config(self.boot.profile().secrets.clone());

        provider = load_content_config(config, provider, verbose);
        provider = load_web_config(config, provider, verbose);
        provider = load_web_metadata(config, provider, verbose);

        Some(provider)
    }

    fn load_domain_validators(
        &mut self,
        provider: &ValidationConfigProvider,
        report: &mut StartupValidationReport,
        verbose: bool,
    ) -> bool {
        if verbose {
            CliService::output("");
            CliService::output(&format!(
                "{} {}",
                BrandColors::primary("▸"),
                BrandColors::white_bold("Validating domains")
            ));
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
                    CliService::output(&format!(
                        "  {} [{}] {}",
                        BrandColors::stopped("✗"),
                        domain_id,
                        e
                    ));

                    let mut domain_report = ValidationReport::new(domain_id);
                    domain_report.add_error(ValidationIssue::new(
                        format!("{}_config", domain_id),
                        format!("Failed to load: {}", e),
                    ));
                    report.add_domain(domain_report);
                },
            }
        }

        report.has_errors()
    }

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
                    CliService::output(&format!(
                        "  {} [{}] {}",
                        BrandColors::stopped("✗"),
                        domain_id,
                        e
                    ));

                    let mut domain_report = ValidationReport::new(domain_id);
                    domain_report.add_error(ValidationIssue::new(
                        format!("{}_validation", domain_id),
                        format!("Validation error: {}", e),
                    ));
                    report.add_domain(domain_report);
                },
            }
        }
    }

    fn print_domain_result(domain_report: &ValidationReport, domain_id: &str) {
        if domain_report.has_errors() {
            CliService::output(&format!(
                "  {} [{}] {} error(s)",
                BrandColors::stopped("✗"),
                domain_id,
                domain_report.errors.len()
            ));
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
