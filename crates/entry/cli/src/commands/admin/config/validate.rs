use anyhow::Result;
use clap::Args;

use super::types::{read_yaml_file, ConfigFileInfo, ConfigSection, ConfigValidateOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    #[arg(value_name = "PATH_OR_SECTION")]
    pub target: Option<String>,

    #[arg(long)]
    pub strict: bool,
}

pub fn execute(
    args: ValidateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ConfigValidateOutput>> {
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
