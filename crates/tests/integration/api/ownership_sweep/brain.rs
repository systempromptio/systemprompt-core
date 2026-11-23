use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::Request;
use axum::{Extension, Router};
use systemprompt_brain_entry::http::user_routes;
use systemprompt_extension::DynNotificationSink;
use systemprompt_identifiers::UserId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::FactStatus;
use systemprompt_runtime::Kernel;
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_test_fixtures::{fixture_brain_kernel, seed_brain_fact, seed_bridge_credential};
use uuid::Uuid;

use super::engine::{Fixture, get, sweep};
use super::table::Family;
use crate::common::{request_context, setup_ctx};

struct Brain {
    kernel: Kernel,
    owner: UserId,
    stranger: UserId,
    fact: String,
    subject: String,
}

fn as_user(kernel: &Kernel, user: &UserId) -> Router {
    user_routes()
        .materialize(&kernel.resolved())
        .expect("the brain route state resolves")
        .into_iter()
        .fold(Router::new(), Router::merge)
        .layer(Extension(
            request_context(user.as_str()).with_browser_session(),
        ))
}

impl Fixture for Brain {
    fn owner(&self) -> Router {
        as_user(&self.kernel, &self.owner)
    }

    fn stranger(&self) -> Router {
        as_user(&self.kernel, &self.stranger)
    }

    fn resource(&self) -> &str {
        &self.fact
    }

    fn owner_view(&self) -> Request<Body> {
        get(&format!("/facts/{}", self.fact))
    }

    fn absent(&self) -> String {
        format!("m_{}", Uuid::new_v4())
    }

    fn primary_for(&self, template: &str) -> (String, String) {
        if template.starts_with("/subjects/") {
            (
                self.subject.clone(),
                format!("service/svc-{}", Uuid::new_v4().simple()),
            )
        } else {
            (self.fact.clone(), self.absent())
        }
    }
}

#[tokio::test]
async fn a_second_user_is_refused_every_route_on_a_personal_fact() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let kernel = fixture_brain_kernel(
        &pool,
        Some(ctx.documents()),
        notifications,
        BrainProfile {
            enabled: true,
            ..BrainProfile::default()
        },
    );
    let owner_user = seed_bridge_credential(&pool, "sweep-brain-owner")
        .await?
        .user_id;
    let stranger = seed_bridge_credential(&pool, "sweep-brain-stranger")
        .await?
        .user_id;
    let service = format!("svc-{}", Uuid::new_v4().simple());
    let fact = seed_brain_fact(
        &pool,
        &owner_user,
        FactStatus::Confirmed,
        &format!("service:{service}"),
    )
    .await?;
    sweep(
        &Brain {
            kernel,
            owner: owner_user,
            stranger,
            fact: fact.as_str().to_owned(),
            subject: format!("service/{service}"),
        },
        Family::Brain,
    )
    .await
}
