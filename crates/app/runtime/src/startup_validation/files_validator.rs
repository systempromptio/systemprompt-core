use systemprompt_files::FilesConfig;
use systemprompt_traits::validation_report::ValidationError;
use systemprompt_traits::{ConfigProvider, ValidationReport};

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
