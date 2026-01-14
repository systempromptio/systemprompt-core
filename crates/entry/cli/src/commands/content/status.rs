use super::types::{ContentStatusRow, StatusOutput};
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_content::ContentRepository;
use systemprompt_identifiers::SourceId;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(long, help = "Filter by source ID")]
    pub source: String,

    #[arg(long, help = "Web dist directory to check for prerendered HTML")]
    pub web_dist: Option<PathBuf>,

    #[arg(long, help = "URL pattern (e.g., /{source}/{slug})")]
    pub url_pattern: Option<String>,

    #[arg(long, default_value = "50")]
    pub limit: i64,
}

pub async fn execute(
    args: StatusArgs,
    _config: &CliConfig,
) -> Result<CommandResult<StatusOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ContentRepository::new(ctx.db_pool())?;

    let source = SourceId::new(args.source.clone());
    let contents = repo.list_by_source(&source).await?;

    let url_pattern = args
        .url_pattern
        .unwrap_or_else(|| format!("/{}/{{}}", args.source));

    let mut items = Vec::with_capacity(contents.len().min(args.limit as usize));
    let mut healthy = 0i64;
    let mut issues = 0i64;

    for content in contents.into_iter().take(args.limit as usize) {
        let expected_url = url_pattern.replace("{slug}", &content.slug);
        let expected_url = expected_url.replace("{}", &content.slug);

        let prerendered = args.web_dist.as_ref().map(|dist_dir| {
            let html_path =
                dist_dir.join(format!("{}/index.html", expected_url.trim_start_matches('/')));
            html_path.exists()
        });

        let is_healthy = content.public && prerendered.unwrap_or(true);
        if is_healthy {
            healthy += 1;
        } else {
            issues += 1;
        }

        items.push(ContentStatusRow {
            slug: content.slug,
            title: content.title,
            in_database: true,
            is_public: content.public,
            prerendered,
            http_status: None,
            last_updated: content.updated_at,
        });
    }

    let total = items.len() as i64;

    let output = StatusOutput {
        items,
        source_id: args.source,
        total,
        healthy,
        issues,
    };

    Ok(CommandResult::table(output)
        .with_title("Content Status")
        .with_columns(vec![
            "slug".to_string(),
            "title".to_string(),
            "is_public".to_string(),
            "prerendered".to_string(),
            "last_updated".to_string(),
        ]))
}
