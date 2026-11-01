//! Signed upload tokens routed through the uploads gate: a token is
//! refused outside its upload and grant, and inside them carries an upload
//! from its part to its content.

use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::response::Response;
use systemprompt_api::routes::documents::DocumentsState;
use systemprompt_api::routes::uploads::{router, with_upload_tokens};
use systemprompt_identifiers::DocumentUploadId;
use systemprompt_runtime::documents::upload_links::{
    UPLOAD_AUDIENCE, UPLOAD_LINK_TTL, UPLOAD_TOKEN_HEADER, UploadGrant, upload_purpose,
};
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;

use super::common::setup_ctx;
use super::documents::mint_download;
use super::routes_uploads::{PAYLOAD, open};

fn token(
    state_ctx: &systemprompt_runtime::AppContext,
    user: &AuthedFixture,
    grant: UploadGrant,
    id: &str,
) -> String {
    let purpose = upload_purpose(grant, &DocumentUploadId::new(id));
    mint_download(
        state_ctx.token_authority().as_ref(),
        user,
        UPLOAD_AUDIENCE,
        UPLOAD_LINK_TTL,
        purpose,
    )
}

async fn gated(
    state: &DocumentsState,
    method: Method,
    uri: &str,
    header_token: Option<&str>,
    body: Body,
) -> Result<Response> {
    let app = with_upload_tokens(router(state.clone()), state.clone());
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = header_token {
        builder = builder.header(UPLOAD_TOKEN_HEADER, token);
    }
    Ok(app.oneshot(builder.body(body)?).await?)
}

#[tokio::test]
async fn a_signed_token_is_refused_outside_its_upload_and_grant() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let user = seed_bridge_credential(&pool, "uploads-token").await?;
    let id = open(&state, &user).await?;
    let other = open(&state, &user).await?;
    let write = token(&ctx, &user, UploadGrant::Write, &id);
    let download = token(&ctx, &user, UploadGrant::Download, &id);

    let wrong_grant = gated(
        &state,
        Method::GET,
        &format!("/{id}/content"),
        Some(&write),
        Body::empty(),
    )
    .await?;
    assert_eq!(wrong_grant.status(), StatusCode::FORBIDDEN);
    let wrong_upload = gated(
        &state,
        Method::GET,
        &format!("/{other}"),
        Some(&write),
        Body::empty(),
    )
    .await?;
    assert_eq!(wrong_upload.status(), StatusCode::FORBIDDEN);
    let abort = gated(
        &state,
        Method::DELETE,
        &format!("/{id}"),
        Some(&write),
        Body::empty(),
    )
    .await?;
    assert_eq!(abort.status(), StatusCode::FORBIDDEN);
    let forged = gated(
        &state,
        Method::GET,
        &format!("/{id}"),
        Some("not-a-jwt"),
        Body::empty(),
    )
    .await?;
    assert_eq!(forged.status(), StatusCode::UNAUTHORIZED);
    let bad_id = gated(
        &state,
        Method::GET,
        "/%20/content",
        Some(&download),
        Body::empty(),
    )
    .await?;
    assert!(bad_id.status().is_client_error());
    let query = gated(
        &state,
        Method::DELETE,
        &format!("/{id}?access_token={download}"),
        None,
        Body::empty(),
    )
    .await?;
    assert_eq!(query.status(), StatusCode::FORBIDDEN);
    let untokened = gated(&state, Method::GET, &format!("/{id}"), None, Body::empty()).await?;
    assert_eq!(untokened.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

async fn status_and_body(response: Response) -> Result<(StatusCode, Vec<u8>)> {
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    Ok((status, bytes.to_vec()))
}

#[tokio::test]
async fn a_signed_token_carries_an_upload_from_part_to_content() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let user = seed_bridge_credential(&pool, "uploads-token-flow").await?;
    let id = open(&state, &user).await?;
    let write = token(&ctx, &user, UploadGrant::Write, &id);
    let download = token(&ctx, &user, UploadGrant::Download, &id);

    let put = gated(
        &state,
        Method::PUT,
        &format!("/{id}/parts/1"),
        Some(&write),
        Body::from(PAYLOAD),
    )
    .await?;
    let (status, body) = status_and_body(put).await?;
    assert!(
        status.is_success(),
        "{status} {}",
        String::from_utf8_lossy(&body)
    );

    let progress = gated(
        &state,
        Method::GET,
        &format!("/{id}"),
        Some(&write),
        Body::empty(),
    )
    .await?;
    let (status, body) = status_and_body(progress).await?;
    assert_eq!(status, StatusCode::OK);
    let progress: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(progress["missing_parts"], serde_json::json!([]));

    let complete = gated(
        &state,
        Method::POST,
        &format!("/{id}/complete"),
        Some(&write),
        Body::empty(),
    )
    .await?;
    let (status, body) = status_and_body(complete).await?;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    let content = gated(
        &state,
        Method::GET,
        &format!("/{id}/content?access_token={download}"),
        None,
        Body::empty(),
    )
    .await?;
    let (status, body) = status_and_body(content).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, PAYLOAD);
    Ok(())
}
