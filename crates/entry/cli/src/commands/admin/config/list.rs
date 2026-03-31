use clap::Args;

use super::types::{ConfigFileInfo, ConfigListOutput, ConfigSection, read_yaml_file};
use crate::CliConfig;
use crate::shared::CommandResult;

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, short = 'e')]
    pub errors_only: bool,
}

pub fn execute(args: ListArgs, _config: &CliConfig) -> CommandResult<ConfigListOutput> {
    let mut files = Vec::new();
    let mut valid_count = 0;
    let mut invalid_count = 0;

    for section in ConfigSection::all() {
        let section_files = match section.all_files() {
            Ok(f) => f,
            Err(e) => {
                tracing::debug!(section = %section, error = %e, "Failed to list config files for section");
                continue;
            },
        };

        for file_path in section_files {
            let exists = file_path.exists();
            let (valid, error) = if exists {
                match read_yaml_file(&file_path) {
                    Ok(_) => (true, None),
                    Err(e) => (false, Some(e.to_string())),
                }
            } else {
                (false, Some("File not found".to_string()))
            };

            if valid {
                valid_count += 1;
            } else {
                invalid_count += 1;
            }

            if args.errors_only && valid {
                continue;
            }

            files.push(ConfigFileInfo {
                path: file_path.display().to_string(),
                section: section.to_string(),
                exists,
                valid,
                error,
            });
        }
    }

    let output = ConfigListOutput {
        total: valid_count + invalid_count,
        valid: valid_count,
        invalid: invalid_count,
        files,
    };

    CommandResult::table(output).with_title("Configuration Files")
}
