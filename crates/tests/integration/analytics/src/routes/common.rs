use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use http_body_util::BodyExt;
use systemprompt_analytics_entry::extension::AnalyticsHttpExtension;
use systemprompt_database::DbPool;
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::router::{ApiBase, BasePath, BaseRef, WebBase};
use systemprompt_extension::{Contributions, Extension, HandleId};
use systemprompt_identifiers::{AgentName, ContextId, ExtensionId, SessionId, TraceId, UserId};
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context, test_db_pool};

pub async fn setup_ctx() -> Result<(DbPool, Arc<AppContext>)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&bootstrap.database_url).await;
    Ok((pool, ctx))
}

fn member_router(ctx: &AppContext, base: BaseRef, handles: &[HandleId]) -> Router {
    let mut sink = ContributionSink::default();
    let mut contributions = Contributions::__new(ExtensionId::new("analytics_http"), &mut sink);
    AnalyticsHttpExtension
        .contribute(&mut contributions)
        .expect("the analytics_http member contributes");
    let resolved = ctx.kernel().route_scoped(handles);
    sink.routes()
        .iter()
        .filter(|set| set.base() == base)
        .flat_map(|set| {
            set.materialize(&resolved)
                .expect("the fixture kernel resolves the analytics route state")
        })
        .fold(Router::new(), Router::merge)
}

pub fn analytics_router(ctx: &AppContext) -> Router {
    member_router(
        ctx,
        BaseRef::Path(BasePath::Api(ApiBase::Analytics)),
        &[HandleId::AnalyticsBroadcaster],
    )
}

pub fn engagement_router(ctx: &AppContext) -> Router {
    Router::new().nest(
        ApiPaths::TRACK_ENGAGEMENT,
        member_router(ctx, BaseRef::Path(BasePath::Web(WebBase::TrackEngagement)), &[]),
    )
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
