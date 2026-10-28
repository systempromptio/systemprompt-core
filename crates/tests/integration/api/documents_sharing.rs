//! Sharing over HTTP: subject search, unknown subjects, the grant history,
//! folder grants with inherited access, and access requests while the
//! install leaves them off.

use anyhow::Result;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, folders_router, router};
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{request_context, setup_ctx};

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
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    ))
}

fn docs(state: &DocumentsState, user: &AuthedFixture) -> Router {
    router(state.clone()).layer(Extension(request_context(user.user_id.as_str())))
}

fn folders(state: &DocumentsState, user: &AuthedFixture) -> Router {
    folders_router(state.clone()).layer(Extension(request_context(user.user_id.as_str())))
}

async fn document(
    state: &DocumentsState,
    owner: &AuthedFixture,
    folder: Option<&str>,
) -> Result<String> {
    let body = json!({
        "template_id": "blank-markdown",
        "name": format!("share-{}.md", Uuid::new_v4().simple()),
        "folder_id": folder,
        "params": { "values": { "title": "Plan" } }
    });
    let (status, created) = call(docs(state, owner), Method::POST, "/", Some(&body)).await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    Ok(created["document"]["id"].as_str().expect("id").to_owned())
}

async fn folder(
    state: &DocumentsState,
    owner: &AuthedFixture,
    parent: Option<&str>,
    name: &str,
) -> Result<String> {
    let body = json!({ "name": name, "parent_id": parent });
    let (status, created) = call(folders(state, owner), Method::POST, "/", Some(&body)).await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    Ok(created["id"].as_str().expect("folder id").to_owned())
}

#[tokio::test]
async fn subjects_are_searched_and_unknown_subjects_are_refused() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let base = format!("srch{}", Uuid::new_v4().simple());
    let owner = seed_bridge_credential(&pool, &format!("{base}-owner")).await?;
    let (status, found) = call(
        docs(&state, &owner),
        Method::GET,
        &format!("/subjects?kind=user&q={base}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{found}");
    assert_eq!(found["searchable"], true);
    assert!(
        found["subjects"]
            .as_array()
            .expect("subjects")
            .iter()
            .any(|s| s["value"] == owner.user_id.as_str()),
        "{found}"
    );
    let (_, roles) = call(
        docs(&state, &owner),
        Method::GET,
        "/subjects?kind=role&q=us",
        None,
    )
    .await?;
    assert!(
        roles["subjects"]
            .as_array()
            .expect("roles")
            .iter()
            .any(|s| s["value"] == "user"),
        "{roles}"
    );
    let (bad_kind, _) = call(
        docs(&state, &owner),
        Method::GET,
        "/subjects?kind=planet",
        None,
    )
    .await?;
    assert_eq!(bad_kind, StatusCode::BAD_REQUEST);

    let id = document(&state, &owner, None).await?;
    for (kind, value) in [("user", "nobody-at-all"), ("role", "no-such-role-anywhere")] {
        let share = json!({ "subject_kind": kind, "subject_value": value, "level": "view" });
        let (status, body) = call(
            docs(&state, &owner),
            Method::POST,
            &format!("/{id}/grants"),
            Some(&share),
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error_key"], "unknown_subject", "{body}");
    }
    Ok(())
}

#[tokio::test]
async fn history_is_read_by_share_holders_and_folder_grants_show_their_folder() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "share-hist-owner").await?;
    let reader = seed_bridge_credential(&pool, "share-hist-reader").await?;
    let parent_name = format!("Parent {}", Uuid::new_v4().simple());
    let parent = folder(&state, &owner, None, &parent_name).await?;
    let child = folder(&state, &owner, Some(&parent), "Child").await?;
    let share = json!({ "subject_kind": "user", "subject_value": reader.user_id.as_str(), "level": "comment", "message": "for review" });
    let (status, body) = call(
        folders(&state, &owner),
        Method::POST,
        &format!("/{parent}/grants"),
        Some(&share),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, listed) = call(
        folders(&state, &owner),
        Method::GET,
        &format!("/{parent}/grants"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        listed["grants"].as_array().map(Vec::len),
        Some(2),
        "{listed}"
    );

    let (status, access) = call(
        folders(&state, &owner),
        Method::GET,
        &format!("/{child}/access?history=true"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{access}");
    assert_eq!(access["folder_id"], child.as_str());
    let inherited = access["inherited"].as_array().expect("inherited");
    assert!(
        inherited
            .iter()
            .any(|g| g["subject_value"] == reader.user_id.as_str()
                && g["inherited_from"]["folder_id"] == parent.as_str()
                && g["inherited_from"]["name"] == parent_name.as_str()),
        "{access}"
    );

    let id = document(&state, &owner, Some(&child)).await?;
    let (_, as_reader) = call(
        docs(&state, &reader),
        Method::GET,
        &format!("/{id}/access?history=true"),
        None,
    )
    .await?;
    assert_eq!(as_reader["my_access"], "comment", "{as_reader}");
    assert!(
        as_reader.get("history").is_none()
            && as_reader["grants"].as_array().is_some_and(Vec::is_empty)
    );
    let (_, parent_access) = call(
        folders(&state, &owner),
        Method::GET,
        &format!("/{parent}/access?history=true"),
        None,
    )
    .await?;
    assert!(
        parent_access["history"]
            .as_array()
            .expect("history")
            .iter()
            .any(|e| e["event"] == "granted" && e["subject_value"] == reader.user_id.as_str()),
        "{parent_access}"
    );

    let grant_id = listed["grants"]
        .as_array()
        .expect("grants")
        .iter()
        .find(|g| g["subject_value"] == reader.user_id.as_str())
        .and_then(|g| g["id"].as_str())
        .expect("grant")
        .to_owned();
    let (status, _) = call(
        folders(&state, &owner),
        Method::DELETE,
        &format!("/{parent}/grants?grant_id={grant_id}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(docs(&state, &reader), Method::GET, &format!("/{id}"), None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn access_requests_stay_not_found_while_the_install_disables_them() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "share-req-owner").await?;
    let stranger = seed_bridge_credential(&pool, "share-req-stranger").await?;
    let id = document(&state, &owner, None).await?;
    let (status, refused) = call(
        docs(&state, &stranger),
        Method::GET,
        &format!("/{id}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(refused["detail"]["can_request"].is_null(), "{refused}");
    let ask = json!({ "level": "view", "message": "please" });
    let (status, _) = call(
        docs(&state, &stranger),
        Method::POST,
        &format!("/{id}/access-requests"),
        Some(&ask),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}
