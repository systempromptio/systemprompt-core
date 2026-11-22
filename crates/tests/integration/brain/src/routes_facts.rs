//! `/api/v1/brain` fact reads, search and person-entered facts, driven
//! through the `brain-http` user route set over a brain the users and brain
//! members compose on the test database.

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::response::Response;
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_brain_entry::http::user_routes;
use systemprompt_database::DbPool;
use systemprompt_extension::DynNotificationSink;
use systemprompt_extension::router::RouteSet;
use systemprompt_identifiers::{AgentName, BrainFactId, ContextId, SessionId, TraceId, UserId};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::RequestContext;
use systemprompt_models::brain::FactStatus;
use systemprompt_runtime::Kernel;
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, fixture_brain_kernel, seed_brain_fact,
    seed_bridge_credential, test_app_context, test_db_pool,
};
use tower::ServiceExt;
use uuid::Uuid;

pub(super) struct BrainFixture {
    pub(super) pool: DbPool,
    pub(super) kernel: Kernel,
}

pub(super) async fn brain_fixture() -> Result<BrainFixture> {
    let boot = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&boot.database_url).await;
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let kernel = fixture_brain_kernel(
        &pool,
        Some(ctx.documents()),
        notifications,
        BrainProfile {
            enabled: true,
            ..BrainProfile::default()
        },
    );
    Ok(BrainFixture { pool, kernel })
}

pub(super) fn request_context(user: &UserId) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("brain-routes").expect("agent name"),
        systemprompt_identifiers::Actor::user(user.clone()),
    )
    .with_browser_session()
}

pub(super) fn mounted(kernel: &Kernel, set: &RouteSet, user: &UserId) -> Result<Router> {
    Ok(set
        .materialize(&kernel.resolved())?
        .into_iter()
        .fold(Router::new(), Router::merge)
        .layer(Extension(request_context(user))))
}

