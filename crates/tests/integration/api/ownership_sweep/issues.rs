use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::json;
use systemprompt_api::routes::issues::{IssuesState, router};
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::seed_bridge_credential;
use uuid::Uuid;

use super::engine::{Fixture, get, send, sweep};
use super::table::Family;
use crate::common::{request_context, setup_ctx};

struct Issues {
    ctx: std::sync::Arc<AppContext>,
    owner: UserId,
    stranger: UserId,
    id: String,
}

fn as_user(ctx: &AppContext, user: &UserId) -> Router {
    let authed = AuthenticatedUser::new(
        user.clone(),
        "sweep-reporter".to_owned(),
        "sweep-reporter@example.invalid".to_owned(),
        vec![Permission::User],
    );
    router(IssuesState::from_context(ctx))
        .layer(Extension(request_context(user.as_str()).with_user(authed)))
}

impl Fixture for Issues {
    fn owner(&self) -> Router {
        as_user(&self.ctx, &self.owner)
    }

    fn stranger(&self) -> Router {
        as_user(&self.ctx, &self.stranger)
    }

    fn resource(&self) -> &str {
        &self.id
    }

    fn owner_view(&self) -> Request<Body> {
        get(&format!("/{}", self.id))
    }
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_issue_route() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_bridge_credential(&pool, "sweep-issue-owner")
        .await?
        .user_id;
    let stranger = seed_bridge_credential(&pool, "sweep-issue-stranger")
        .await?
        .user_id;
    let filing = json!({
        "kind": "bug",
        "severity": "medium",
        "title": "sweep",
        "body": "steps",
        "fingerprint": Uuid::new_v4().simple().to_string(),
        "client": {"os": "linux"},
    });
    let request = Request::builder()
        .method(Method::POST)
        .uri("/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(filing.to_string()))?;
    let filed = send(as_user(&ctx, &owner), request).await?;
    assert_eq!(filed.status, StatusCode::CREATED, "{}", filed.body);
    let filed: serde_json::Value = serde_json::from_str(&filed.body)?;
    let id = filed["id"].as_str().expect("issue id").to_owned();
    sweep(
        &Issues {
            ctx,
            owner,
            stranger,
            id,
        },
        Family::Issues,
    )
    .await
}
