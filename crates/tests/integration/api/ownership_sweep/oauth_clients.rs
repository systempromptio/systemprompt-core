use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::authenticated_router;
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::UserType;
use systemprompt_oauth::OAuthState;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{seed_oauth_client, test_oauth_signing};
use systemprompt_traits::AppContext as _;

use super::engine::{Fixture, get, send, sweep};
use super::table::Family;
use crate::common::{request_context, setup_ctx};

struct Clients {
    ctx: std::sync::Arc<AppContext>,
    owner: UserId,
    stranger: UserId,
    id: String,
}

pub(super) fn clients_as(ctx: &AppContext, user: &UserId, user_type: UserType) -> Router {
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("users"),
        test_oauth_signing(),
    );
    let caller = request_context(user.as_str()).with_user_type(user_type);
    let inject = move |mut request: Request<Body>, next: Next| {
        let caller = caller.clone();
        async move {
            request.extensions_mut().insert(caller);
            next.run(request).await
        }
    };
    authenticated_router()
        .layer(middleware::from_fn(inject))
        .with_state(state)
        .layer(axum::Extension(ctx.config_arc().clone()))
}

impl Fixture for Clients {
    fn owner(&self) -> Router {
        clients_as(&self.ctx, &self.owner, UserType::User)
    }

    fn stranger(&self) -> Router {
        clients_as(&self.ctx, &self.stranger, UserType::User)
    }

    fn resource(&self) -> &str {
        &self.id
    }

    fn owner_view(&self) -> Request<Body> {
        get(&format!("/clients/{}", self.id))
    }
}

pub(super) async fn seed_users(pool: &systemprompt_database::DbPool) -> Result<(UserId, UserId)> {
    let mut users = Vec::new();
    for label in ["sweep-client-owner", "sweep-client-stranger"] {
        let user = systemprompt_test_fixtures::unique_user_id(label);
        systemprompt_test_fixtures::seed_user_row(
            pool,
            &user,
            &format!("{}@sweep.invalid", user.as_str()),
        )
        .await?;
        users.push(user);
    }
    let stranger = users.pop().expect("stranger");
    Ok((users.pop().expect("owner"), stranger))
}

#[tokio::test]
async fn a_second_user_cannot_read_change_or_delete_another_users_oauth_client() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (owner, stranger) = seed_users(&pool).await?;
    let client = seed_oauth_client(&pool, &owner).await?;
    sweep(
        &Clients {
            ctx,
            owner,
            stranger,
            id: client.client_id.as_str().to_owned(),
        },
        Family::OAuthClients,
    )
    .await
}

#[tokio::test]
async fn an_admin_reads_another_users_oauth_client_and_an_owner_lists_only_its_own() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (owner, admin) = seed_users(&pool).await?;
    let client = seed_oauth_client(&pool, &owner).await?;
    let id = client.client_id.as_str();
    let read = send(
        clients_as(&ctx, &admin, UserType::Admin),
        get(&format!("/clients/{id}")),
    )
    .await?;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
    let read: serde_json::Value = serde_json::from_str(&read.body)?;
    assert_eq!(read["data"]["client_id"], id);
    let own = send(clients_as(&ctx, &owner, UserType::User), get("/clients")).await?;
    assert_eq!(own.status, StatusCode::OK, "{}", own.body);
    let own: serde_json::Value = serde_json::from_str(&own.body)?;
    let ids: Vec<&str> = own["data"]
        .as_array()
        .expect("data")
        .iter()
        .filter_map(|c| c["client_id"].as_str())
        .collect();
    assert_eq!(ids, vec![id], "an owner lists exactly its own clients");
    assert_eq!(own["meta"]["pagination"]["total"], 1);
    Ok(())
}
