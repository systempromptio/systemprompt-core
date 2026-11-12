use anyhow::Result;
use axum::body::Body;
use axum::http::Request;
use axum::{Extension, Router};
use systemprompt_api::routes::brain::{BrainRoutes, router};
use systemprompt_identifiers::UserId;
use systemprompt_models::brain::FactStatus;
use uuid::Uuid;

use super::engine::{Fixture, get, sweep};
use super::table::Family;
use crate::brain_routes_facts::{brain_fixture, owner, seed_fact_on};
use crate::common::request_context;

struct Brain {
    routes: BrainRoutes,
    owner: UserId,
    stranger: UserId,
    fact: String,
    subject: String,
}

fn as_user(routes: &BrainRoutes, user: &UserId) -> Router {
    router(Some(routes.clone())).layer(Extension(
        request_context(user.as_str()).with_browser_session(),
    ))
}

impl Fixture for Brain {
    fn owner(&self) -> Router {
        as_user(&self.routes, &self.owner)
    }

    fn stranger(&self) -> Router {
        as_user(&self.routes, &self.stranger)
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
    let fx = brain_fixture().await?;
    let owner_user = owner(&fx.pool, "sweep-brain-owner").await?.user_id;
    let stranger = owner(&fx.pool, "sweep-brain-stranger").await?.user_id;
    let service = format!("svc-{}", Uuid::new_v4().simple());
    let fact = seed_fact_on(
        &fx.pool,
        &owner_user,
        FactStatus::Confirmed,
        &format!("service:{service}"),
    )
    .await?;
    sweep(
        &Brain {
            routes: fx.routes,
            owner: owner_user,
            stranger,
            fact: fact.as_str().to_owned(),
            subject: format!("service/{service}"),
        },
        Family::Brain,
    )
    .await
}
