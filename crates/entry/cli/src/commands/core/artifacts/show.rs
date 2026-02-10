use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value as JsonValue;
use systemprompt_agent::repository::content::artifact::ArtifactRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ArtifactId;
use systemprompt_logging::CliService;
use systemprompt_models::a2a::Part;
use systemprompt_runtime::AppContext;

use super::types::{ArtifactDetailOutput, ArtifactPartOutput};
use crate::cli_settings::CliConfig;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Artifact ID (full or partial prefix)")]
    pub artifact_id: String,

    #[arg(long, help = "Show full content without truncation")]
    pub full: bool,
}

pub async fn execute(
    args: ShowArgs,
    config: &CliConfig,
) -> Result<CommandResult<ArtifactDetailOutput>> {
    let _session_ctx = get_or_create_session(config).await?;
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    pool: &DbPool,
    config: &CliConfig,
) -> Result<CommandResult<ArtifactDetailOutput>> {
    let repo = ArtifactRepository::new(pool)?;

    let artifact_id = resolve_artifact_id(&args.artifact_id, &repo).await?;

    let artifact = repo
        .get_artifact_by_id(&artifact_id)
        .await
        .context("Failed to fetch artifact")?
        .ok_or_else(|| anyhow::anyhow!("Artifact not found: {}", args.artifact_id))?;

    let parts: Vec<ArtifactPartOutput> = artifact
        .parts
        .iter()
        .map(|p| match p {
            Part::Text(text_part) => ArtifactPartOutput {
                kind: "text".to_string(),
                text: Some(text_part.text.clone()),
                data: None,
            },
            Part::Data(data_part) => ArtifactPartOutput {
                kind: "data".to_string(),
                text: None,
                data: Some(JsonValue::Object(data_part.data.clone())),
            },
            Part::File(file_part) => ArtifactPartOutput {
                kind: "file".to_string(),
                text: file_part.file.name.clone(),
                data: Some(serde_json::json!({
                    "mimeType": file_part.file.mime_type,
                    "bytes": format!("[{} bytes]", file_part.file.bytes.len()),
                })),
            },
        })
        .collect();

    let output = ArtifactDetailOutput {
        id: artifact.id.as_str().to_string(),
        name: artifact.name.clone(),
        description: artifact.description.clone(),
        artifact_type: artifact.metadata.artifact_type.clone(),
        tool_name: artifact.metadata.tool_name.clone(),
        source: artifact.metadata.source.clone(),
        task_id: artifact.metadata.task_id.as_str().to_string(),
        context_id: artifact.metadata.context_id.as_str().to_string(),
        skill_id: artifact.metadata.skill_id.clone(),
        skill_name: artifact.metadata.skill_name.clone(),
        mcp_execution_id: artifact.metadata.mcp_execution_id.clone(),
        fingerprint: artifact.metadata.fingerprint.clone(),
        created_at: artifact.metadata.created_at.clone(),
        parts: parts.clone(),
        rendering_hints: artifact.metadata.rendering_hints.clone(),
    };

    if !config.is_json_output() {
        CliService::section("Artifact Details");
        CliService::key_value("ID", artifact.id.as_str());

        if let Some(ref name) = artifact.name {
            CliService::key_value("Name", name);
        }

        if let Some(ref desc) = artifact.description {
            CliService::key_value("Description", desc);
        }

        CliService::key_value("Type", &artifact.metadata.artifact_type);

        if let Some(ref tool) = artifact.metadata.tool_name {
            CliService::key_value("Tool", tool);
        }

        if let Some(ref source) = artifact.metadata.source {
            CliService::key_value("Source", source);
        }

        CliService::key_value("Task ID", artifact.metadata.task_id.as_str());
        CliService::key_value("Context ID", artifact.metadata.context_id.as_str());

        if let Some(ref skill_name) = artifact.metadata.skill_name {
            CliService::key_value("Skill", skill_name);
        }

        if let Some(ref mcp_id) = artifact.metadata.mcp_execution_id {
            CliService::key_value("MCP Execution", mcp_id);
        }

        if let Some(ref fingerprint) = artifact.metadata.fingerprint {
            CliService::key_value("Fingerprint", fingerprint);
        }

        CliService::key_value("Created", &artifact.metadata.created_at);

        CliService::info("");
        CliService::section("Parts");

        for (i, part) in parts.iter().enumerate() {
            CliService::info(&format!("Part {} [{}]:", i + 1, part.kind));

            if let Some(ref text) = part.text {
                let display_text = if args.full || text.len() <= 500 {
                    text.clone()
                } else {
                    format!(
                        "{}...\n[Truncated - use --full for complete content]",
                        &text[..500]
                    )
                };

                for line in display_text.lines() {
                    CliService::info(&format!("  {}", line));
                }
            }

            if let Some(ref data) = part.data {
                let formatted =
                    serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string());

                let display_data = if args.full || formatted.len() <= 1000 {
                    formatted
                } else {
                    format!(
                        "{}...\n[Truncated - use --full for complete content]",
                        &formatted[..1000]
                    )
                };

                for line in display_data.lines() {
                    CliService::info(&format!("  {}", line));
                }
            }
        }
    }

    Ok(CommandResult::card(output).with_title("Artifact Details"))
}

async fn resolve_artifact_id(input: &str, repo: &ArtifactRepository) -> Result<ArtifactId> {
    let artifact_id = ArtifactId::new(input);
    if repo.get_artifact_by_id(&artifact_id).await?.is_some() {
        return Ok(artifact_id);
    }

    let all_artifacts = repo.get_all_artifacts(Some(100)).await?;
    let matches: Vec<_> = all_artifacts
        .iter()
        .filter(|a| a.id.as_str().starts_with(input))
        .collect();

    match matches.len() {
        0 => Err(anyhow::anyhow!("No artifact found matching: {}", input)),
        1 => Ok(matches[0].id.clone()),
        _ => {
            let ids: Vec<&str> = matches.iter().map(|a| a.id.as_str()).collect();
            Err(anyhow::anyhow!(
                "Multiple artifacts match prefix '{}': {:?}. Please be more specific.",
                input,
                ids
            ))
        },
    }
}
