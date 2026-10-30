//! `/api/v1/documents/{id}/agent-cap`,
//! `/api/v1/document-folders/{id}/agent-cap`, the `/live` channel, awareness
//! and binary collab updates, driven through the documents and folders routers.

use std::time::Duration;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::response::Response;
use axum::{Extension, Router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, folders_router, router};
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{browser_page, read_json, request_context, setup_ctx};

fn app(base: Router, user: &AuthedFixture) -> Router {
    browser_page(base).layer(Extension(
        request_context(user.user_id.as_str()).with_browser_session(),
    ))
}

async fn send(app: Router, method: Method, uri: &str, body: Option<Value>) -> Result<Response> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    let request = match body {
        Some(body) => builder.body(Body::from(body.to_string()))?,
        None => builder.body(Body::empty())?,
    };
    Ok(app.oneshot(request).await?)
}

async fn json_call(
    state: &DocumentsState,
    user: &AuthedFixture,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> Result<(StatusCode, Value)> {
    let response = send(app(router(state.clone()), user), method, uri, body).await?;
    let status = response.status();
    Ok((status, read_json(response).await?))
}

async fn create_html(state: &DocumentsState, owner: &AuthedFixture) -> Result<String> {
    let body = json!({
        "template_id": "blank-html",
        "name": format!("cap-{}.html", Uuid::new_v4().simple()),
        "params": { "values": { "title": "Cap" } }
    });
    let (status, created) = json_call(state, owner, Method::POST, "/", Some(body)).await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    Ok(created["document"]["id"].as_str().context("id")?.to_owned())
}

#[tokio::test]
async fn the_owner_reads_sets_and_clears_a_document_agent_cap() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "cap-owner").await?;
    let stranger = seed_bridge_credential(&pool, "cap-stranger").await?;
    let id = create_html(&state, &owner).await?;
    let uri = format!("/{id}/agent-cap");

    let (status, read) = json_call(&state, &owner, Method::GET, &uri, None).await?;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["target_kind"], json!("document"));
    assert_eq!(read["target_id"], json!(id));
    assert!(read["override"].is_null(), "{read}");
    assert!(read["attended"]["agent_cap"].is_string());
    assert!(read["unattended"]["reason"].is_string());

    let (status, set) = json_call(
        &state,
        &owner,
        Method::PUT,
        &uri,
        Some(json!({"cap": "read"})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(set["override"], json!("read"));
    assert_eq!(set["agent_cap"], json!("read"));

    let query = format!("{uri}?agent=claude-code&attended=true&channel=drive_sync");
    let (status, named) = json_call(&state, &owner, Method::GET, &query, None).await?;
    assert_eq!(status, StatusCode::OK, "{named}");
    assert_eq!(named["override"], json!("read"));

    let (status, _) = json_call(
        &state,
        &owner,
        Method::GET,
        &format!("{uri}?agent=unknown"),
        None,
    )
    .await?;
    assert!(status.is_client_error(), "{status}");
    let (status, _) = json_call(
        &state,
        &owner,
        Method::PUT,
        &uri,
        Some(json!({"cap": "everything"})),
    )
    .await?;
    assert!(status.is_client_error(), "{status}");

    let (status, refused) = json_call(
        &state,
        &stranger,
        Method::PUT,
        &uri,
        Some(json!({"cap": "edit"})),
    )
    .await?;
    assert!(
        matches!(status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
        "{status} {refused}"
    );

    let (status, cleared) = json_call(
        &state,
        &owner,
        Method::PUT,
        &uri,
        Some(json!({"cap": null})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared["override"].is_null(), "{cleared}");
    Ok(())
}

#[tokio::test]
async fn the_owner_sets_a_folder_agent_cap() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "cap-folder").await?;
    let folders = || app(folders_router(state.clone()), &owner);
    let name = format!("cap-folder-{}", Uuid::new_v4().simple());
    let created = send(folders(), Method::POST, "/", Some(json!({"name": name}))).await?;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = read_json(created).await?;
    let id = created["id"]
        .as_str()
        .or_else(|| created["folder"]["id"].as_str())
        .context("folder id")?
        .to_owned();
    let uri = format!("/{id}/agent-cap");

    let response = send(
        folders(),
        Method::PUT,
        &uri,
        Some(json!({"cap": "comment"})),
    )
    .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let set = read_json(response).await?;
    assert_eq!(set["target_kind"], json!("folder"));
    assert_eq!(set["override"], json!("comment"));

    let response = send(folders(), Method::GET, &uri, None).await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(read_json(response).await?["override"], json!("comment"));

    let response = send(folders(), Method::GET, "/%20/agent-cap", None).await?;
    assert!(response.status().is_client_error());
    Ok(())
}

async fn first_frames(response: Response) -> Result<String> {
    let mut body = response.into_body();
    let mut seen = String::new();
    while !seen.contains("document.live.ready") {
        let frame = tokio::time::timeout(Duration::from_secs(5), body.frame())
            .await?
            .context("stream ended")??;
        if let Ok(data) = frame.into_data() {
            seen.push_str(&String::from_utf8_lossy(&data));
        }
    }
    Ok(seen)
}

#[tokio::test]
async fn the_live_channel_opens_ready_and_carries_awareness() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "live-owner").await?;
    let stranger = seed_bridge_credential(&pool, "live-stranger").await?;
    let id = create_html(&state, &owner).await?;

    let awareness = json!({"color": "#336699", "mode": "viewing"});
    let (status, published) = json_call(
        &state,
        &owner,
        Method::POST,
        &format!("/{id}/collab/awareness"),
        Some(awareness),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{published}");
    assert!(published["published"].is_boolean(), "{published}");
    assert!(published["expires_at"].is_string(), "{published}");

    let response = send(
        app(router(state.clone()), &owner),
        Method::GET,
        &format!("/{id}/live"),
        None,
    )
    .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers()[header::CONTENT_TYPE].to_str()?;
    assert!(content_type.starts_with("text/event-stream"));
    let seen = first_frames(response).await?;
    assert!(seen.contains(&id), "{seen}");

    let uri = format!("/{id}/live?last_event_id=garbage");
    let response = send(app(router(state.clone()), &owner), Method::GET, &uri, None).await?;
    assert_eq!(response.status(), StatusCode::OK);
    first_frames(response).await?;

    let response = send(
        app(router(state.clone()), &stranger),
        Method::GET,
        &format!("/{id}/live"),
        None,
    )
    .await?;
    assert!(response.status().is_client_error(), "{}", response.status());
    let (status, _) = json_call(
        &state,
        &stranger,
        Method::POST,
        &format!("/{id}/collab/awareness"),
        Some(json!({"color": "#000000", "mode": "editing"})),
    )
    .await?;
    assert!(status.is_client_error(), "{status}");
    Ok(())
}

#[tokio::test]
async fn binary_collab_updates_need_a_session_in_the_query() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "collab-binary").await?;
    let id = create_html(&state, &owner).await?;
    let (status, joined) = json_call(
        &state,
        &owner,
        Method::POST,
        &format!("/{id}/collab/join"),
        Some(json!({"mode": "view"})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{joined}");
    let session = joined["session_id"].as_str().context("session")?.to_owned();

    let binary = |uri: String| {
        Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(vec![0_u8, 1, 2, 3]))
    };
    let response = app(router(state.clone()), &owner)
        .oneshot(binary(format!("/{id}/collab/updates"))?)
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app(router(state.clone()), &owner)
        .oneshot(binary(format!(
            "/{id}/collab/updates?session_id={session}"
        ))?)
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let (status, _) = json_call(
        &state,
        &owner,
        Method::POST,
        &format!("/{id}/collab/updates"),
        Some(json!({"nope": 1})),
    )
    .await?;
    assert!(status.is_client_error(), "{status}");
    Ok(())
}
