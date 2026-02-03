use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::path::Path;

use super::path_helpers::{path_to_playbook_info, playbook_id_to_path, scan_all_playbooks};
use super::types::{ListOrDetail, PlaybookDetailOutput, PlaybookListOutput, PlaybookSummary};
use crate::shared::CommandResult;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(help = "Playbook ID to show details (optional)")]
    pub playbook_id: Option<String>,

    #[arg(long, help = "Filter by category")]
    pub category: Option<String>,
}

pub fn execute(args: ListArgs) -> Result<CommandResult<ListOrDetail>> {
    let playbooks_path = get_playbooks_path()?;

    if let Some(playbook_id) = args.playbook_id {
        return show_playbook_detail(&playbook_id, &playbooks_path);
    }

    let playbooks = scan_playbooks(&playbooks_path, args.category.as_deref())?;

    let output = PlaybookListOutput { playbooks };

    Ok(CommandResult::table(ListOrDetail::List(output))
        .with_title("Playbooks")
        .with_columns(vec![
            "playbook_id".to_string(),
            "name".to_string(),
            "category".to_string(),
            "domain".to_string(),
            "enabled".to_string(),
            "file_path".to_string(),
        ]))
}

fn get_playbooks_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(format!(
        "{}/playbook",
        profile.paths.services
    )))
}

fn show_playbook_detail(
    playbook_id: &str,
    playbooks_path: &Path,
) -> Result<CommandResult<ListOrDetail>> {
    let md_path = playbook_id_to_path(playbooks_path, playbook_id)?;

    if !md_path.exists() {
        return Err(anyhow!(
            "Playbook '{}' not found at {}",
            playbook_id,
            md_path.display()
        ));
    }

    let path_info = path_to_playbook_info(playbooks_path, &md_path)?;
    let parsed = parse_playbook_markdown(&md_path, &path_info.category, &path_info.domain)?;

    let instructions_preview = parsed.instructions.chars().take(200).collect::<String>()
        + if parsed.instructions.len() > 200 {
            "..."
        } else {
            ""
        };

    let output = PlaybookDetailOutput {
        playbook_id: playbook_id.to_string(),
        name: parsed.name,
        description: parsed.description,
        category: path_info.category,
        domain: path_info.domain,
        enabled: parsed.enabled,
        tags: parsed.tags,
        file_path: md_path.to_string_lossy().to_string(),
        instructions_preview,
    };

    Ok(CommandResult::card(ListOrDetail::Detail(output))
        .with_title(format!("Playbook: {}", playbook_id)))
}

fn scan_playbooks(
    playbooks_path: &Path,
    filter_category: Option<&str>,
) -> Result<Vec<PlaybookSummary>> {
    let all_playbooks = scan_all_playbooks(playbooks_path);

    let mut playbooks = Vec::new();

    for info in all_playbooks {
        if let Some(filter) = filter_category {
            if info.category != filter {
                continue;
            }
        }

        match parse_playbook_markdown(&info.file_path, &info.category, &info.domain) {
            Ok(parsed) => {
                playbooks.push(PlaybookSummary {
                    playbook_id: info.playbook_id,
                    name: parsed.name,
                    category: info.category,
                    domain: info.domain,
                    enabled: parsed.enabled,
                    tags: parsed.tags,
                    file_path: info.file_path.to_string_lossy().to_string(),
                });
            },
            Err(e) => {
                tracing::warn!(
                    path = %info.file_path.display(),
                    error = %e,
                    "Failed to parse playbook"
                );
            },
        }
    }

    Ok(playbooks)
}

struct ParsedPlaybook {
    name: String,
    description: String,
    enabled: bool,
    tags: Vec<String>,
    instructions: String,
}

fn parse_playbook_markdown(
    md_path: &Path,
    _category: &str,
    _domain: &str,
) -> Result<ParsedPlaybook> {
    let content = std::fs::read_to_string(md_path)
        .with_context(|| format!("Failed to read {}", md_path.display()))?;

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(anyhow!(
            "Invalid frontmatter format in {}",
            md_path.display()
        ));
    }

    let frontmatter: serde_yaml::Value = serde_yaml::from_str(parts[1])
        .with_context(|| format!("Invalid YAML in {}", md_path.display()))?;

    let name = frontmatter
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing title in {}", md_path.display()))?
        .to_string();

    let description = frontmatter
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let enabled = frontmatter
        .get("enabled")
        .and_then(serde_yaml::Value::as_bool)
        .unwrap_or(true);

    let tags = frontmatter
        .get("keywords")
        .or_else(|| frontmatter.get("tags"))
        .and_then(|v| v.as_sequence())
        .map_or_else(Vec::new, |seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    Ok(ParsedPlaybook {
        name,
        description,
        enabled,
        tags,
        instructions: parts[2].trim().to_string(),
    })
}
