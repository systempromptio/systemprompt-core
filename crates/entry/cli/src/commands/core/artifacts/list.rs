use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::content::artifact::ArtifactRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContextId;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use tabled::{Table, Tabled};

use super::types::{ArtifactListOutput, ArtifactSummary};
use crate::cli_settings::CliConfig;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, short = 'c', help = "Filter by context ID")]
    pub context_id: Option<String>,

    #[arg(
        long,
        short = 'l',
        default_value = "20",
        help = "Maximum artifacts to show"
    )]
    pub limit: i32,
}

#[derive(Tabled)]
struct ArtifactRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    artifact_type: String,
    #[tabled(rename = "Tool")]
    tool_name: String,
    #[tabled(rename = "Created")]
    created_at: String,
}

pub async fn execute(
    args: ListArgs,
    config: &CliConfig,
) -> Result<CommandResult<ArtifactListOutput>> {
    let session_ctx = get_or_create_session(config).await?;
    let ctx = AppContext::new().await?;
    execute_with_pool(args, &session_ctx.session.user_id, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    user_id: &systemprompt_identifiers::UserId,
    pool: &DbPool,
    config: &CliConfig,
) -> Result<CommandResult<ArtifactListOutput>> {
    let repo = ArtifactRepository::new(Clone::clone(pool));

    let artifacts = if let Some(ref ctx_id) = args.context_id {
        let context_id = ContextId::new(ctx_id);
        repo.get_artifacts_by_context(&context_id)
            .await
            .context("Failed to fetch artifacts by context")?
    } else {
        repo.get_artifacts_by_user_id(user_id, Some(args.limit))
            .await
            .context("Failed to fetch artifacts")?
    };

    let summaries: Vec<ArtifactSummary> = artifacts
        .iter()
        .take(args.limit as usize)
        .map(|a| ArtifactSummary {
            id: a.id.as_str().to_string(),
            name: a.name.clone(),
            artifact_type: a.metadata.artifact_type.clone(),
            tool_name: a.metadata.tool_name.clone(),
            task_id: a.metadata.task_id.as_str().to_string(),
            created_at: chrono::DateTime::parse_from_rfc3339(&a.metadata.created_at)
                .map_or_else(|_| chrono::Utc::now(), |dt| dt.with_timezone(&chrono::Utc)),
        })
        .collect();

    let total = summaries.len();

    let output = ArtifactListOutput {
        artifacts: summaries.clone(),
        total,
        context_id: args.context_id.clone(),
    };

    if !config.is_json_output() {
        CliService::section("Artifacts");

        if summaries.is_empty() {
            CliService::info("No artifacts found");
        } else {
            let rows: Vec<ArtifactRow> = summaries
                .iter()
                .map(|a| ArtifactRow {
                    id: truncate_id(&a.id, 12),
                    name: a.name.clone().unwrap_or_else(|| "-".to_string()),
                    artifact_type: a.artifact_type.clone(),
                    tool_name: a.tool_name.clone().unwrap_or_else(|| "-".to_string()),
                    created_at: a.created_at.format("%Y-%m-%d %H:%M").to_string(),
                })
                .collect();

            let table = Table::new(rows).to_string();
            CliService::output(&table);

            CliService::info(&format!("Showing {} artifact(s)", total));
        }
    }

    Ok(CommandResult::table(output)
        .with_title("Artifacts")
        .with_columns(vec![
            "id".to_string(),
            "name".to_string(),
            "artifact_type".to_string(),
            "tool_name".to_string(),
            "created_at".to_string(),
        ]))
}

fn truncate_id(id: &str, max_len: usize) -> String {
    if id.len() <= max_len {
        id.to_string()
    } else {
        format!("{}...", &id[..max_len])
    }
}
