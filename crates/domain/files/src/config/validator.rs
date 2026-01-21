use anyhow::Result;
use systemprompt_traits::validation_report::{
    ValidationError, ValidationReport, ValidationWarning,
};
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

use super::types::FilesConfigYaml;
use super::FilesConfig;

const MAX_RECOMMENDED_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024;
const MIN_VIDEO_FILE_SIZE: u64 = 100 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct FilesConfigValidator {
    config: Option<FilesConfigYaml>,
}

impl FilesConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DomainConfig for FilesConfigValidator {
    fn domain_id(&self) -> &'static str {
        "files"
    }

    fn priority(&self) -> u32 {
        10
    }

    fn load(&mut self, _config: &dyn ConfigProvider) -> Result<(), DomainConfigError> {
        let yaml_config = FilesConfig::load_yaml_config()
            .map_err(|e| DomainConfigError::LoadError(e.to_string()))?;
        self.config = Some(yaml_config);
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("files");

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| DomainConfigError::ValidationError("Not loaded".into()))?;

        if !config.url_prefix.starts_with('/') {
            report.add_error(ValidationError::new(
                "files.urlPrefix",
                "URL prefix must start with '/'",
            ));
        }

        if config.upload.max_file_size_bytes > MAX_RECOMMENDED_FILE_SIZE {
            report.add_warning(
                ValidationWarning::new(
                    "files.upload.maxFileSizeBytes",
                    "Max file size > 2GB may cause memory issues",
                )
                .with_suggestion("Consider using a smaller max file size for better performance"),
            );
        }

        if config.upload.allowed_types.video
            && config.upload.max_file_size_bytes < MIN_VIDEO_FILE_SIZE
        {
            report.add_warning(
                ValidationWarning::new(
                    "files.upload.allowedTypes.video",
                    "Video uploads enabled but max file size < 100MB",
                )
                .with_suggestion("Increase maxFileSizeBytes to at least 100MB for video uploads"),
            );
        }

        Ok(report)
    }
}
