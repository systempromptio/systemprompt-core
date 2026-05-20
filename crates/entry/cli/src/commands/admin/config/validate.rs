use anyhow::{Result, anyhow};
use clap::Args;

use super::types::{ConfigFileInfo, ConfigSection, ConfigValidateOutput, read_yaml_file};
use crate::CliConfig;
use crate::shared::CommandResult;
use systemprompt_logging::CliService;
use systemprompt_models::profile::Profile;

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    #[arg(value_name = "PATH_OR_SECTION")]
    pub target: Option<String>,

    #[arg(long)]
    pub strict: bool,

    /// Print the generated JSON schema for the `Profile` config type
    /// instead of validating any file.
    #[arg(long)]
    pub schema: bool,
}

pub fn execute(
    args: &ValidateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ConfigValidateOutput>> {
    if args.schema {
        return print_profile_schema();
    }

    // A target that is an existing `.yaml`/`.yml` file is treated as a
    // full profile document and validated against the `Profile` schema.
    if let Some(target) = &args.target {
        let path = std::path::PathBuf::from(target);
        if path.exists() && is_yaml_file(&path) && target.parse::<ConfigSection>().is_err() {
            return validate_profile_file(&path);
        }
    }

    let files_to_validate = if let Some(target) = &args.target {
        if let Ok(section) = target.parse::<ConfigSection>() {
            section.all_files()?
        } else {
            vec![std::path::PathBuf::from(target)]
        }
    } else {
        let mut all_files = Vec::new();
        for section in ConfigSection::all() {
            if let Ok(files) = section.all_files() {
                all_files.extend(files);
            }
        }
        all_files
    };

    let mut results = Vec::new();
    let mut all_valid = true;

    for file_path in files_to_validate {
        let section = detect_section(&file_path);
        let exists = file_path.exists();

        let (valid, error) = if exists {
            match validate_file(&file_path, args.strict) {
                Ok(()) => (true, None),
                Err(e) => {
                    all_valid = false;
                    (false, Some(e.to_string()))
                },
            }
        } else {
            all_valid = false;
            (false, Some("File not found".to_string()))
        };

        results.push(ConfigFileInfo {
            path: file_path.display().to_string(),
            section,
            exists,
            valid,
            error,
        });
    }

    let output = ConfigValidateOutput {
        files: results,
        all_valid,
    };

    let title = if all_valid {
        "Validation Passed"
    } else {
        "Validation Failed"
    };

    Ok(CommandResult::table(output).with_title(title))
}

/// Print the JSON schema for the `Profile` config type.
///
/// The schema is rendered directly (skip-render) so machine consumers
/// receive a clean, unwrapped JSON document on stdout.
fn print_profile_schema() -> Result<CommandResult<ConfigValidateOutput>> {
    let schema = schemars::schema_for!(Profile);
    let json = serde_json::to_string_pretty(&schema)
        .map_err(|e| anyhow!("failed to serialize Profile JSON schema: {e}"))?;
    CliService::output(&json);

    let output = ConfigValidateOutput {
        files: Vec::new(),
        all_valid: true,
    };
    Ok(CommandResult::table(output).with_skip_render())
}

/// Validate a single `.yaml`/`.yml` file as a complete `Profile` document.
///
/// Returns `Err` (non-zero exit) when the file fails to deserialize so
/// callers and CI can rely on the exit code. The serde error already
/// names the offending field or rejected enum value — it is surfaced
/// verbatim so the failure is actionable.
fn validate_profile_file(path: &std::path::Path) -> Result<CommandResult<ConfigValidateOutput>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("failed to read profile {}: {e}", path.display()))?;

    match Profile::from_yaml(&content, path) {
        Ok(profile) => {
            let output = ConfigValidateOutput {
                files: vec![ConfigFileInfo {
                    path: path.display().to_string(),
                    section: "profile".to_string(),
                    exists: true,
                    valid: true,
                    error: None,
                }],
                all_valid: true,
            };
            let title = format!("Profile '{}' is valid", profile.name);
            Ok(CommandResult::table(output).with_title(title))
        },
        Err(e) => Err(anyhow!(
            "invalid profile {}: {e}\nThe error above names the offending field or value — fix it \
             and re-run.",
            path.display()
        )),
    }
}

fn is_yaml_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("yaml" | "yml")
    )
}

fn validate_file(path: &std::path::Path, _strict: bool) -> Result<()> {
    let _content = read_yaml_file(path)?;
    Ok(())
}

fn detect_section(path: &std::path::Path) -> String {
    let path_str = path.display().to_string();

    if path_str.contains("/ai/") {
        "ai".to_string()
    } else if path_str.contains("/content/") {
        "content".to_string()
    } else if path_str.contains("/web/") {
        "web".to_string()
    } else if path_str.contains("/scheduler/") {
        "scheduler".to_string()
    } else if path_str.contains("/agents/") {
        "agents".to_string()
    } else if path_str.contains("/mcp/") {
        "mcp".to_string()
    } else if path_str.contains("/skills/") {
        "skills".to_string()
    } else if path_str.contains("profile.yaml") {
        "profile".to_string()
    } else if path_str.contains("/config/config.yaml") {
        "services".to_string()
    } else {
        "unknown".to_string()
    }
}
