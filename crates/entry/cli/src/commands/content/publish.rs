use super::types::PublishOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_content::{IngestionOptions, IngestionService, IngestionSource};
use systemprompt_identifiers::SourceId;
use systemprompt_runtime::AppContext;

const ALLOWED_CONTENT_TYPES: &[&str] = &["article", "paper", "guide", "tutorial"];

#[derive(Debug, Args)]
pub struct PublishArgs {
    #[arg(help = "Markdown file path")]
    pub file: PathBuf,

    #[arg(long, help = "Source ID")]
    pub source: String,

    #[arg(long, help = "Category ID")]
    pub category: Option<String>,

    #[arg(long, help = "Web dist directory to verify prerender")]
    pub web_dist: Option<PathBuf>,

    #[arg(long, help = "Base URL to verify HTTP status")]
    pub base_url: Option<String>,

    #[arg(long, help = "URL pattern (e.g., /{source}/{slug})")]
    pub url_pattern: Option<String>,

    #[arg(long, help = "Force republish even if unchanged")]
    pub force: bool,
}

pub async fn execute(
    args: PublishArgs,
    _config: &CliConfig,
) -> Result<CommandResult<PublishOutput>> {
    if !args.file.exists() {
        return Err(anyhow!("File does not exist: {}", args.file.display()));
    }

    if !args.file.is_file() {
        return Err(anyhow!("Path is not a file: {}", args.file.display()));
    }

    let ctx = AppContext::new().await?;
    let service = IngestionService::new(ctx.db_pool())?;
    let repo = systemprompt_core_content::ContentRepository::new(ctx.db_pool())?;

    let category_id = args.category.as_deref().unwrap_or("default");
    let ingestion_source = IngestionSource::new(&args.source, category_id, ALLOWED_CONTENT_TYPES);

    let parent_dir = args
        .file
        .parent()
        .ok_or_else(|| anyhow!("Cannot get parent directory"))?;

    let options = IngestionOptions::default()
        .with_override(args.force)
        .with_dry_run(false);

    let report = service
        .ingest_directory(parent_dir, &ingestion_source, options)
        .await?;

    if !report.errors.is_empty() {
        return Err(anyhow!("Ingestion failed: {}", report.errors.join(", ")));
    }

    let slug = extract_slug_from_file(&args.file)?;
    let source = SourceId::new(args.source.clone());

    let content = repo
        .get_by_source_and_slug(&source, &slug)
        .await?
        .ok_or_else(|| anyhow!("Content not found after ingestion: {}", slug))?;

    let url_pattern = args
        .url_pattern
        .unwrap_or_else(|| format!("/{}/{{}}", args.source));
    let expected_url = url_pattern.replace("{slug}", &slug).replace("{}", &slug);

    let prerendered = args.web_dist.as_ref().map(|dist_dir| {
        let html_path =
            dist_dir.join(format!("{}/index.html", expected_url.trim_start_matches('/')));
        html_path.exists()
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

    let action = if report.files_processed > 0 {
        "published"
    } else {
        "unchanged"
    };

    let success = prerendered.unwrap_or(true) && http_status.is_none_or( |s| s == 200);

    let output = PublishOutput {
        content_id: content.id.to_string(),
        slug,
        source_id: args.source,
        action: action.to_string(),
        expected_url,
        prerendered,
        http_status,
        success,
    };

    Ok(CommandResult::card(output).with_title("Content Published"))
}

fn extract_slug_from_file(path: &PathBuf) -> Result<String> {
    let content = std::fs::read_to_string(path)?;
    let parts: Vec<&str> = content.splitn(3, "---").collect();

    if parts.len() < 3 {
        return Err(anyhow!("Invalid frontmatter format"));
    }

    #[derive(serde::Deserialize)]
    struct FrontMatter {
        slug: String,
    }

    let fm: FrontMatter = serde_yaml::from_str(parts[1])?;
    Ok(fm.slug)
}
