use super::xml::{build_rss_xml, RssChannel, RssItem};
use anyhow::{anyhow, Context, Result};
use std::path::Path;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_models::{AppPaths, Config};
use tokio::fs;

const MAX_FEED_ITEMS: i64 = 20;

struct RssContext {
    db_pool: DbPool,
    base_url: String,
    web_dir: std::path::PathBuf,
    site_title: String,
    site_description: String,
}

pub async fn generate_feed(db_pool: DbPool) -> Result<()> {
    let ctx = load_rss_context(db_pool)?;
    let items = collect_feed_items(&ctx).await?;
    write_feed_file(&ctx, items).await
}

fn load_rss_context(db_pool: DbPool) -> Result<RssContext> {
    let global_config = Config::get()?;
    let web_dir = AppPaths::get()
        .map_err(|e| anyhow!("{}", e))?
        .web()
        .dist()
        .to_path_buf();

    Ok(RssContext {
        db_pool,
        base_url: global_config.api_external_url.clone(),
        web_dir,
        site_title: "Tying Shoelaces".to_string(),
        site_description: "Technical insights and engineering perspectives".to_string(),
    })
}

async fn collect_feed_items(ctx: &RssContext) -> Result<Vec<RssItem>> {
    let repo = ContentRepository::new(&ctx.db_pool)
        .map_err(|e| anyhow!("Failed to create content repository: {}", e))?;

    let source_id = SourceId::new("blog");
    let content_items = repo
        .list_by_source_limited(&source_id, MAX_FEED_ITEMS)
        .await
        .context("Failed to fetch blog content for RSS feed")?;

    let items = content_items
        .into_iter()
        .map(|content| {
            let link = format!("{}/blog/{}", ctx.base_url, content.slug);
            RssItem {
                title: content.title,
                link: link.clone(),
                description: content.description,
                pub_date: content.published_at,
                guid: link,
                author: Some(content.author),
            }
        })
        .collect();

    Ok(items)
}

async fn write_feed_file(ctx: &RssContext, items: Vec<RssItem>) -> Result<()> {
    let channel = RssChannel {
        title: ctx.site_title.clone(),
        link: ctx.base_url.clone(),
        description: ctx.site_description.clone(),
        items,
    };

    let xml = build_rss_xml(&channel);
    let feed_path = ctx.web_dir.join("feed.xml");

    ensure_parent_exists(&feed_path).await?;
    fs::write(&feed_path, &xml).await?;

    tracing::info!(
        path = %feed_path.display(),
        items = channel.items.len(),
        "Generated RSS feed"
    );

    Ok(())
}

async fn ensure_parent_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await?;
        }
    }
    Ok(())
}
