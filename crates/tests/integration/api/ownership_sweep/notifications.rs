use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use serde_json::json;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_api::routes::notifications::{NotificationsState, router};
use systemprompt_identifiers::{NotificationStreamId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::notifications::{AudienceSubject, StreamAttention, StreamDecl};
use systemprompt_notifications::repository::NotificationStreamRepository;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    AuthedFixture, seed_bridge_credential, seed_user_row, unique_user_id,
};
use uuid::Uuid;

use super::engine::{Fixture, get, send, sweep, with_bearer};
use super::table::Family;
use crate::common::{json_post, request_context, setup_ctx};

struct Notifications {
    ctx: Arc<AppContext>,
    owner: UserId,
    stranger: UserId,
    id: String,
    view: &'static str,
}

fn as_user(ctx: &AppContext, user: &UserId) -> Router {
    router(NotificationsState::from_context(ctx)).layer(Extension(
        request_context(user.as_str()).with_user_type(UserType::User),
    ))
}

impl Fixture for Notifications {
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
        get(self.view)
    }
}

async fn users(ctx: &AppContext) -> Result<(UserId, UserId)> {
    let mut out = Vec::new();
    for label in ["sweep-notify-owner", "sweep-notify-stranger"] {
        let user = unique_user_id(label);
        seed_user_row(
            ctx.db_pool(),
            &user,
            &format!("{}@sweep.invalid", user.as_str()),
        )
        .await?;
        out.push(user);
    }
    let stranger = out.pop().expect("stranger");
    let owner = out.pop().expect("owner");
    Ok((owner, stranger))
}

async fn notify(ctx: &AppContext, recipient: &UserId) -> Result<String> {
    let action = json!({
        "prompt": "sweep",
        "open_url": null,
        "document_id": null,
        "comment_id": null,
        "kind": "document.shared",
        "actor": recipient.as_str(),
    });
    Ok(sqlx::query_scalar::<_, String>(
        "INSERT INTO notifications (recipient_user_id, kind, actor_user_id, actor_is_agent, \
         subjects, attention, attention_reason, action, coalesce_key, first_event_at, \
         last_event_at, outbox_id, delivery_state) VALUES ($1, 'document.shared', $1, FALSE, \
         '[]'::jsonb, 'agent', 'default', $2, gen_random_uuid()::text, now(), now(), 'sweep', \
         'delivered') RETURNING id::text",
    )
    .bind(recipient.as_str())
    .bind(action)
    .fetch_one(ctx.db_pool().write_pool().as_ref())
    .await?)
}

async fn stream(ctx: &AppContext, owner: &UserId) -> Result<NotificationStreamId> {
    let id = NotificationStreamId::try_new(format!("sweep-{}", Uuid::new_v4().simple()))?;
    let decl = StreamDecl {
        id: id.clone(),
        name: "Sweep stream".to_owned(),
        description: None,
        audience: vec![AudienceSubject::User(owner.clone())],
        publishers: vec![AudienceSubject::User(owner.clone())],
        attention: StreamAttention::default(),
        retention_days: None,
        coalesce_window_secs: None,
    };
    let repos = ctx.notifications().service.repositories();
    NotificationStreamRepository::upsert(repos.write_pool(), &decl, "test").await?;
    Ok(id)
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_notification_route() -> Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let (owner, stranger) = users(&ctx).await?;
    let id = notify(&ctx, &owner).await?;
    sweep(
        &Notifications {
            ctx,
            owner,
            stranger,
            id,
            view: "/",
        },
        Family::Notifications,
    )
    .await
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_subscription_route() -> Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let (owner, stranger) = users(&ctx).await?;
    let id = subscribe(&ctx, &owner).await?;
    sweep(
        &Notifications {
            ctx,
            owner,
            stranger,
            id,
            view: "/subscriptions",
        },
        Family::Subscriptions,
    )
    .await
}

struct BridgeNotifications {
    ctx: Arc<AppContext>,
    gateway: Router,
    owner: AuthedFixture,
    stranger: AuthedFixture,
    notification: String,
    subscription: String,
}

impl Fixture for BridgeNotifications {
    fn owner(&self) -> Router {
        with_bearer(self.gateway.clone(), self.owner.jwt.as_str())
    }

    fn stranger(&self) -> Router {
        with_bearer(self.gateway.clone(), self.stranger.jwt.as_str())
    }

    fn resource(&self) -> &str {
        &self.notification
    }

    fn owner_view(&self) -> Request<Body> {
        get("/")
    }

    fn extra_views(&self) -> Vec<Request<Body>> {
        vec![get("/subscriptions")]
    }

    fn view_router(&self) -> Router {
        as_user(&self.ctx, &self.owner.user_id)
    }

    fn primary_for(&self, template: &str) -> (String, String) {
        if template.contains("/subscriptions") {
            (self.subscription.clone(), self.absent())
        } else {
            (self.notification.clone(), self.absent())
        }
    }
}

async fn subscribe(ctx: &AppContext, owner: &UserId) -> Result<String> {
    let stream = stream(ctx, owner).await?;
    let body = json!({
        "subject": format!("user:{owner}"),
        "target": {"stream": stream.as_str()}
    });
    let created = send(as_user(ctx, owner), json_post("/subscriptions", body)).await?;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let created: serde_json::Value = serde_json::from_str(&created.body)?;
    Ok(created["id"].as_str().expect("subscription id").to_owned())
}

#[tokio::test]
async fn a_second_bridge_user_is_refused_every_owned_bridge_notification_route() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let gateway = gateway_router(&ctx)?;
    let owner = seed_bridge_credential(&pool, "sweep-bridge-notify-owner").await?;
    let stranger = seed_bridge_credential(&pool, "sweep-bridge-notify-stranger").await?;
    let notification = notify(&ctx, &owner.user_id).await?;
    let subscription = subscribe(&ctx, &owner.user_id).await?;
    sweep(
        &BridgeNotifications {
            ctx,
            gateway,
            owner,
            stranger,
            notification,
            subscription,
        },
        Family::BridgeNotifications,
    )
    .await
}
