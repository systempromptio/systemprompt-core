//! `/import` from a completed upload, `/{id}/export/docx` and
//! `/{id}/media` through the documents router.

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::{Method, StatusCode};
use axum::{Extension, Router};
use serde_json::json;
use systemprompt_api::routes::documents::DocumentsState;
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;

use super::common::request_context;
use super::documents::{app, create_markdown, fixture, id_of, json_request, request, send};
use super::routes_uploads::{PAYLOAD, open};

fn uploads(state: &DocumentsState, user: &AuthedFixture) -> Router {
    systemprompt_api::routes::uploads::router(state.clone())
        .layer(Extension(request_context(user.user_id.as_str())))
}

async fn completed_blob(state: &DocumentsState, user: &AuthedFixture) -> Result<String> {
    let id = open(state, user).await?;
    let put = uploads(state, user)
        .oneshot(
            request(Method::PUT, &format!("/{id}/parts/1"))
                .body(Body::from(PAYLOAD))
                .expect("request"),
        )
        .await?;
    assert_eq!(put.status(), StatusCode::OK);
    let done = send(
        uploads(state, user),
        request(Method::POST, &format!("/{id}/complete"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(done.status, StatusCode::OK);
    Ok(done.json()["blob_ref"]
        .as_str()
        .context("blob ref")?
        .to_owned())
}

#[tokio::test]
async fn a_completed_upload_imports_as_a_document_its_owner_alone_can_use() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-import-blob").await?;
    let blob = completed_blob(&state, &owner).await?;
    let name = format!("imported-{}.txt", uuid::Uuid::new_v4().simple());

    let reply = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            "/import",
            &json!({ "blob_ref": blob, "name": name }),
        ),
    )
    .await?;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.json());
    let created = reply.json();
    assert_eq!(created["document"]["name"], name.as_str());
    let id = id_of(&created);

    let content = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/content"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(content.status, StatusCode::OK);
    assert_eq!(content.body, PAYLOAD);

    let stranger = seed_bridge_credential(&pool, "docs-import-stranger").await?;
    let refused = send(
        app(&state, &stranger),
        json_request(
            Method::POST,
            "/import",
            &json!({ "blob_ref": blob, "name": "stolen.txt" }),
        ),
    )
    .await?;
    assert!(
        refused.status.is_client_error(),
        "another user's blob is not importable: {}",
        refused.status
    );

    let malformed = send(
        app(&state, &owner),
        json_request(Method::POST, "/import", &json!({ "name": "x" })),
    )
    .await?;
    assert!(malformed.status.is_client_error());
    Ok(())
}

#[tokio::test]
async fn a_markdown_docx_export_unknown_media_and_strangers_are_refused() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-export").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);

    let exported = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/export/docx"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(
        exported.status,
        StatusCode::BAD_REQUEST,
        "only a docx document exports with its comments"
    );

    let stranger = seed_bridge_credential(&pool, "docs-export-stranger").await?;
    let hidden = send(
        app(&state, &stranger),
        request(Method::GET, &format!("/{id}/export/docx"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);

    let media = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/media?ref=missing.png"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert!(media.status.is_client_error(), "{}", media.status);

    let bad = send(
        app(&state, &owner),
        request(Method::GET, "/not-an-id/export/docx")
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert!(bad.status.is_client_error());
    Ok(())
}
