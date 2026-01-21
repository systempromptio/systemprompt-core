use anyhow::Result;
use systemprompt_database::DbPool;

use crate::prerender::content::process_all_sources;
use crate::prerender::context::load_prerender_context;
use crate::prerender::homepage::prerender_homepage as render_homepage;

pub async fn prerender_content(db_pool: DbPool) -> Result<()> {
    let ctx = load_prerender_context(db_pool).await?;
    let total_rendered = process_all_sources(&ctx).await?;
    tracing::info!(items_rendered = total_rendered, "Prerendering completed");
    Ok(())
}

pub async fn prerender_homepage(db_pool: DbPool) -> Result<()> {
    let ctx = load_prerender_context(db_pool).await?;
    render_homepage(&ctx).await
}
