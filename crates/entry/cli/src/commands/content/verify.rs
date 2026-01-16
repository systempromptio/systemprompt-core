use super::types::VerifyOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_content::ContentRepository;
use systemprompt_identifiers::{ContentId, SourceId};
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct VerifyArgs {
    #[arg(help = "Content slug or ID")]
    pub identifier: String,

    #[arg(long, help = "Source ID (required when using slug)")]
    pub source: Option<String>,

    #[arg(long, help = "Web dist directory to check for prerendered HTML")]
    pub web_dist: Option<PathBuf>,

    #[arg(long, help = "Base URL to check HTTP status")]
    pub base_url: Option<String>,

    #[arg(long, help = "URL pattern (e.g., /{source}/{slug})")]
    pub url_pattern: Option<String>,
}

pub async fn execute(args: VerifyArgs, _config: &CliConfig) -> Result<CommandResult<VerifyOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ContentRepository::new(ctx.db_pool())?;

    let content = if uuid::Uuid::parse_str(&args.identifier).is_ok() {
        let id = ContentId::new(args.identifier.clone());
        repo.get_by_id(&id)
            .await?
            .ok_or_else(|| anyhow!("Content not found: {}", args.identifier))?
    } else {
        let source_id = args
            .source
            .as_ref()
            .ok_or_else(|| anyhow!("--source required when using slug"))?;
        let source = SourceId::new(source_id.clone());
        repo.get_by_source_and_slug(&source, &args.identifier)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Content not found: {} in source {}",
                    args.identifier,
                    source_id
                )
            })?
    };

    let url_pattern = args
        .url_pattern
        .unwrap_or_else(|| format!("/{}/{{}}", content.source_id.as_str()));
    let expected_url = url_pattern.replace("{slug}", &content.slug);
    let expected_url = expected_url.replace("{}", &content.slug);

    let (prerendered, prerender_path) = args.web_dist.as_ref().map_or((None, None), |dist_dir| {
        let html_path = dist_dir.join(format!(
            "{}/index.html",
            expected_url.trim_start_matches('/')
        ));
        let exists = html_path.exists();
        (Some(exists), Some(html_path.to_string_lossy().to_string()))
    });

    let http_status = if let Some(base_url) = &args.base_url {
        let full_url = format!("{}{}", base_url.trim_end_matches('/'), expected_url);
        match reqwest::Client::new()
            .head(&full_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => Some(response.status().as_u16()),
            Err(_) => None,
        }
    } else {
        None
    };

    let output = VerifyOutput {
        content_id: content.id.clone(),
        slug: content.slug,
        source_id: content.source_id.clone(),
        in_database: true,
        is_public: content.public,
        url: expected_url,
        prerendered,
        prerender_path,
        http_status,
        template: Some(content.kind),
        last_updated: content.updated_at,
    };

    Ok(CommandResult::card(output).with_title("Content Verification"))
}
