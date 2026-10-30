//! `/api/v1/brain` fact reads, search and person-entered facts, driven
//! through the brain router over a brain composed on the test database.

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::brain::{BrainRoutes, router};
use systemprompt_brain::{FactRepository, NewFact};
use systemprompt_database::DbPool;
use systemprompt_extension::DynNotificationSink;
use systemprompt_identifiers::{BrainFactId, FactKindId, InstanceId, TopicId, UserId};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{FactCandidate, FactScope, FactSource, FactStatus, SubjectKey};
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_runtime::{BrainCore, BrainInputs, RecallSeams};
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{read_json, request_context, setup_ctx};

pub(super) struct BrainFixture {
    pub(super) pool: DbPool,
    pub(super) routes: BrainRoutes,
}

pub(super) async fn brain_fixture() -> Result<BrainFixture> {
    let (pool, ctx) = setup_ctx().await?;
    let profile = BrainProfile {
        enabled: true,
        ..BrainProfile::default()
    };
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let listener = pool.write_pool().as_ref().clone();
    let core = BrainCore::new(&BrainInputs {
        database: &pool,
        instance_id: &InstanceId::new("wp7-brain-routes"),
        listener_pool: &listener,
        notifications: &notifications,
        profile: &profile,
    })
    .context("an enabled profile composes a brain core")?;
    let handle = core.assemble(ctx.documents(), RecallSeams::default());
    let (service, recall, profile) = handle.routes();
    let users = ctx.user_provider().context("user provider")?;
    let routes = BrainRoutes::new(service, recall, profile, users);
    Ok(BrainFixture { pool, routes })
}

pub(super) async fn call(
    routes: &BrainRoutes,
    user: &UserId,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> Result<(StatusCode, Value)> {
    let app: Router = router(Some(routes.clone())).layer(Extension(
        request_context(user.as_str()).with_browser_session(),
    ));
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
    seed_fact_on(pool, owner, status, &subject).await
}

async fn seed_fact_on(
    pool: &DbPool,
    owner: &UserId,
    status: FactStatus,
    subject: &str,
) -> Result<BrainFactId> {
    let subject: SubjectKey = subject.parse()?;
    let candidate = FactCandidate::builder(
        FactKindId::try_new("decision")?,
        subject.clone(),
        FactSource::EnteredBy {
            user_id: owner.clone(),
        },
    )
    .field("owner", json!("billing"))
    .topic(TopicId::try_new("ops")?)
    .extractor("wp7")
    .build();
    let fact = NewFact::builder(
        candidate,
        FactScope::User(owner.clone()),
        "Billing owns the deploy gate.",
        format!("identity:{subject}:{}", Uuid::new_v4().simple()),
    )
    .status(status)
    .importance_base(0.5)
    .owner(owner.clone())
    .build();
    let mut conn = pool.write_pool().acquire().await?;
    Ok(FactRepository::insert_candidate(&mut conn, &fact).await?.id)
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
        &fx.routes,
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
        &fx.routes,
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
    let (status, why) = call(&fx.routes, &me.user_id, Method::GET, &uri, None).await?;
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
        &fx.routes,
        &other.user_id,
        Method::GET,
        &format!("/facts/{id}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &fx.routes,
        &other.user_id,
        Method::GET,
        &format!("/facts/{id}/timeline"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, listed) = call(
        &fx.routes,
        &other.user_id,
        Method::GET,
        "/facts?kind=decision&limit=500",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = listed
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
    let id = seed_fact_on(
        &fx.pool,
        &me.user_id,
        FactStatus::Confirmed,
        &format!("{subject_kind}:{subject_id}"),
    )
    .await?;

    let uri = format!(
        "/facts?kind=decision&subject={subject_kind}:{subject_id}&status=confirmed,proposed&limit=0"
    );
    let (status, listed) = call(&fx.routes, &me.user_id, Method::GET, &uri, None).await?;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed.as_array().map(Vec::len), Some(1), "{listed}");

    let uri = format!("/subjects/{subject_kind}/{subject_id}");
    let (status, subject) = call(&fx.routes, &me.user_id, Method::GET, &uri, None).await?;
    assert_eq!(status, StatusCode::OK, "{subject}");
    assert_eq!(subject[0]["id"], json!(id.as_str()));

    let (status, _) = call(
        &fx.routes,
        &me.user_id,
        Method::GET,
        "/facts?status=bogus",
        None,
    )
    .await?;
    assert!(status.is_client_error(), "{status}");
    let (status, _) = call(
        &fx.routes,
        &me.user_id,
        Method::GET,
        "/facts?cursor=x",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn an_unknown_or_inactive_caller_is_unauthorized() -> Result<()> {
    let fx = brain_fixture().await?;
    let ghost = UserId::generate();
    let (status, body) = call(&fx.routes, &ghost, Method::GET, "/facts", None).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    let (status, _) = call(&fx.routes, &ghost, Method::GET, "/proposals", None).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
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
        call(&fx.routes, &me.user_id, Method::POST, "/facts", Some(body)).await?;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");

    let missing = BrainFactId::generate();
    let (status, _) = call(
        &fx.routes,
        &me.user_id,
        Method::GET,
        &format!("/facts/{missing}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}
