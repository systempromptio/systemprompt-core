//! A caller whose account is deactivated is refused by both brain route
//! groups on the very next request: the handler reads the caller's roles
//! from the role directory, which answers only for an active account, so no
//! gate cache window lets a deactivated user keep brain access.

use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::json;
use systemprompt_brain_entry::http::admin_routes;
use systemprompt_identifiers::UserId;
use tower::ServiceExt;

use super::routes_facts::{brain_fixture, call, mounted, owner, read_json};

async fn deactivate(pool: &systemprompt_database::DbPool, user: &UserId) -> Result<()> {
    sqlx::query("UPDATE users SET status = 'inactive' WHERE id = $1")
        .bind(user.as_str())
        .execute(pool.pool().as_ref())
        .await?;
    Ok(())
}

#[tokio::test]
async fn a_deactivated_user_is_refused_by_the_user_routes_immediately() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-inactive").await?;
    let (status, body) = call(&fx.kernel, &me.user_id, Method::GET, "/facts", None).await?;
    assert_ne!(
        status,
        StatusCode::FORBIDDEN,
        "an active user is served: {body}"
    );

    deactivate(&fx.pool, &me.user_id).await?;
    let (status, body) = call(&fx.kernel, &me.user_id, Method::GET, "/facts", None).await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error_key"], json!("account_inactive"), "{body}");
    Ok(())
}

#[tokio::test]
async fn a_deactivated_user_is_refused_by_the_admin_routes_immediately() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-inactive-admin").await?;
    deactivate(&fx.pool, &me.user_id).await?;
    let app = mounted(&fx.kernel, &admin_routes(), &me.user_id)?;
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/simulate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"user_id": me.user_id.as_str(), "client": "claude-code", "prompt": "p"})
                        .to_string(),
                ))?,
        )
        .await?;
    let status = response.status();
    let body = read_json(response).await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error_key"], json!("account_inactive"), "{body}");
    Ok(())
}
