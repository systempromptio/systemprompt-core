//! `/api/v1/uploads` driven through the uploads router: a session-authed
//! upload from open to content, aborts and refusals.

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use systemprompt_api::routes::documents::DocumentsState;
use systemprompt_api::routes::uploads::router;
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{read_json, request_context, setup_ctx};

const PAYLOAD: &[u8] = b"hello upload world\n";

fn authed(state: &DocumentsState, user: &AuthedFixture) -> Router {
    router(state.clone()).layer(Extension(request_context(user.user_id.as_str())))
}

fn request(method: Method, uri: &str, body: Body) -> Result<Request<Body>> {
    Ok(Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)?)
}

async fn call(app: Router, method: Method, uri: &str, body: Body) -> Result<(StatusCode, Value)> {
    let response = app.oneshot(request(method, uri, body)?).await?;
    let status = response.status();
    Ok((status, read_json(response).await?))
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(super) async fn open(state: &DocumentsState, user: &AuthedFixture) -> Result<String> {
    let body = json!({
        "name": format!("notes-{}.txt", Uuid::new_v4().simple()),
        "mime": "text/plain",
        "size": PAYLOAD.len(),
        "sha256": sha(PAYLOAD),
    });
    let (status, created) = call(
        authed(state, user),
        Method::POST,
        "/",
        Body::from(body.to_string()),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["status"], json!("open"));
    assert_eq!(created["part_count"], json!(1));
    assert_eq!(created["missing_parts"], json!([1]));
    Ok(created["upload_id"]
        .as_str()
        .context("upload id")?
        .to_owned())
}

#[tokio::test]
async fn an_upload_runs_from_open_to_content() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let user = seed_bridge_credential(&pool, "uploads-flow").await?;
    let id = open(&state, &user).await?;

    let response = authed(&state, &user)
        .oneshot(request(
            Method::PUT,
            &format!("/{id}/parts/1"),
            Body::from(PAYLOAD),
        )?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key(header::ETAG));

    let (status, progress) = call(
        authed(&state, &user),
        Method::GET,
        &format!("/{id}"),
        Body::empty(),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{progress}");
    assert_eq!(progress["received_parts"], json!([1]));
    assert_eq!(progress["received_bytes"], json!(PAYLOAD.len()));
    assert_eq!(progress["missing_parts"], json!([]));

    let (status, done) = call(
        authed(&state, &user),
        Method::POST,
        &format!("/{id}/complete"),
        Body::empty(),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["blob_ref"], json!(sha(PAYLOAD)));
    assert_eq!(done["size_bytes"], json!(PAYLOAD.len()));

    let content = format!("/{id}/content");
    let full = authed(&state, &user)
        .oneshot(request(Method::GET, &content, Body::empty())?)
        .await?;
    assert_eq!(full.status(), StatusCode::OK);
    let etag = full.headers()[header::ETAG].to_str()?.to_owned();
    assert_eq!(full.headers()[header::ACCEPT_RANGES], "bytes");
    assert!(
        full.headers()[header::CONTENT_DISPOSITION]
            .to_str()?
            .starts_with("attachment;")
    );
    assert!(full.headers().contains_key(header::CONTENT_SECURITY_POLICY));
    let bytes = axum::body::to_bytes(full.into_body(), 1 << 20).await?;
    assert_eq!(&bytes[..], PAYLOAD);

    let ranged = Request::builder()
        .uri(&content)
        .header(header::RANGE, "bytes=0-4")
        .body(Body::empty())?;
    let ranged = authed(&state, &user).oneshot(ranged).await?;
    assert_eq!(ranged.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        ranged.headers()[header::CONTENT_RANGE],
        format!("bytes 0-4/{}", PAYLOAD.len())
    );
    assert_eq!(
        &axum::body::to_bytes(ranged.into_body(), 64).await?[..],
        b"hello"
    );

    let far = Request::builder()
        .uri(&content)
        .header(header::RANGE, "bytes=9000-9001")
        .body(Body::empty())?;
    let far = authed(&state, &user).oneshot(far).await?;
    assert_eq!(far.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        far.headers()[header::CONTENT_RANGE],
        format!("bytes */{}", PAYLOAD.len())
    );

    let cached = Request::builder()
        .uri(&content)
        .header(header::IF_NONE_MATCH, &etag)
        .body(Body::empty())?;
    assert_eq!(
        authed(&state, &user).oneshot(cached).await?.status(),
        StatusCode::NOT_MODIFIED
    );

    let head = authed(&state, &user)
        .oneshot(request(Method::HEAD, &content, Body::empty())?)
        .await?;
    assert_eq!(head.status(), StatusCode::OK);
    assert!(axum::body::to_bytes(head.into_body(), 64).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn aborts_strangers_and_bad_ids_are_refused() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let user = seed_bridge_credential(&pool, "uploads-abort").await?;
    let stranger = seed_bridge_credential(&pool, "uploads-stranger").await?;
    let id = open(&state, &user).await?;

    let (status, _) = call(
        authed(&state, &stranger),
        Method::GET,
        &format!("/{id}"),
        Body::empty(),
    )
    .await?;
    assert!(
        matches!(status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
        "{status}"
    );
    let (status, _) = call(
        authed(&state, &user),
        Method::GET,
        "/not-an-id",
        Body::empty(),
    )
    .await?;
    assert!(status.is_client_error(), "{status}");
    let (status, _) = call(
        authed(&state, &user),
        Method::GET,
        &format!("/{id}/content"),
        Body::empty(),
    )
    .await?;
    assert!(status.is_client_error(), "{status}");

    let (status, _) = call(
        router(state.clone()),
        Method::GET,
        &format!("/{id}"),
        Body::empty(),
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(router(state.clone()), Method::POST, "/", Body::from("{}")).await?;
    assert!(status.is_client_error(), "{status}");

    let (status, _) = call(
        authed(&state, &user),
        Method::DELETE,
        &format!("/{id}"),
        Body::empty(),
    )
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, progress) = call(
        authed(&state, &user),
        Method::GET,
        &format!("/{id}"),
        Body::empty(),
    )
    .await?;
    assert_ne!(progress["status"], json!("open"), "{progress}");
    assert_eq!(progress["missing_parts"], json!([]));
    Ok(())
}
