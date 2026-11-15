use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::json;
use systemprompt_test_fixtures::seed_admin_credential;

use super::engine::{Fixture, get, send, sweep};
use super::table::Family;
use crate::common::{json_post, setup_ctx};
use crate::routes_agent_runs::{Runs as RunRoutes, agent_runs};

const ORIGIN: &str = "sp_test_ownership_sweep";

struct Runs {
    state: RunRoutes,
    owner: String,
    stranger: String,
    id: String,
}

impl Fixture for Runs {
    fn owner(&self) -> Router {
        self.state.as_user(&self.owner)
    }

    fn stranger(&self) -> Router {
        self.state.as_user(&self.stranger)
    }

    fn resource(&self) -> &str {
        &self.id
    }

    fn owner_view(&self) -> Request<Body> {
        get(&format!("/{}", self.id))
    }
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_agent_run_route() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_admin_credential(&pool, "sweep-run-owner").await?;
    let stranger = seed_admin_credential(&pool, "sweep-run-stranger").await?;
    let state = agent_runs(&pool, &ctx, &[ORIGIN])?;
    let body = json!({
        "origin_kind": ORIGIN,
        "idempotency_key": format!("sweep-{}", owner.user_id),
        "request": {
            "host": "opencode-cli",
            "prompt": "sweep",
            "permission_mode": "default",
            "workspace": "/tmp",
            "wall_clock": { "secs": 60, "nanos": 0 },
            "traffic": "evaluation"
        }
    });
    let created = send(
        state.as_user(owner.user_id.as_str()),
        json_post("/", body),
    )
    .await?;
    assert_eq!(created.status, StatusCode::ACCEPTED, "{}", created.body);
    let created: serde_json::Value = serde_json::from_str(&created.body)?;
    let id = created["id"].as_str().expect("run id").to_owned();
    sweep(
        &Runs {
            state,
            owner: owner.user_id.as_str().to_owned(),
            stranger: stranger.user_id.as_str().to_owned(),
            id,
        },
        Family::AgentRuns,
    )
    .await
}
