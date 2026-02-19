use std::path::Path;

use crate::{DiskSkillConfig, SKILL_CONFIG_FILENAME};
use systemprompt_traits::validation_report::{ValidationError, ValidationReport};
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

#[derive(Debug, Default)]
pub struct SkillConfigValidator {
    skills_path: Option<String>,
}

impl SkillConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DomainConfig for SkillConfigValidator {
    fn domain_id(&self) -> &'static str {
        "skills"
    }

    fn priority(&self) -> u32 {
        25
    }

    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError> {
        let skills_path = config
            .get("skills_path")
            .ok_or_else(|| DomainConfigError::NotFound("skills_path not configured".into()))?;

        self.skills_path = Some(skills_path);
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("skills");

        let skills_path = self
            .skills_path
            .as_ref()
            .ok_or_else(|| DomainConfigError::ValidationError("Skills path not set".into()))?;

        let skills_dir = Path::new(skills_path);
        if !skills_dir.exists() {
            report.add_error(
                ValidationError::new("skills_path", "Skills directory does not exist")
                    .with_path(skills_dir)
                    .with_suggestion("Create the skills directory or update skills_path in config"),
            );
            return Ok(report);
        }

        let entries = std::fs::read_dir(skills_dir).map_err(|e| {
            DomainConfigError::LoadError(format!("Cannot read skills directory: {e}"))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                DomainConfigError::LoadError(format!("Cannot read directory entry: {e}"))
            })?;

            if !entry.path().is_dir() {
                continue;
            }

            let dir_name = entry.file_name().to_string_lossy().to_string();
            let config_path = entry.path().join(SKILL_CONFIG_FILENAME);

            if !config_path.exists() {
                report.add_error(
                    ValidationError::new(
                        format!("skills.{dir_name}"),
                        format!("Missing {SKILL_CONFIG_FILENAME}"),
                    )
                    .with_path(&config_path)
                    .with_suggestion("Add a config.yaml with id, name, and description"),
                );
                continue;
            }

            let config_text = match std::fs::read_to_string(&config_path) {
                Ok(text) => text,
                Err(e) => {
                    report.add_error(
                        ValidationError::new(
                            format!("skills.{dir_name}"),
                            format!("Cannot read {SKILL_CONFIG_FILENAME}: {e}"),
                        )
                        .with_path(&config_path),
                    );
                    continue;
                },
            };

            let config: DiskSkillConfig = match serde_yaml::from_str(&config_text) {
                Ok(cfg) => cfg,
                Err(e) => {
                    report.add_error(
                        ValidationError::new(
                            format!("skills.{dir_name}"),
                            format!("Invalid {SKILL_CONFIG_FILENAME}: {e}"),
                        )
                        .with_path(&config_path)
                        .with_suggestion(
                            "Ensure config.yaml has required fields: id, name, description",
                        ),
                    );
                    continue;
                },
            };

            let content_file = config.content_file();
            let content_path = entry.path().join(content_file);
            if !content_path.exists() {
                report.add_error(
                    ValidationError::new(
                        format!("skills.{dir_name}.file"),
                        format!("Content file '{content_file}' not found"),
                    )
                    .with_path(&content_path)
                    .with_suggestion(
                        "Create the content file or update the file field in config.yaml",
                    ),
                );
            }
        }

        Ok(report)
    }
}
