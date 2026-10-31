use anyhow::Result;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, router};
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{browser_page, request_context, setup_ctx};

async fn send(app: Router, req: Request<Body>) -> Result<(StatusCode, Value)> {
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let body = to_bytes(resp.into_body(), 16 * 1024 * 1024).await?;
    Ok((status, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

fn app(state: &DocumentsState, user: &AuthedFixture) -> Router {
    browser_page(router(state.clone())).layer(Extension(
        request_context(user.user_id.as_str()).with_browser_session(),
    ))
}

fn empty(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

fn json_request(method: Method, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

struct Doc {
    state: DocumentsState,
    owner: AuthedFixture,
    id: String,
    comment: String,
}

async fn commented(tag: &str) -> Result<Doc> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, tag).await?;
    let body = json!({
        "template_id": "blank-markdown",
        "name": format!("{tag}-{}.md", Uuid::new_v4().simple()),
        "params": { "values": { "title": "Plan" } }
    });
    let (status, created) =
        send(app(&state, &owner), json_request(Method::POST, "/", &body)).await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["document"]["id"].as_str().expect("id").to_owned();
    let (_, outline) = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/read?view=outline")),
    )
    .await?;
    let line = outline["lines"]
        .as_array()
        .and_then(|lines| lines.first())
        .and_then(Value::as_str)
        .expect("a line");
    let (anchor, _) = line.split_once('|').expect("hashline");
    let node = anchor.rsplit_once(':').expect("node:hash").0.to_owned();
    let (status, comment) = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/comments"),
            &json!({ "anchor": { "node": node }, "body": "first draft" }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{comment}");
    let comment = comment["id"].as_str().expect("comment id").to_owned();
    Ok(Doc {
        state,
        owner,
        id,
        comment,
    })
}

async fn listed(doc: &Doc) -> Result<Vec<Value>> {
    let (status, listed) = send(
        app(&doc.state, &doc.owner),
        empty(Method::GET, &format!("/{}/comments", doc.id)),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{listed}");
    Ok(listed["comments"].as_array().cloned().unwrap_or_default())
}

#[tokio::test]
async fn the_author_edits_a_comment_body() -> Result<()> {
    let doc = commented("docs-cedit").await?;
    let (status, edited) = send(
        app(&doc.state, &doc.owner),
        json_request(
            Method::PATCH,
            &format!("/{}/comments/{}", doc.id, doc.comment),
            &json!({ "body": "second draft" }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["id"], doc.comment.as_str());
    assert_eq!(edited["body"], "second draft");
    let comments = listed(&doc).await?;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "second draft");
    Ok(())
}

#[tokio::test]
async fn an_option_for_an_unmentioned_user_is_refused_and_leaves_the_body() -> Result<()> {
    let doc = commented("docs-cedit-opt").await?;
    let (status, refused) = send(
        app(&doc.state, &doc.owner),
        json_request(
            Method::PATCH,
            &format!("/{}/comments/{}", doc.id, doc.comment),
            &json!({
                "body": "with options",
                "mention_options": [
                    { "user_id": "user-not-mentioned", "attention": "human" }
                ]
            }),
        ),
    )
    .await?;
    assert!(
        status.is_client_error() || status.is_server_error(),
        "{status} {refused}"
    );
    assert_eq!(listed(&doc).await?[0]["body"], "first draft");
    Ok(())
}

#[tokio::test]
async fn a_malformed_edit_body_is_400() -> Result<()> {
    let doc = commented("docs-cedit-bad").await?;
    let (status, _) = send(
        app(&doc.state, &doc.owner),
        json_request(
            Method::PATCH,
            &format!("/{}/comments/{}", doc.id, doc.comment),
            &json!({ "text": "no body field" }),
        ),
    )
    .await?;
    assert!(status.is_client_error(), "{status}");
    assert_eq!(listed(&doc).await?[0]["body"], "first draft");
    Ok(())
}

#[tokio::test]
async fn deleting_a_comment_removes_it_and_a_second_delete_is_not_found() -> Result<()> {
    let doc = commented("docs-cdel").await?;
    let uri = format!("/{}/comments/{}", doc.id, doc.comment);
    let (status, _) = send(app(&doc.state, &doc.owner), empty(Method::DELETE, &uri)).await?;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(listed(&doc).await?.is_empty());

    let (status, again) = send(app(&doc.state, &doc.owner), empty(Method::DELETE, &uri)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{again}");
    let (status, edit) = send(
        app(&doc.state, &doc.owner),
        json_request(Method::PATCH, &uri, &json!({ "body": "too late" })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{edit}");
    Ok(())
}

#[tokio::test]
async fn a_stranger_cannot_edit_or_delete_a_comment() -> Result<()> {
    let doc = commented("docs-cstranger").await?;
    let (pool, _ctx) = setup_ctx().await?;
    let stranger = seed_bridge_credential(&pool, "docs-cstranger-x").await?;
    let uri = format!("/{}/comments/{}", doc.id, doc.comment);
    let (status, _) = send(
        app(&doc.state, &stranger),
        json_request(Method::PATCH, &uri, &json!({ "body": "hijack" })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(app(&doc.state, &stranger), empty(Method::DELETE, &uri)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(listed(&doc).await?[0]["body"], "first draft");
    Ok(())
}

#[tokio::test]
async fn an_invalid_comment_id_is_refused() -> Result<()> {
    let doc = commented("docs-cbadid").await?;
    let (status, _) = send(
        app(&doc.state, &doc.owner),
        empty(Method::DELETE, &format!("/{}/comments/%20", doc.id)),
    )
    .await?;
    assert!(status.is_client_error(), "{status}");
    assert_eq!(listed(&doc).await?.len(), 1);
    Ok(())
}
