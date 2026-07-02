use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::content::artifact::ArtifactRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContextId;
use systemprompt_logging::CliService;

use super::types::{ArtifactListOutput, ArtifactSummary};
use crate::cli_settings::CliConfig;
use crate::context::CommandContext;
use crate::presentation::tables::artifact_list_table;
use crate::session::get_or_create_session;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long = "context-id", short = 'c', help = "Filter by context ID")]
    pub context: Option<String>,

    #[arg(
        long,
        short = 'l',
        default_value = "20",
        help = "Maximum artifacts to show"
    )]
    pub limit: i32,
}

pub(super) async fn execute(args: ListArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let session_ctx = get_or_create_session(ctx).await?;
    let pool = ctx.db_pool().await?;
    execute_with_pool(args, &session_ctx.session.user_id, &pool, &ctx.cli).await
}

pub(super) async fn execute_with_pool(
    args: ListArgs,
    user_id: &systemprompt_identifiers::UserId,
    pool: &DbPool,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ArtifactRepository::new(pool)?;

    let artifacts = if let Some(ref ctx_id) = args.context {
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
            artifact_id: a.id.clone(),
            name: a.title.clone(),
            artifact_type: a.metadata.artifact_type.clone(),
            tool_name: a.metadata.tool_name.clone(),
            task_id: a.metadata.task_id.clone(),
            created_at: chrono::DateTime::parse_from_rfc3339(&a.metadata.created_at)
                .map_or_else(|_| chrono::Utc::now(), |dt| dt.with_timezone(&chrono::Utc)),
        })
        .collect();

    let total = summaries.len();

    let output = ArtifactListOutput {
        artifacts: summaries.clone(),
        total,
        context: args.context.clone(),
    };

    if !config.is_json_output() {
        CliService::section("Artifacts");

        if summaries.is_empty() {
            CliService::info("No artifacts found");
        } else {
            CliService::output(&artifact_list_table(&summaries));

            CliService::info(&format!("Showing {} artifact(s)", total));
        }
    }

    Ok(CommandOutput::table_of(
        vec!["id", "name", "artifact_type", "tool_name", "created_at"],
        &output.artifacts,
    )
    .with_title("Artifacts"))
}
