//! Access requests over HTTP while the install enables them: a signed-in
//! stranger is told they may ask, asks once, the owner sees the request on
//! the access view, and granting or declining closes it.

use anyhow::Result;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, router};
use systemprompt_files::DocumentsConfig;
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, fixture_app_context_documents_configured, fixture_config,
    seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::request_context;

async fn requests_enabled() -> Result<(systemprompt_database::DbPool, DocumentsState)> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let documents = DocumentsConfig {
        enabled: true,
        access_requests: true,
        ..DocumentsConfig::default()
    };
    let ctx = fixture_app_context_documents_configured(
        &pool,
        fixture_config(&b.database_url),
        documents,
    )?;
    let state = DocumentsState::build(&ctx)?;
    Ok((pool, state))
}

async fn call(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<&Value>,
) -> Result<(StatusCode, Value)> {
    let builder = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))?,
        None => builder.body(Body::empty())?,
    };
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    ))
}

fn docs(state: &DocumentsState, user: &AuthedFixture) -> Router {
    router(state.clone()).layer(Extension(
        request_context(user.user_id.as_str()).with_browser_session(),
    ))
}

async fn document(state: &DocumentsState, owner: &AuthedFixture) -> Result<String> {
    let body = json!({
        "template_id": "blank-markdown",
        "name": format!("ask-{}.md", Uuid::new_v4().simple()),
        "params": { "values": { "title": "Plan" } }
    });
    let (status, created) = call(docs(state, owner), Method::POST, "/", Some(&body)).await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    Ok(created["document"]["id"].as_str().expect("id").to_owned())
}

async fn ask(
    state: &DocumentsState,
    who: &AuthedFixture,
    id: &str,
    level: &str,
) -> Result<(StatusCode, Value)> {
    let body = json!({ "level": level, "message": "need this for the review" });
    call(
        docs(state, who),
        Method::POST,
        &format!("/{id}/access-requests"),
        Some(&body),
    )
    .await
}

async fn decide(
    state: &DocumentsState,
    who: &AuthedFixture,
    id: &str,
    request: &str,
    verb: &str,
) -> Result<(StatusCode, Value)> {
    call(
        docs(state, who),
        Method::POST,
        &format!("/{id}/access-requests/{request}/{verb}"),
        None,
    )
    .await
}

async fn access_view(
    state: &DocumentsState,
    who: &AuthedFixture,
    id: &str,
) -> Result<(StatusCode, Value)> {
    call(
        docs(state, who),
        Method::GET,
        &format!("/{id}/access"),
        None,
    )
    .await
}

#[tokio::test]
async fn a_stranger_may_ask_once_the_owner_sees_it_and_a_grant_shares_the_level() -> Result<()> {
    let (pool, state) = requests_enabled().await?;
    let owner = seed_bridge_credential(&pool, "ask-grant-owner").await?;
    let asker = seed_bridge_credential(&pool, "ask-grant-asker").await?;
    let id = document(&state, &owner).await?;

    let (status, refused) =
        call(docs(&state, &asker), Method::GET, &format!("/{id}"), None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");
    assert_eq!(refused["detail"]["can_request"], true, "{refused}");

    let (status, created) = ask(&state, &asker, &id, "comment").await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["status"], "open");
    assert_eq!(created["document_id"], id.as_str());
    assert_eq!(created["requester_user_id"], asker.user_id.as_str());
    assert_eq!(created["level"], "comment");
    assert_eq!(created["message"], "need this for the review");
    let request = created["request_id"]
        .as_str()
        .expect("request id")
        .to_owned();

    let (status, again) = ask(&state, &asker, &id, "view").await?;
    assert_eq!(status, StatusCode::CONFLICT, "{again}");

    let (status, seen) = access_view(&state, &owner, &id).await?;
    assert_eq!(status, StatusCode::OK, "{seen}");
    let open = seen["requests"].as_array().expect("requests");
    assert_eq!(open.len(), 1, "{seen}");
    assert_eq!(open[0]["request_id"], request.as_str());
    assert_eq!(open[0]["status"], "open");

    let (status, not_yours) = access_view(&state, &asker, &id).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{not_yours}");
    assert_eq!(not_yours["detail"]["can_request"], true, "{not_yours}");

    let (status, granted) = decide(&state, &owner, &id, &request, "grant").await?;
    assert_eq!(status, StatusCode::OK, "{granted}");
    assert_eq!(granted["status"], "granted");
    assert_eq!(granted["decided_by"], owner.user_id.as_str());
    assert!(granted["decided_at"].is_string(), "{granted}");

    let (status, opened) = call(docs(&state, &asker), Method::GET, &format!("/{id}"), None).await?;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let (status, mine) = access_view(&state, &asker, &id).await?;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine["my_access"], "comment");
    let (_, settled) = access_view(&state, &owner, &id).await?;
    assert!(settled.get("requests").is_none(), "{settled}");
    assert!(
        settled["grants"]
            .as_array()
            .expect("grants")
            .iter()
            .any(|g| g["subject_value"] == asker.user_id.as_str() && g["level"] == "comment"),
        "{settled}"
    );

    let (status, closed) = decide(&state, &owner, &id, &request, "decline").await?;
    assert_eq!(status, StatusCode::CONFLICT, "{closed}");
    let (status, needless) = ask(&state, &asker, &id, "edit").await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{needless}");
    Ok(())
}

#[tokio::test]
async fn a_decline_closes_the_request_and_only_share_holders_may_decide() -> Result<()> {
    let (pool, state) = requests_enabled().await?;
    let owner = seed_bridge_credential(&pool, "ask-decline-owner").await?;
    let asker = seed_bridge_credential(&pool, "ask-decline-asker").await?;
    let stranger = seed_bridge_credential(&pool, "ask-decline-stranger").await?;
    let id = document(&state, &owner).await?;

    let (status, created) = ask(&state, &asker, &id, "view").await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let request = created["request_id"]
        .as_str()
        .expect("request id")
        .to_owned();

    let (status, refused) = decide(&state, &stranger, &id, &request, "decline").await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");
    let (status, refused) = decide(&state, &asker, &id, &request, "grant").await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");

    let (status, declined) = decide(&state, &owner, &id, &request, "decline").await?;
    assert_eq!(status, StatusCode::OK, "{declined}");
    assert_eq!(declined["status"], "declined");
    assert_eq!(declined["decided_by"], owner.user_id.as_str());

    let (status, still_out) =
        call(docs(&state, &asker), Method::GET, &format!("/{id}"), None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{still_out}");
    assert_eq!(still_out["detail"]["can_request"], true, "{still_out}");
    let (_, after) = access_view(&state, &owner, &id).await?;
    assert!(after.get("requests").is_none(), "{after}");

    let (status, renewed) = ask(&state, &asker, &id, "view").await?;
    assert_eq!(status, StatusCode::CREATED, "{renewed}");
    assert_ne!(renewed["request_id"], request.as_str());

    let missing = Uuid::new_v4();
    let (status, unknown) = decide(&state, &owner, &id, &missing.to_string(), "grant").await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{unknown}");
    Ok(())
}
