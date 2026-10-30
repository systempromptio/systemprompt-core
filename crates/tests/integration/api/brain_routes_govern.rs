//! `/api/v1/brain` lifecycle actions, dispute resolution, the proposal
//! queue and the admin simulator, driven through the brain routers.

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::brain::admin_router;
use systemprompt_identifiers::{BrainProposalId, UserId};
use systemprompt_models::brain::FactStatus;
use systemprompt_test_fixtures::credential::seed_user_row_with_roles;
use tower::ServiceExt;

use super::brain_routes_facts::{brain_fixture, call, owner, seed_personal_fact};
use super::common::{read_json, request_context};

#[tokio::test]
async fn the_owner_confirms_pins_edits_and_retracts_a_personal_fact() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-govern").await?;
    let id = seed_personal_fact(&fx.pool, &me.user_id, FactStatus::Proposed).await?;
    let user = &me.user_id;

    let (status, confirmed) = call(
        &fx.routes,
        user,
        Method::POST,
        &format!("/facts/{id}/confirm"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    assert_eq!(confirmed["revision"], json!(2));

    let (status, pinned) = call(
        &fx.routes,
        user,
        Method::POST,
        &format!("/facts/{id}/pin"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{pinned}");
    let (status, unpinned) = call(
        &fx.routes,
        user,
        Method::POST,
        &format!("/facts/{id}/unpin"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{unpinned}");

    let fields = json!({"fields": {"owner": "platform"}});
    let (status, edited) = call(
        &fx.routes,
        user,
        Method::PATCH,
        &format!("/facts/{id}/fields"),
        Some(fields),
    )
    .await?;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{edited}");
    let (_, fact) = call(&fx.routes, user, Method::GET, &format!("/facts/{id}"), None).await?;
    assert_eq!(fact["fields"]["owner"], json!("billing"), "{fact}");

    let (status, retracted) = call(
        &fx.routes,
        user,
        Method::POST,
        &format!("/facts/{id}/retract"),
        Some(json!({"reason": "superseded"})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{retracted}");
    let (_, timeline) = call(
        &fx.routes,
        user,
        Method::GET,
        &format!("/facts/{id}/timeline"),
        None,
    )
    .await?;
    assert!(timeline.as_array().map_or(0, Vec::len) >= 4, "{timeline}");
    Ok(())
}

#[tokio::test]
async fn a_stranger_may_not_govern_a_personal_fact() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-govern-owner").await?;
    let other = owner(&fx.pool, "brain-govern-other").await?;
    let id = seed_personal_fact(&fx.pool, &me.user_id, FactStatus::Proposed).await?;

    for action in ["confirm", "pin", "unpin", "erase"] {
        let (status, body) = call(
            &fx.routes,
            &other.user_id,
            Method::POST,
            &format!("/facts/{id}/{action}"),
            None,
        )
        .await?;
        assert_eq!(status, StatusCode::FORBIDDEN, "{action}: {body}");
    }
    let (_, fact) = call(
        &fx.routes,
        &me.user_id,
        Method::GET,
        &format!("/facts/{id}"),
        None,
    )
    .await?;
    assert_eq!(fact["status"], json!("proposed"), "{fact}");
    Ok(())
}

#[tokio::test]
async fn a_dispute_is_opened_resolved_and_an_erased_fact_is_gone() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-dispute").await?;
    let id = seed_personal_fact(&fx.pool, &me.user_id, FactStatus::Confirmed).await?;
    let user = &me.user_id;

    let (status, created) = call(
        &fx.routes,
        user,
        Method::POST,
        &format!("/facts/{id}/dispute"),
        Some(json!({"reason": "billing moved teams"})),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let dispute = created["dispute_id"]
        .as_str()
        .context("dispute id")?
        .to_owned();

    let resolve = json!({"fact_id": id.as_str(), "outcome": "dismissed", "note": "still true"});
    let (status, body) = call(
        &fx.routes,
        user,
        Method::POST,
        &format!("/disputes/{dispute}/resolve"),
        Some(resolve),
    )
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let (status, body) = call(
        &fx.routes,
        user,
        Method::POST,
        &format!("/facts/{id}/erase"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, _) = call(&fx.routes, user, Method::GET, &format!("/facts/{id}"), None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn only_an_admin_reads_and_decides_proposals() -> Result<()> {
    let fx = brain_fixture().await?;
    let person = owner(&fx.pool, "brain-proposals-person").await?;
    let admin = UserId::generate();
    let email = format!("{}@brain-admin.test", admin.as_str());
    seed_user_row_with_roles(&fx.pool, &admin, &email, &["admin".to_owned()]).await?;
    let missing = BrainProposalId::generate();

    let (status, _) = call(&fx.routes, &person.user_id, Method::GET, "/proposals", None).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &fx.routes,
        &person.user_id,
        Method::POST,
        &format!("/proposals/{missing}/approve"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, listed) =
        call(&fx.routes, &admin, Method::GET, "/proposals?limit=5", None).await?;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert!(listed.is_array(), "{listed}");
    for action in ["approve", "reject"] {
        let (status, body) = call(
            &fx.routes,
            &admin,
            Method::POST,
            &format!("/proposals/{missing}/{action}"),
            None,
        )
        .await?;
        assert_eq!(status, StatusCode::NOT_FOUND, "{action}: {body}");
    }
    let window = json!({"since": "2026-01-01T00:00:00Z", "until": "2026-02-01T00:00:00Z"});
    let (status, body) = call(
        &fx.routes,
        &admin,
        Method::POST,
        &format!("/proposals/{missing}/replay"),
        Some(window),
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    Ok(())
}

async fn simulate(
    fx_routes: &systemprompt_api::routes::brain::BrainRoutes,
    user: &UserId,
    body: Value,
) -> Result<(StatusCode, Value)> {
    let app: Router =
        admin_router(Some(fx_routes.clone())).layer(Extension(request_context(user.as_str())));
    let request = Request::builder()
        .method(Method::POST)
        .uri("/simulate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))?;
    let response = app.oneshot(request).await?;
    let status = response.status();
    Ok((status, read_json(response).await?))
}

#[tokio::test]
async fn the_admin_simulator_answers_a_descriptor_for_a_principal() -> Result<()> {
    let fx = brain_fixture().await?;
    let me = owner(&fx.pool, "brain-simulate").await?;
    seed_personal_fact(&fx.pool, &me.user_id, FactStatus::Confirmed).await?;

    for reuse in [false, true] {
        let body = json!({
            "user_id": me.user_id.as_str(),
            "client": "claude-code",
            "project": "core",
            "prompt": "who owns the deploy gate for billing",
            "reuse": reuse,
        });
        let (status, simulated) = simulate(&fx.routes, &me.user_id, body).await?;
        assert_eq!(status, StatusCode::OK, "{simulated}");
        assert!(simulated["descriptor"].is_string(), "{simulated}");
        assert!(
            simulated.get("block").is_some() != simulated.get("skipped").is_some(),
            "{simulated}"
        );
    }
    let (status, _) = simulate(&fx.routes, &me.user_id, json!({"user_id": "x"})).await?;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    Ok(())
}
