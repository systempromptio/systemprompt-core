#![allow(clippy::print_stdout)]

use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::time::Duration;
use systemprompt_core_files::FilesConfig;
use systemprompt_core_logging::services::cli::{
    render_phase_success, render_phase_warning, BrandColors,
};
use systemprompt_core_logging::CliService;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_loader::{ConfigLoader, ExtensionRegistry as McpExtensionRegistry};
use systemprompt_models::validators::{
    AgentConfigValidator, AiConfigValidator, ContentConfigValidator, McpConfigValidator,
    ValidationConfigProvider, WebConfigRaw, WebConfigValidator, WebMetadataRaw,
};
use systemprompt_models::{Config, ContentConfigRaw, ProfileBootstrap, ServicesConfig};
use systemprompt_traits::validation_report::ValidationError;
use systemprompt_traits::{
    ConfigProvider, DomainConfigRegistry, StartupValidationReport, ValidationReport,
};

#[derive(Debug)]
pub struct StartupValidator {
    registry: DomainConfigRegistry,
}

impl StartupValidator {
    pub fn new() -> Self {
        let mut registry = DomainConfigRegistry::new();

        registry.register(Box::new(FilesConfigValidator::new()));
        registry.register(Box::new(WebConfigValidator::new()));
        registry.register(Box::new(ContentConfigValidator::new()));
        registry.register(Box::new(AgentConfigValidator::new()));
        registry.register(Box::new(McpConfigValidator::new()));
        registry.register(Box::new(AiConfigValidator::new()));

        Self { registry }
    }

    pub fn validate(&mut self, config: &Config) -> StartupValidationReport {
        let mut report = StartupValidationReport::new();

        if let Ok(path) = ProfileBootstrap::get_path() {
            report = report.with_profile_path(path);
        }

        CliService::section("Validating configuration");

        let Some(validation_provider) = Self::load_configs(config, &mut report) else {
            return report;
        };

        if self.load_domain_validators(&validation_provider, &mut report) {
            return report;
        }

        self.run_domain_validations(&mut report);

        Self::validate_mcp_manifests(config, validation_provider.services_config(), &mut report);

        if report.has_errors() {
            return report;
        }

        Self::validate_extensions(config, &mut report);

        println!();

        report
    }

