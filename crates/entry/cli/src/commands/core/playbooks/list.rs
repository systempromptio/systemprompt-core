use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use super::types::{ListOrDetail, PlaybookDetailOutput, PlaybookListOutput, PlaybookSummary};
use crate::shared::CommandResult;
use systemprompt_agent::repository::content::PlaybookRepository;
use systemprompt_database::{Database, DatabaseProvider};
use systemprompt_identifiers::PlaybookId;
use systemprompt_logging::CliService;
use systemprompt_models::SecretsBootstrap;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(help = "Show details for a specific playbook")]
    pub playbook_id: Option<String>,

    #[arg(long, help = "Filter by category")]
    pub category: Option<String>,
}

pub async fn execute(args: ListArgs) -> Result<CommandResult<ListOrDetail>> {
    let db = create_db_provider().await?;
    let repo = PlaybookRepository::new(db);

    if let Some(playbook_id) = args.playbook_id {
        let playbook = repo
            .get_by_playbook_id(&PlaybookId::new(&playbook_id))
            .await?
            .ok_or_else(|| anyhow::anyhow!("Playbook not found: {}", playbook_id))?;

        let instructions_preview = if playbook.instructions.len() > 200 {
            format!("{}...", &playbook.instructions[..200])
        } else {
            playbook.instructions.clone()
        };

        let output = PlaybookDetailOutput {
            playbook_id: playbook.playbook_id.to_string(),
            name: playbook.name,
            description: playbook.description,
            category: playbook.category,
            domain: playbook.domain,
            enabled: playbook.enabled,
            tags: playbook.tags,
            file_path: playbook.file_path,
            instructions_preview,
        };

        return Ok(CommandResult::text(ListOrDetail::Detail(output)).with_title("Playbook Details"));
    }

    let playbooks = if let Some(category) = args.category {
        repo.list_by_category(&category).await?
    } else {
        repo.list_all().await?
    };

    CliService::section("Playbooks");

    if playbooks.is_empty() {
        CliService::info("No playbooks found");
    } else {
        for playbook in &playbooks {
            let status = if playbook.enabled { "✓" } else { "○" };
            CliService::info(&format!(
                "{} {} ({}/{}) - {}",
                status, playbook.playbook_id, playbook.category, playbook.domain, playbook.name
            ));
        }
        CliService::info(&format!("\n{} playbook(s) total", playbooks.len()));
    }

    let summaries: Vec<PlaybookSummary> = playbooks
        .into_iter()
        .map(|p| PlaybookSummary {
            playbook_id: p.playbook_id.to_string(),
            name: p.name,
            category: p.category,
            domain: p.domain,
            enabled: p.enabled,
            tags: p.tags,
            file_path: p.file_path,
        })
        .collect();

    Ok(CommandResult::text(ListOrDetail::List(PlaybookListOutput {
        playbooks: summaries,
    }))
    .with_title("Playbooks"))
}

async fn create_db_provider() -> Result<Arc<dyn DatabaseProvider>> {
    let url = SecretsBootstrap::database_url()
        .context("Database URL not configured")?
        .to_string();

    let database = Database::from_config("postgres", &url)
        .await
        .context("Failed to connect to database")?;

    Ok(Arc::new(database))
}
