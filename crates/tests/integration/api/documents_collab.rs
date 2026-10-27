//! `/api/v1/documents/{id}/collab/*` driven through the documents router:
//! response shapes, base64 payloads, edit-mode enforcement and access.

use std::sync::Arc;

use anyhow::Result;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loro::{ExportMode, LoroDoc};
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, router};
use systemprompt_database::DbPool;
use systemprompt_governance::authz::{AllowAllHook, SharedAuthzHook, SubjectProviderSet};
use systemprompt_runtime::AppContext;
use systemprompt_storage::LocalFileStorage;
use systemprompt_test_fixtures::{
    AuthedFixture, fixture_documents_runtime_on, seed_bridge_credential,
};
use systemprompt_traits::{BackgroundTasks, FileStorage};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{request_context, setup_ctx};

async fn fixture() -> Result<(DbPool, Arc<AppContext>, DocumentsState)> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    Ok((pool, ctx, state))
}

async fn post(
    state: &DocumentsState,
    user: &AuthedFixture,
    uri: &str,
    body: &Value,
) -> Result<(StatusCode, Value)> {
    let app: Router =
        router(state.clone()).layer(Extension(request_context(user.user_id.as_str())));
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))?;
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 16 * 1024 * 1024).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    ))
}

async fn create_html(state: &DocumentsState, owner: &AuthedFixture) -> Result<String> {
    let body = json!({
        "template_id": "blank-html",
        "name": format!("page-{}.html", Uuid::new_v4().simple()),
        "params": { "values": { "title": "Page" } }
    });
    let (status, created) = post(state, owner, "/", &body).await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    Ok(created["document"]["id"].as_str().expect("id").to_owned())
}

fn update_from(joined: &Value, text: &str) -> Result<(String, String)> {
    let doc = LoroDoc::new();
    doc.import(&STANDARD.decode(joined["snapshot_b64"].as_str().expect("snapshot"))?)?;
    let before = doc.oplog_vv();
    let body = doc.get_text("body");
    body.insert(body.len_unicode(), text)?;
    doc.commit();
    let update = doc.export(ExportMode::updates(&before))?;
    Ok((
        STANDARD.encode(update),
        STANDARD.encode(doc.oplog_vv().encode()),
    ))
}

#[tokio::test]
async fn the_collab_routes_join_apply_cursor_and_leave() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "collab-routes").await?;
    let id = create_html(&state, &owner).await?;

    let (status, joined) = post(
        &state,
        &owner,
        &format!("/{id}/collab/join"),
        &json!({"mode": "edit"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{joined}");
    let session = joined["session_id"].as_str().expect("session").to_owned();
    assert!(joined["vv"].is_string());
    assert!(joined["head_revision"].is_string());
    assert_eq!(joined["seq"], json!(0));

    let (update, vv) = update_from(&joined, "<p>Routed</p>")?;
    let (status, applied) = post(
        &state,
        &owner,
        &format!("/{id}/collab/updates"),
        &json!({"session_id": session, "vv": vv, "update_b64": update}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert!(applied["seq"].as_i64().expect("seq") > 0);
    assert!(applied["vv"].is_string());

    let (status, _) = post(
        &state,
        &owner,
        &format!("/{id}/collab/cursor"),
        &json!({"session_id": session, "cursor": {"anchor": 3, "head": 3}}),
    )
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, left) = post(
        &state,
        &owner,
        &format!("/{id}/collab/leave"),
        &json!({"session_id": session}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{left}");
    assert_eq!(left["closed"], json!(true));
    assert_eq!(left["checkpoint"], json!("committed"));
    assert!(left["checkpoint_revision"].is_string());
    assert!(left.get("proposal_id").is_none());
    Ok(())
}

#[tokio::test]
async fn a_viewer_leaving_an_unedited_session_leaves_it_open_and_unchanged() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "collab-view-leave").await?;
    let id = create_html(&state, &owner).await?;
    let (status, joined) = post(
        &state,
        &owner,
        &format!("/{id}/collab/join"),
        &json!({"mode": "view"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{joined}");
    let (status, left) = post(
        &state,
        &owner,
        &format!("/{id}/collab/leave"),
        &json!({"session_id": joined["session_id"]}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{left}");
    assert_eq!(left["closed"], json!(false));
    assert_eq!(left["checkpoint"], json!("unchanged"));
    assert!(left["checkpoint_revision"].is_null());
    Ok(())
}

#[tokio::test]
async fn the_documents_runtime_starts_one_collab_timer() -> Result<()> {
    let (pool, _ctx, _state) = fixture().await?;
    let storage = tempfile::TempDir::new()?;
    let files: Arc<dyn FileStorage> = Arc::new(LocalFileStorage::new(storage.path().to_path_buf()));
    let tasks = BackgroundTasks::new();
    let _runtime = fixture_documents_runtime_on(
        &pool,
        &files,
        &(Arc::new(AllowAllHook::null()) as SharedAuthzHook),
        Arc::new(SubjectProviderSet::default()),
        tasks.clone(),
    );
    assert_eq!(tasks.in_flight(), 1);
    Ok(())
}

#[tokio::test]
async fn a_view_join_cannot_post_updates_and_bad_base64_is_rejected() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "collab-view").await?;
    let id = create_html(&state, &owner).await?;
    let (_, joined) = post(
        &state,
        &owner,
        &format!("/{id}/collab/join"),
        &json!({"mode": "view"}),
    )
    .await?;
    let session = joined["session_id"].as_str().expect("session").to_owned();
    let (update, _) = update_from(&joined, "<p>x</p>")?;

    let (status, refused) = post(
        &state,
        &owner,
        &format!("/{id}/collab/updates"),
        &json!({"session_id": session, "update_b64": update}),
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");

    let (status, _) = post(
        &state,
        &owner,
        &format!("/{id}/collab/updates"),
        &json!({"session_id": session, "update_b64": "%%%"}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn a_stranger_cannot_join_and_a_closed_session_is_a_conflict() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "collab-owner").await?;
    let stranger = seed_bridge_credential(&pool, "collab-stranger").await?;
    let id = create_html(&state, &owner).await?;

    let (status, _) = post(
        &state,
        &stranger,
        &format!("/{id}/collab/join"),
        &json!({"mode": "view"}),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, joined) = post(
        &state,
        &owner,
        &format!("/{id}/collab/join"),
        &json!({"mode": "edit"}),
    )
    .await?;
    let (update, _) = update_from(&joined, "<p>late</p>")?;
    let stale = Uuid::new_v4().to_string();
    let (status, body) = post(
        &state,
        &owner,
        &format!("/{id}/collab/updates"),
        &json!({"session_id": stale, "update_b64": update}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    Ok(())
}
