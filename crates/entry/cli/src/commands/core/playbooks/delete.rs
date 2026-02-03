use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::fs;
use std::path::Path;

use super::path_helpers::{cleanup_empty_parent_dirs, playbook_id_to_path, scan_all_playbooks};
use super::types::PlaybookDeleteOutput;
use crate::interactive::{require_confirmation, resolve_required};
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_logging::CliService;
use systemprompt_models::ProfileBootstrap;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Playbook ID to delete (format: category_domain)")]
    pub name: Option<String>,

    #[arg(long, help = "Delete all playbooks")]
    pub all: bool,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub fn execute(
    args: DeleteArgs,
    config: &CliConfig,
) -> Result<CommandResult<PlaybookDeleteOutput>> {
    let playbooks_path = get_playbooks_path()?;

    let playbooks_to_delete: Vec<String> = if args.all {
        list_all_playbooks(&playbooks_path)
    } else {
        let name = resolve_required(args.name, "name", config, || {
            prompt_playbook_selection(&playbooks_path)
        })?;

        let playbook_file = playbook_id_to_path(&playbooks_path, &name)?;

        if !playbook_file.exists() {
            return Err(anyhow!("Playbook '{}' not found", name));
        }

        vec![name]
    };

    if playbooks_to_delete.is_empty() {
        return Ok(CommandResult::text(PlaybookDeleteOutput {
            deleted: vec![],
            message: "No playbooks to delete".to_string(),
        })
        .with_title("Delete Playbook"));
    }

    let confirm_message = if args.all {
        format!("Delete ALL {} playbooks?", playbooks_to_delete.len())
    } else {
        format!("Delete playbook '{}'?", playbooks_to_delete[0])
    };

    require_confirmation(&confirm_message, args.yes, config)?;

    let mut deleted = Vec::new();

    for playbook_id in &playbooks_to_delete {
        CliService::info(&format!("Deleting playbook '{}'...", playbook_id));

        match delete_playbook(&playbooks_path, playbook_id) {
            Ok(()) => {
                CliService::success(&format!("Playbook '{}' deleted", playbook_id));
                deleted.push(playbook_id.clone());
            },
            Err(e) => {
                CliService::error(&format!(
                    "Failed to delete playbook '{}': {}",
                    playbook_id, e
                ));
            },
        }
    }

    let message = match deleted.len() {
        0 => "No playbooks were deleted".to_string(),
        1 => format!("Playbook '{}' deleted successfully", deleted[0]),
        n => format!("{} playbook(s) deleted successfully", n),
    };

    let output = PlaybookDeleteOutput { deleted, message };
    Ok(CommandResult::text(output).with_title("Delete Playbook"))
}

fn get_playbooks_path() -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(format!(
        "{}/playbook",
        profile.paths.services
    )))
}

fn delete_playbook(playbooks_path: &Path, playbook_id: &str) -> Result<()> {
    let playbook_file = playbook_id_to_path(playbooks_path, playbook_id)?;

    if !playbook_file.exists() {
        return Err(anyhow!("Playbook '{}' not found", playbook_id));
    }

    fs::remove_file(&playbook_file).with_context(|| {
        format!(
            "Failed to remove playbook file: {}",
            playbook_file.display()
        )
    })?;

    cleanup_empty_parent_dirs(playbooks_path, &playbook_file)?;

    Ok(())
}

fn list_all_playbooks(playbooks_path: &Path) -> Vec<String> {
    scan_all_playbooks(playbooks_path)
        .into_iter()
        .map(|p| p.playbook_id)
        .collect()
}

fn prompt_playbook_selection(playbooks_path: &Path) -> Result<String> {
    let playbooks = list_all_playbooks(playbooks_path);

    if playbooks.is_empty() {
        return Err(anyhow!("No playbooks found"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select playbook to delete")
        .items(&playbooks)
        .default(0)
        .interact()
        .context("Failed to get playbook selection")?;

    Ok(playbooks[selection].clone())
}
