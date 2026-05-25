use clap::Args;

use super::discover_registry;
use super::types::{ExtensionValidationOutput, ValidationError, ValidationWarning};
use crate::CliConfig;
use crate::shared::CommandResult;

#[derive(Debug, Clone, Copy, Args)]
pub struct ValidateArgs {
    #[arg(long, help = "Show detailed validation information")]
    pub verbose: bool,
}

pub(crate) fn execute(
    args: &ValidateArgs,
    _config: &CliConfig,
) -> CommandResult<ExtensionValidationOutput> {
    let registry = discover_registry();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if let Err(e) = registry.validate_dependencies() {
        errors.push(ValidationError {
            extension_id: None,
            error_type: "dependency".to_owned(),
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
                    extension_id: Some(ext.id().to_owned()),
                    error_type: "missing_dependency".to_owned(),
                    message: format!("Missing dependency: {}", dep),
                });
            }
        }

        if ext.config_prefix().is_some() {
            if let Some(schema) = ext.config_schema() {
                if schema.is_null() {
                    warnings.push(ValidationWarning {
                        extension_id: Some(ext.id().to_owned()),
                        warning_type: "config".to_owned(),
                        message: "Config prefix defined but schema is null".to_owned(),
                    });
                }
            }
        }
    }

    for ext in registry.asset_extensions() {
        warnings.push(ValidationWarning {
            extension_id: Some(ext.id().to_owned()),
            warning_type: "asset_validation_skipped".to_owned(),
            message: "Asset validation requires full profile initialization. Use 'systemprompt \
                      infra db validate'."
                .to_owned(),
        });
    }

    let valid = errors.is_empty();
    let extension_count = registry.len();

    let output = ExtensionValidationOutput {
        valid,
        extension_count,
        errors,
        warnings,
    };

    CommandResult::card(output).with_title("Extension Validation")
}
