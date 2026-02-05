pub mod blog;
pub mod links;
pub mod query;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;

pub use blog::{get_content_handler, get_content_markdown_handler, list_content_by_source_handler};
pub use links::{
    generate_link_handler, get_campaign_performance_handler, get_content_journey_handler,
    get_link_clicks_handler, get_link_performance_handler, list_links_handler, redirect_handler,
};
pub use query::query_handler;

pub fn router(ctx: &AppContext) -> Router {
    let suffix = ctx
        .config()
        .content_negotiation
        .markdown_suffix
        .trim_start_matches('.');

    let mut router = Router::new()
        .route("/query", post(query_handler))
        .route("/{source_id}", get(list_content_by_source_handler))
        .route("/{source_id}/{slug}", get(get_content_handler))
        .route("/links/generate", post(generate_link_handler))
        .route("/links", get(list_links_handler))
        .route(
            "/links/{link_id}/performance",
            get(get_link_performance_handler),
        )
        .route("/links/{link_id}/clicks", get(get_link_clicks_handler))
        .route(
            "/links/campaigns/{campaign_id}/performance",
            get(get_campaign_performance_handler),
        )
        .route("/links/journey", get(get_content_journey_handler));

    if ctx.config().content_negotiation.enabled {
        // Use separate path segment for format since axum doesn't support {param}.suffix
        router = router.route(
            &format!("/{{source_id}}/{{slug}}/{}", suffix),
            get(get_content_markdown_handler),
        );
    }

    router.with_state(ctx.clone())
}

pub fn redirect_router(db_pool: &DbPool) -> Router {
    Router::new()
        .route("/r/{short_code}", get(redirect_handler))
        .with_state(Arc::clone(db_pool))
}