pub(super) async fn read_json(response: Response) -> Result<Value> {
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

pub(super) async fn call(
    kernel: &Kernel,
    user: &UserId,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> Result<(StatusCode, Value)> {
    let app = mounted(kernel, &user_routes(), user)?;
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    let request = match body {
        Some(body) => builder.body(Body::from(body.to_string()))?,
        None => builder.body(Body::empty())?,
    };
    let response = app.oneshot(request).await?;
    let status = response.status();
    Ok((status, read_json(response).await?))
}

pub(super) async fn seed_personal_fact(
    pool: &DbPool,
    owner: &UserId,
    status: FactStatus,
) -> Result<BrainFactId> {
    let subject = format!("service:svc-{}", Uuid::new_v4().simple());
    seed_brain_fact(pool, owner, status, &subject).await
}

pub(super) async fn owner(pool: &DbPool, base: &str) -> Result<AuthedFixture> {
    seed_bridge_credential(pool, base).await
}

#[tokio::test]
async fn the_owner_reads_a_personal_fact_its_timeline_and_why() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-read").await?;
    let id = seed_personal_fact(&fx.pool, &me.user_id, FactStatus::Confirmed).await?;

    let (status, fact) = call(
        &fx.kernel,
        &me.user_id,
        Method::GET,
        &format!("/facts/{id}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{fact}");
    assert_eq!(fact["id"], json!(id.as_str()));
    assert_eq!(fact["statement"], json!("Billing owns the deploy gate."));

    let (status, timeline) = call(
        &fx.kernel,
        &me.user_id,
        Method::GET,
        &format!("/facts/{id}/timeline"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{timeline}");
    let revisions = timeline.as_array().context("timeline array")?;
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0]["revision"], json!(1));

    let uri = format!("/facts/{id}/why?project=core&client=claude-code");
    let (status, why) = call(&fx.kernel, &me.user_id, Method::GET, &uri, None).await?;
    assert_eq!(status, StatusCode::OK, "{why}");
    assert_eq!(why["fact"]["id"], json!(id.as_str()));
    assert!(why["terms"].is_object(), "{why}");
    Ok(())
}

#[tokio::test]
async fn a_stranger_reads_another_users_personal_fact_as_not_found() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-hidden-owner").await?;
    let other = owner(&fx.pool, "brain-hidden-other").await?;
    let id = seed_personal_fact(&fx.pool, &me.user_id, FactStatus::Confirmed).await?;

    let (status, _) = call(
        &fx.kernel,
        &other.user_id,
        Method::GET,
        &format!("/facts/{id}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &fx.kernel,
        &other.user_id,
        Method::GET,
        &format!("/facts/{id}/timeline"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, listed) = call(
        &fx.kernel,
        &other.user_id,
        Method::GET,
        "/facts?kind=decision&limit=500",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = listed["facts"]
        .as_array()
        .context("search array")?
        .iter()
        .filter_map(|fact| fact["id"].as_str())
        .collect();
    assert!(!ids.contains(&id.as_str()), "{listed}");
    Ok(())
}

#[tokio::test]
async fn search_and_subject_filter_to_the_callers_visible_facts() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-search").await?;
    let subject_kind = "service";
    let subject_id = format!("svc-{}", Uuid::new_v4().simple());
    let id = seed_brain_fact(
        &fx.pool,
        &me.user_id,
        FactStatus::Confirmed,
        &format!("{subject_kind}:{subject_id}"),
    )
    .await?;

    let uri = format!(
        "/facts?kind=decision&subject={subject_kind}:{subject_id}&status=confirmed,proposed&limit=0"
    );
    let (status, listed) = call(&fx.kernel, &me.user_id, Method::GET, &uri, None).await?;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(
        listed["facts"].as_array().map(Vec::len),
        Some(1),
        "{listed}"
    );

    let uri = format!("/subjects/{subject_kind}/{subject_id}");
    let (status, subject) = call(&fx.kernel, &me.user_id, Method::GET, &uri, None).await?;
    assert_eq!(status, StatusCode::OK, "{subject}");
    assert_eq!(subject["facts"][0]["id"], json!(id.as_str()));

    let (status, _) = call(
        &fx.kernel,
        &me.user_id,
        Method::GET,
        "/facts?status=bogus",
        None,
    )
    .await?;
    assert!(status.is_client_error(), "{status}");
    let (status, _) = call(
        &fx.kernel,
        &me.user_id,
        Method::GET,
        "/facts?cursor=x",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, refused) = call(
        &fx.kernel,
        &me.user_id,
        Method::GET,
        "/facts?after=not-a-cursor",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert_eq!(refused["error_key"], json!("invalid_cursor"));
    let (status, page) = call(&fx.kernel, &me.user_id, Method::GET, "/facts?limit=1", None).await?;
    assert_eq!(status, StatusCode::OK, "{page}");
    let token = page["next"]
        .as_str()
        .context("a full page carries a cursor")?;
    let other = owner(&fx.pool, "brain-search-other").await?;
    let (status, foreign) = call(
        &fx.kernel,
        &other.user_id,
        Method::GET,
        &format!("/facts?after={token}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{foreign}");
    assert_eq!(foreign["error_key"], json!("invalid_cursor"));
    Ok(())
}

#[tokio::test]
async fn entering_an_undeclared_kind_is_refused_and_a_missing_fact_is_404() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-enter").await?;
    let body = json!({
        "kind": "never-declared",
        "subject": "service:billing",
        "fields": {"owner": "billing"},
        "scope": {"level": "user", "key": me.user_id.as_str()},
    });
    let (status, refused) =
        call(&fx.kernel, &me.user_id, Method::POST, "/facts", Some(body)).await?;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");

    let missing = BrainFactId::generate();
    let (status, _) = call(
        &fx.kernel,
        &me.user_id,
        Method::GET,
        &format!("/facts/{missing}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}
