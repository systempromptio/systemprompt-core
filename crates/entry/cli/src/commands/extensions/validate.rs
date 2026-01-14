use anyhow::Result;
use clap::Args;
use systemprompt_extension::ExtensionRegistry;

use super::types::{ExtensionValidationOutput, ValidationError, ValidationWarning};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Args)]
pub struct ValidateArgs {
    #[arg(long, help = "Show detailed validation information")]
    pub verbose: bool,
}

pub fn execute(args: ValidateArgs, _config: &CliConfig) -> Result<CommandResult<ExtensionValidationOutput>> {
    let registry = ExtensionRegistry::discover();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if let Err(e) = registry.validate_dependencies() {
        errors.push(ValidationError {
            extension_id: None,
            error_type: "dependency".to_string(),
            message: e.to_string(),
        });
    }

    for ext in registry.extensions() {
        if ext.dependencies().is_empty() && args.verbose {
            continue;
        }

        for dep in ext.dependencies() {
            if !registry.has(dep) {
                errors.push(ValidationError {
                    extension_id: Some(ext.id().to_string()),
                    error_type: "missing_dependency".to_string(),
                    message: format!("Missing dependency: {}", dep),
                });
            }
        }

        if ext.config_prefix().is_some() {
            if let Some(schema) = ext.config_schema() {
                if schema.is_null() {
                    warnings.push(ValidationWarning {
                        extension_id: Some(ext.id().to_string()),
                        warning_type: "config".to_string(),
                        message: "Config prefix defined but schema is null".to_string(),
                    });
                }
            }
        }

        if ext.has_schemas() && ext.migration_weight() == 100 && args.verbose {
            warnings.push(ValidationWarning {
                extension_id: Some(ext.id().to_string()),
                warning_type: "migration_weight".to_string(),
                message: "Using default migration weight (100)".to_string(),
            });
        }
    }

    let valid = errors.is_empty();
    let extension_count = registry.len();

    let output = ExtensionValidationOutput {
        valid,
        extension_count,
        errors,
        warnings,
    };

    Ok(CommandResult::card(output).with_title("Extension Validation"))
}