    fn load_configs(
        config: &Config,
        report: &mut StartupValidationReport,
    ) -> Option<ValidationConfigProvider> {
        let spinner = create_spinner("Loading services config");
        let services_config = match ConfigLoader::load() {
            Ok(cfg) => {
                spinner.finish_and_clear();
                CliService::phase_success("Services config", Some("includes merged"));
                cfg
            },
            Err(e) => {
                spinner.finish_and_clear();
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

        provider = load_content_config(config, provider);
        provider = load_web_config(config, provider);
        provider = load_web_metadata(config, provider);

        Some(provider)
    }

    fn load_domain_validators(
        &mut self,
        provider: &ValidationConfigProvider,
        report: &mut StartupValidationReport,
    ) -> bool {
        println!();
        println!(
            "{} {}",
            BrandColors::primary("▸"),
            BrandColors::white_bold("Validating domains")
        );

        for validator in self.registry.validators_mut() {
            let domain_id = validator.domain_id();
            let spinner = create_spinner(&format!("Loading {}", domain_id));

            match validator.load(provider) {
                Ok(()) => {
                    spinner.finish_and_clear();
                },
                Err(e) => {
                    spinner.finish_and_clear();
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

    fn run_domain_validations(&self, report: &mut StartupValidationReport) {
        for validator in self.registry.validators_sorted() {
            let domain_id = validator.domain_id();
            let spinner = create_spinner(&format!("Validating {}", domain_id));

            match validator.validate() {
                Ok(domain_report) => {
                    spinner.finish_and_clear();
                    Self::print_domain_result(&domain_report, domain_id);
                    report.add_domain(domain_report);
                },
                Err(e) => {
                    spinner.finish_and_clear();
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

    fn validate_extensions(config: &Config, report: &mut StartupValidationReport) {
        let extensions = ExtensionRegistry::discover();
        let config_extensions = extensions.config_extensions();

        if config_extensions.is_empty() {
            return;
        }

        println!();
        println!(
            "{} {}",
            BrandColors::primary("▸"),
            BrandColors::white_bold("Validating extensions")
        );

        for ext in config_extensions {
            Self::validate_single_extension(config, ext.as_ref(), report);
        }
    }

    fn validate_single_extension(
        _config: &Config,
        ext: &dyn systemprompt_extension::Extension,
        _report: &mut StartupValidationReport,
    ) {
        let ext_id = ext.id();
        if ext.config_prefix().is_none() {
            return;
        }

        render_phase_success(&format!("[ext:{}]", ext_id), Some("loaded"));
    }

    fn validate_mcp_manifests(
        config: &Config,
        services_config: &ServicesConfig,
        report: &mut StartupValidationReport,
    ) {
        let registry = McpExtensionRegistry::build(
            Path::new(&config.system_path),
            config.is_cloud,
            &config.bin_path,
        );

        let mut mcp_errors: Vec<ValidationError> = Vec::new();

        for (name, deployment) in &services_config.mcp_servers {
            if !deployment.enabled {
                continue;
            }

            if let Err(e) = registry.get_path(&deployment.binary) {
                mcp_errors.push(
                    ValidationError::new(
                        format!("mcp_servers.{}.binary", name),
                        format!(
                            "Manifest not found for binary '{}': {}",
                            deployment.binary, e
                        ),
                    )
                    .with_suggestion(format!(
                        "Ensure manifest.yaml exists at extensions/mcp/{}/manifest.yaml",
                        deployment.binary
                    )),
                );
            }
        }

        if mcp_errors.is_empty() {
            return;
        }

        if let Some(mcp_report) = report.domains.iter_mut().find(|d| d.domain == "mcp") {
            for error in mcp_errors {
                mcp_report.add_error(error);
            }
        } else {
            let mut mcp_report = ValidationReport::new("mcp");
            for error in mcp_errors {
                mcp_report.add_error(error);
            }
            report.add_domain(mcp_report);
        }
    }
}

impl Default for StartupValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct FilesConfigValidator {
    initialized: bool,
}

impl FilesConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl systemprompt_traits::DomainConfig for FilesConfigValidator {
    fn domain_id(&self) -> &'static str {
        "files"
    }

    fn priority(&self) -> u32 {
        5
    }

    fn load(
        &mut self,
        _config: &dyn ConfigProvider,
    ) -> Result<(), systemprompt_traits::DomainConfigError> {
        self.initialized = FilesConfig::get_optional().is_some();
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, systemprompt_traits::DomainConfigError> {
        let mut report = ValidationReport::new("files");

        let Some(files_config) = FilesConfig::get_optional() else {
            return Ok(report);
        };

        let errors = files_config.validate_storage_structure();
        for error_msg in errors {
            report
                .add_error(ValidationError::new("storage", &error_msg).with_suggestion(
                    "Ensure static files are copied to storage during deployment",
                ));
        }

        Ok(report)
    }
}

fn load_content_config(
    config: &Config,
    mut provider: ValidationConfigProvider,
) -> ValidationConfigProvider {
    if let Some(content_config_path) = config.get("content_config_path") {
        let spinner = create_spinner("Loading content config");
        match load_yaml_config::<ContentConfigRaw>(Path::new(&content_config_path)) {
            Ok(cfg) => {
                spinner.finish_and_clear();
                CliService::phase_success("Content config", None);
                provider = provider.with_content_config(cfg);
            },
            Err(e) => {
                spinner.finish_and_clear();
                CliService::phase_warning("Content config", Some(&e));
            },
        }
    }
    provider
}

fn load_web_config(
    config: &Config,
    mut provider: ValidationConfigProvider,
) -> ValidationConfigProvider {
    if let Some(web_config_path) = config.get("web_config_path") {
        let spinner = create_spinner("Loading web config");
        match load_yaml_config::<WebConfigRaw>(Path::new(&web_config_path)) {
            Ok(cfg) => {
                spinner.finish_and_clear();
                CliService::phase_success("Web config", None);
                provider = provider.with_web_config(cfg);
            },
            Err(e) => {
                spinner.finish_and_clear();
                CliService::phase_warning("Web config", Some(&e));
            },
        }
    }
    provider
}

fn load_web_metadata(
    config: &Config,
    mut provider: ValidationConfigProvider,
) -> ValidationConfigProvider {
    if let Some(web_metadata_path) = config.get("web_metadata_path") {
        let spinner = create_spinner("Loading web metadata");
        match load_yaml_config::<WebMetadataRaw>(Path::new(&web_metadata_path)) {
            Ok(cfg) => {
                spinner.finish_and_clear();
                CliService::phase_success("Web metadata", None);
                provider = provider.with_web_metadata(cfg);
            },
            Err(e) => {
                spinner.finish_and_clear();
                CliService::phase_warning("Web metadata", Some(&e));
            },
        }
    }
    provider
}

fn create_spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner:.208} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    spinner.set_message(format!("{}...", message));
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

pub fn display_validation_report(report: &StartupValidationReport) {
    println!();
    println!(
        "{} {}",
        BrandColors::stopped("✗"),
        BrandColors::white_bold("Validation Failed")
    );

    if let Some(ref path) = report.profile_path {
        println!(
            "  {} {}",
            BrandColors::dim("Profile:"),
            BrandColors::highlight(&path.display().to_string())
        );
    }

    println!();
    println!(
        "  {} error(s) found:",
        BrandColors::stopped(&report.error_count().to_string())
    );

    for domain in &report.domains {
        display_domain_errors(domain);
    }

    for ext in &report.extensions {
        display_extension_errors(ext);
    }

    println!();
}

fn display_domain_errors(domain: &ValidationReport) {
    if !domain.has_errors() {
        return;
    }

    println!();
    println!(
        "  {} {}",
        BrandColors::stopped("▸"),
        BrandColors::white_bold(&domain.domain)
    );

    for error in &domain.errors {
        println!("    {} {}", BrandColors::dim("field:"), error.field);
        println!("    {} {}", BrandColors::dim("error:"), error.message);
        if let Some(ref path) = error.path {
            println!("    {} {}", BrandColors::dim("path:"), path.display());
        }
        if let Some(ref suggestion) = error.suggestion {
            println!("    {} {}", BrandColors::highlight("fix:"), suggestion);
        }
    }
}

fn display_extension_errors(ext: &ValidationReport) {
    if !ext.has_errors() {
        return;
    }

    println!();
    println!(
        "  {} {}",
        BrandColors::stopped("▸"),
        BrandColors::white_bold(&ext.domain)
    );

    for error in &ext.errors {
        println!("    {} {}", BrandColors::dim("field:"), error.field);
        println!("    {} {}", BrandColors::dim("error:"), error.message);
    }
}

pub fn display_validation_warnings(report: &StartupValidationReport) {
    if report.warning_count() == 0 {
        return;
    }

    println!(
        "  {} warning(s):",
        BrandColors::starting(&report.warning_count().to_string())
    );

    for domain in &report.domains {
        for warning in &domain.warnings {
            println!();
            println!(
                "  {} [{}] {}",
                BrandColors::starting("⚠"),
                domain.domain,
                warning.field
            );
            println!("    {}", warning.message);
            if let Some(ref suggestion) = warning.suggestion {
                println!("    {} {}", BrandColors::highlight("fix:"), suggestion);
            }
        }
    }

    println!();
}

fn load_yaml_config<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
    serde_yaml::from_str(&content).map_err(|e| format!("Cannot parse {}: {}", path.display(), e))
}
