use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::fs;
use std::path::Path;

use super::path_helpers::{playbook_id_to_path, scan_all_playbooks};
use super::types::PlaybookEditOutput;
use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_logging::CliService;
use systemprompt_models::ProfileBootstrap;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Playbook ID to edit (format: category_domain)")]
    pub name: Option<String>,

    #[arg(
        long = "set",
        value_name = "KEY=VALUE",
        help = "Set a configuration value"
    )]
    pub set_values: Vec<String>,

    #[arg(long, help = "Enable the playbook", conflicts_with = "disable")]
    pub enable: bool,

    #[arg(long, help = "Disable the playbook", conflicts_with = "enable")]
    pub disable: bool,

    #[arg(long, help = "Update instructions")]
    pub instructions: Option<String>,

    #[arg(long, help = "File containing updated instructions")]
    pub instructions_file: Option<String>,
}

pub fn execute(args: &EditArgs, config: &CliConfig) -> Result<CommandResult<PlaybookEditOutput>> {
    let playbooks_path = get_playbooks_path()?;

    let name = resolve_required(args.name.clone(), "name", config, || {
        prompt_playbook_selection(&playbooks_path)
    })?;

    let playbook_file = playbook_id_to_path(&playbooks_path, &name)?;

    if !playbook_file.exists() {
        return Err(anyhow!(
            "Playbook '{}' not found at {}",
            name,
            playbook_file.display()
        ));
    }

    edit_playbook(&playbook_file, &name, args)
}

fn edit_playbook(
    playbook_file: &Path,
    name: &str,
    args: &EditArgs,
) -> Result<CommandResult<PlaybookEditOutput>> {
    let content = fs::read_to_string(playbook_file)
        .with_context(|| format!("Failed to read {}", playbook_file.display()))?;

    let (mut frontmatter, instructions) = parse_markdown(&content)?;

    let mut changes = Vec::new();

    if args.enable {
        frontmatter.insert(
            serde_yaml::Value::String("enabled".to_string()),
            serde_yaml::Value::Bool(true),
        );
        changes.push("enabled: true".to_string());
    }

    if args.disable {
        frontmatter.insert(
            serde_yaml::Value::String("enabled".to_string()),
            serde_yaml::Value::Bool(false),
        );
        changes.push("enabled: false".to_string());
    }

    for set_value in &args.set_values {
        let parts: Vec<&str> = set_value.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid --set format: '{}'. Expected key=value",
                set_value
            ));
        }
        apply_set_value(&mut frontmatter, parts[0], parts[1])?;
        changes.push(format!("{}: {}", parts[0], parts[1]));
    }

    let final_instructions = resolve_new_instructions(args, &instructions)?;
    if final_instructions != instructions {
        changes.push("instructions: updated".to_string());
    }

    if changes.is_empty() {
        return Err(anyhow!("No changes specified"));
    }

    CliService::info(&format!("Updating playbook '{}'...", name));

    let new_content = rebuild_markdown(&frontmatter, &final_instructions)?;
    fs::write(playbook_file, new_content)
        .with_context(|| format!("Failed to write {}", playbook_file.display()))?;

    CliService::success(&format!("Playbook '{}' updated successfully", name));

    let output = PlaybookEditOutput {
        playbook_id: name.to_string(),
        message: format!(
            "Playbook '{}' updated successfully with {} change(s)",
            name,
            changes.len()
        ),
        changes,
    };

    Ok(CommandResult::text(output).with_title(format!("Edit Playbook: {}", name)))
}

fn get_playbooks_path() -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(format!(
        "{}/playbook",
        profile.paths.services
    )))
}


fn parse_markdown(content: &str) -> Result<(serde_yaml::Mapping, String)> {
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(anyhow!("Invalid frontmatter format"));
    }

    let frontmatter: serde_yaml::Mapping =
        serde_yaml::from_str(parts[1]).context("Invalid YAML frontmatter")?;

    Ok((frontmatter, parts[2].trim().to_string()))
}

fn rebuild_markdown(frontmatter: &serde_yaml::Mapping, instructions: &str) -> Result<String> {
    let yaml = serde_yaml::to_string(frontmatter).context("Failed to serialize frontmatter")?;

    Ok(format!("---\n{}---\n\n{}\n", yaml, instructions))
}

fn apply_set_value(frontmatter: &mut serde_yaml::Mapping, key: &str, value: &str) -> Result<()> {
    match key {
        "title" | "name" => {
            frontmatter.insert(
                serde_yaml::Value::String("title".to_string()),
                serde_yaml::Value::String(value.to_string()),
            );
        },
        "description" => {
            frontmatter.insert(
                serde_yaml::Value::String("description".to_string()),
                serde_yaml::Value::String(value.to_string()),
            );
        },
        "tags" | "keywords" => {
            let tags: Vec<serde_yaml::Value> = value
                .split(',')
                .map(|s| serde_yaml::Value::String(s.trim().to_string()))
                .collect();
            frontmatter.insert(
                serde_yaml::Value::String("keywords".to_string()),
                serde_yaml::Value::Sequence(tags),
            );
        },
        "enabled" => {
            let enabled = value.parse::<bool>().map_err(|_| {
                anyhow!(
                    "Invalid boolean value for enabled: '{}'. Use true or false",
                    value
                )
            })?;
            frontmatter.insert(
                serde_yaml::Value::String("enabled".to_string()),
                serde_yaml::Value::Bool(enabled),
            );
        },
        _ => {
            return Err(anyhow!(
                "Unknown configuration key: '{}'. Supported: title, description, tags, enabled",
                key
            ));
        },
    }
    Ok(())
}

fn resolve_new_instructions(args: &EditArgs, current: &str) -> Result<String> {
    if let Some(i) = &args.instructions {
        return Ok(i.clone());
    }

    if let Some(file) = &args.instructions_file {
        let path = Path::new(file);
        return fs::read_to_string(path)
            .with_context(|| format!("Failed to read instructions file: {}", path.display()));
    }

    Ok(current.to_string())
}

fn list_all_playbooks(playbooks_path: &Path) -> Vec<String> {
    scan_all_playbooks(playbooks_path)
        .into_iter()
        .map(|p| p.playbook_id)
        .collect()
}

fn prompt_playbook_selection(playbooks_path: &Path) -> Result<String> {
    let playbooks = list_all_playbooks(playbooks_path)?;

    if playbooks.is_empty() {
        return Err(anyhow!("No playbooks found"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select playbook to edit")
        .items(&playbooks)
        .default(0)
        .interact()
        .context("Failed to get playbook selection")?;

    Ok(playbooks[selection].clone())
}
