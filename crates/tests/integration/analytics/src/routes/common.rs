use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use http_body_util::BodyExt;
use systemprompt_analytics_entry::http::{
    AnalyticsHttpState, ContentResolver, analytics, engagement,
};
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionContext;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::{ContentRouting, RequestContext};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context, test_db_pool};
use systemprompt_traits::DynContentCatalogStats;

pub async fn setup_ctx() -> Result<(DbPool, Arc<AppContext>)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&pool, &bootstrap.database_url);
    Ok((pool, ctx))
}

fn state(ctx: &AppContext, routing: Option<Arc<dyn ContentRouting>>) -> AnalyticsHttpState {
    let extension_state = ctx.extension_state();
    let catalog = extension_state
        .get::<DynContentCatalogStats>()
        .expect("the content extension provides the catalog");
    AnalyticsHttpState::new(
        Arc::clone(ctx.analytics_repositories()),
        Arc::clone(&ctx.event_broadcasters().analytics),
        ContentResolver::new(Arc::clone(catalog), routing),
    )
}

pub fn analytics_router(ctx: &AppContext) -> Router {
    analytics_router_with_routing(ctx, None)
}

pub fn analytics_router_with_routing(
    ctx: &AppContext,
    routing: Option<Arc<dyn ContentRouting>>,
) -> Router {
    analytics::router(state(ctx, routing))
}

pub fn engagement_router(ctx: &AppContext) -> Router {
    engagement_router_with_routing(ctx, None)
}

pub fn engagement_router_with_routing(
    ctx: &AppContext,
    routing: Option<Arc<dyn ContentRouting>>,
) -> Router {
    Router::new().nest(
        ApiPaths::TRACK_ENGAGEMENT,
        engagement::router(state(ctx, routing)),
    )
}

pub struct SlugRouter {
    pub page_url: String,
    pub slug: String,
}

impl ContentRouting for SlugRouter {
    fn is_html_page(&self, _path: &str) -> bool {
        true
    }

    fn determine_source(&self, _path: &str) -> String {
        "test".to_owned()
    }

    fn resolve_slug(&self, path: &str) -> Option<String> {
        (path == self.page_url).then(|| self.slug.clone())
    }
}

pub async fn seed_content(db: &DbPool) -> Result<(String, String)> {
    let uniq = uuid::Uuid::new_v4().to_string();
    let slug = format!("post-{uniq}");
    let source = format!("src-{uniq}");
    let id = format!("mc-{uniq}");
    sqlx::query(
        "INSERT INTO markdown_content \
         (id, slug, title, description, body, author, published_at, keywords, source_id, \
         version_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, $8, $9)",
    )
    .bind(&id)
    .bind(&slug)
    .bind("Title")
    .bind("Desc")
    .bind("# body")
    .bind("Author")
    .bind("k1, k2")
    .bind(&source)
    .bind(format!("hash-{uniq}"))
    .execute(db.pool().as_ref())
    .await?;
    let page_url = format!("https://example.com/{slug}");
    Ok((slug, page_url))
}

pub fn request_context(user: &str) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("test-agent").expect("valid AgentName"),
        systemprompt_identifiers::Actor::user(UserId::new(user)),
    )
}

pub async fn body_to_string(resp: Response<Body>) -> Result<(http::StatusCode, String)> {
    let status = resp.status();
    let body = resp.into_body().collect().await?.to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

pub fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request build")
}

pub fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request build")
}
