//! Domain validator for files configuration (size bounds, type sanity).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_config::paths::AppPaths;
use systemprompt_manifest::validators::validation_report::{
    ValidationIssue, ValidationReport, ValidationWarning,
};
use systemprompt_manifest::validators::{DomainConfig, DomainConfigError};
use systemprompt_traits::ConfigProvider;

use super::FilesConfig;
use super::documents::{MAX_DOCUMENT_UPLOAD_BYTES, MIN_DOCUMENT_UPLOAD_BYTES};
use super::types::FilesConfigYaml;

const MAX_RECOMMENDED_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024;
const MIN_VIDEO_FILE_SIZE: u64 = 100 * 1024 * 1024;

fn is_valid_header_value(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| matches!(c, ' '..='~'))
}

#[derive(Debug)]
pub struct FilesConfigValidator {
    paths: AppPaths,
    config: Option<FilesConfigYaml>,
}

impl FilesConfigValidator {
    pub const fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            config: None,
        }
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
        let yaml_config = FilesConfig::load_yaml_config(&self.paths)
            .map_err(|e| DomainConfigError::load("files.yaml", e))?;
        self.config = Some(yaml_config);
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("files");

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| DomainConfigError::ValidationError {
                message: "Not loaded".into(),
            })?;

        if !config.url_prefix.starts_with('/') {
            report.add_error(ValidationIssue::new(
                "files.urlPrefix",
                "URL prefix must start with '/'",
            ));
        }

        if let Some(cache_control) = config.cache_control.as_deref()
            && !is_valid_header_value(cache_control)
        {
            report.add_error(ValidationIssue::new(
                "files.cacheControl",
                "Cache-Control must be a non-empty string of printable ASCII characters",
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

        let documents = &config.documents;
        if !(MIN_DOCUMENT_UPLOAD_BYTES..=MAX_DOCUMENT_UPLOAD_BYTES)
            .contains(&documents.max_upload_bytes)
        {
            report.add_error(ValidationIssue::new(
                "files.documents.maxUploadBytes",
                "Document upload limit must be between 1 MiB and 2 GiB",
            ));
        }
        if documents.blob_gc_grace_hours == 0 {
            report.add_error(ValidationIssue::new(
                "files.documents.blobGcGraceHours",
                "Blob garbage-collection grace must be at least one hour",
            ));
        }
        if documents.presence_ttl_secs == 0 {
            report.add_error(ValidationIssue::new(
                "files.documents.presenceTtlSecs",
                "Presence time-to-live must be at least one second",
            ));
        }

        Ok(report)
    }
}
