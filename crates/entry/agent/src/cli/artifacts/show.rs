//! `core artifacts show` command rendering artifact parts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::models::a2a::Artifact;
use systemprompt_agent::repository::OwnedArtifact;
use systemprompt_agent::repository::content::artifact::ArtifactRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ArtifactId, UserId};
use systemprompt_models::a2a::PartContent;

use super::types::{ArtifactPartOutput, ArtifactSummary};
use systemprompt_database::pool_from_handle;
use systemprompt_extension::cli::{CliConfig, CommandHost, CommandOutput};

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(
        value_name = "ARTIFACT_ID",
        help = "Artifact ID (full or partial prefix)"
    )]
    pub artifact: String,

    #[arg(long, help = "Show full content without truncation")]
    pub full: bool,
}

pub(super) async fn execute(args: ShowArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let caller = host.caller_session().await?;
    execute_with_pool(
        args,
        caller.user_id(),
        &pool_from_handle(host.db().await?)?,
        host.output(),
    )
    .await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    user_id: &UserId,
    pool: &DbPool,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ArtifactRepository::new(pool);

    let owned = resolve_artifact(&args.artifact, user_id, &repo).await?;

    let artifact = repo
        .find_owned_artifact(&owned)
        .await
        .context("Failed to fetch artifact")?;

    let parts: Vec<ArtifactPartOutput> = artifact
        .parts
        .iter()
        .map(|p| match &p.content {
            PartContent::Text(text) => ArtifactPartOutput {
                kind: "text".to_owned(),
                text: Some(text.clone()),
                data: None,
            },
            PartContent::Data(data) => ArtifactPartOutput {
                kind: "data".to_owned(),
                text: None,
                data: Some(data.clone()),
            },
            PartContent::Raw(raw) => ArtifactPartOutput {
                kind: "file".to_owned(),
                text: p.filename.clone(),
                data: Some(serde_json::json!({
                    "mediaType": p.media_type,
                    "raw": format!("[{} bytes]", raw.len()),
                })),
            },
            PartContent::Url(url) => ArtifactPartOutput {
                kind: "file".to_owned(),
                text: p.filename.clone(),
                data: Some(serde_json::json!({
                    "mediaType": p.media_type,
                    "url": url,
                })),
            },
        })
        .collect();

    let output = ArtifactSummary {
        artifact_id: artifact.id.clone(),
        name: artifact.title.clone(),
        artifact_type: artifact.metadata.artifact_type.clone(),
        tool_name: artifact.metadata.tool_name.clone(),
        task_id: artifact.metadata.task_id.clone(),
        created_at: chrono::DateTime::parse_from_rfc3339(&artifact.metadata.created_at)
            .map_or_else(|_| chrono::Utc::now(), |dt| dt.with_timezone(&chrono::Utc)),
    };

    if config.is_json_output() {
        return Ok(CommandOutput::card_value("Artifact Details", &output));
    }
    Ok(CommandOutput::text_titled(
        "Artifact Details",
        render_artifact(&artifact, &parts, args.full),
    ))
}

fn render_artifact(artifact: &Artifact, parts: &[ArtifactPartOutput], full: bool) -> String {
    let mut lines = vec![format!("ID: {}", artifact.id.as_str())];
    if let Some(name) = &artifact.title {
        lines.push(format!("Name: {name}"));
    }
    if let Some(desc) = &artifact.description {
        lines.push(format!("Description: {desc}"));
    }
    lines.push(format!("Type: {}", artifact.metadata.artifact_type));
    if let Some(tool) = &artifact.metadata.tool_name {
        lines.push(format!("Tool: {tool}"));
    }
    if let Some(source) = &artifact.metadata.source {
        lines.push(format!("Source: {source}"));
    }
    lines.push(format!("Task ID: {}", artifact.metadata.task_id.as_str()));
    lines.push(format!(
        "Context ID: {}",
        artifact.metadata.context_id.as_str()
    ));
    if let Some(skill_name) = &artifact.metadata.skill_name {
        lines.push(format!("Skill: {skill_name}"));
    }
    if let Some(mcp_id) = &artifact.metadata.mcp_execution_id {
        lines.push(format!("MCP Execution: {}", mcp_id.as_str()));
    }
    if let Some(fingerprint) = &artifact.metadata.fingerprint {
        lines.push(format!("Fingerprint: {fingerprint}"));
    }
    lines.push(format!("Created: {}", artifact.metadata.created_at));
    lines.push(String::new());
    lines.push("Parts".to_owned());
    for (index, part) in parts.iter().enumerate() {
        render_part(&mut lines, index, part, full);
    }
    lines.join("\n")
}

fn render_part(lines: &mut Vec<String>, index: usize, part: &ArtifactPartOutput, full: bool) {
    lines.push(format!("Part {} [{}]:", index + 1, part.kind));

    if let Some(ref text) = part.text {
        lines.extend(indented(&truncated(text.clone(), full, 500)));
    }

    if let Some(ref data) = part.data {
        let formatted = serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string());
        lines.extend(indented(&truncated(formatted, full, 1000)));
    }
}

fn truncated(text: String, full: bool, limit: usize) -> String {
    if full || text.len() <= limit {
        return text;
    }
    let cut = text.floor_char_boundary(limit);
    format!(
        "{}...\n[Truncated - use --full for complete content]",
        &text[..cut]
    )
}

fn indented(text: &str) -> Vec<String> {
    text.lines().map(|line| format!("  {line}")).collect()
}

async fn resolve_artifact(
    input: &str,
    user_id: &UserId,
    repo: &ArtifactRepository,
) -> Result<OwnedArtifact> {
    match repo.owned_artifact(&ArtifactId::new(input), user_id).await {
        Ok(owned) => return Ok(owned),
        Err(e) if e.is_not_found() => {},
        Err(e) => return Err(e).context("Failed to fetch artifact"),
    }

    let all_artifacts = repo.get_artifacts_by_user_id(user_id, Some(100)).await?;
    let matches: Vec<_> = all_artifacts
        .iter()
        .filter(|a| a.id.as_str().starts_with(input))
        .collect();

    match matches.len() {
        0 => Err(anyhow::anyhow!("No artifact found matching: {}", input)),
        1 => Ok(repo.owned_artifact(&matches[0].id, user_id).await?),
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
