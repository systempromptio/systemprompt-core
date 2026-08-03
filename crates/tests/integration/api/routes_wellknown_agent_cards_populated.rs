//! `/.well-known` agent-card handlers against a registry that has an agent.
//!
//! Every handler here starts by building an `AgentRegistry`, which fails under
//! the default bootstrap (empty services `config.yaml`), so the card-building
//! bodies never run. The messaging bootstrap seeds one enabled agent, which
//! puts the by-name and listing handlers on their success paths and lets the
//! by-name 404 be told apart from the registry-construction 500.

use axum::Router;
use systemprompt_api::routes::wellknown_router;
use systemprompt_test_fixtures::{
    ensure_messaging_bootstrap, fixture_app_context, fixture_db_pool, test_messaging_agent,
};
use tower::ServiceExt;

use super::common::{body_to_string, empty_get};

async fn app() -> anyhow::Result<Router> {
    let b = ensure_messaging_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    Ok(wellknown_router(&ctx))
}

#[tokio::test]
async fn a_configured_agent_has_a_card_under_its_own_name() -> anyhow::Result<()> {
    let agent = test_messaging_agent();
    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(empty_get(&format!("/.well-known/agent-cards/{agent}")))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    let card: serde_json::Value = serde_json::from_str(&body)?;
    assert!(
        body.contains(agent),
        "the card must identify the agent it was requested for: {card}"
    );
    Ok(())
}

#[tokio::test]
async fn the_json_suffix_resolves_the_same_card() -> anyhow::Result<()> {
    let agent = test_messaging_agent();
    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(empty_get(&format!("/.well-known/agent-cards/{agent}.json")))
            .await?,
    )
    .await?;

    assert_eq!(
        status.as_u16(),
        200,
        "a `.json` suffix is stripped before lookup: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn an_unknown_agent_name_is_a_404_not_a_500() -> anyhow::Result<()> {
    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(empty_get("/.well-known/agent-cards/no-such-agent"))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 404, "{body}");
    assert!(
        body.contains("no-such-agent"),
        "the 404 names the agent that was asked for: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn the_listing_returns_a_card_per_configured_agent() -> anyhow::Result<()> {
    let agent = test_messaging_agent();
    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(empty_get("/.well-known/agent-cards"))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    let cards: Vec<serde_json::Value> = serde_json::from_str(&body)?;
    assert!(
        !cards.is_empty(),
        "a configured agent must appear in the listing: {body}"
    );
    assert!(
        body.contains(agent),
        "the configured agent must be among the listed cards: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn the_default_card_route_resolves_or_reports_no_default() -> anyhow::Result<()> {
    let (status, body) = body_to_string(
        app()
            .await?
            .oneshot(empty_get("/.well-known/agent-card.json"))
            .await?,
    )
    .await?;

    // The fixture agent is not flagged default, so this route legitimately has
    // two outcomes; what it must never do is fail as a registry-construction
    // error, which is what the empty-config bootstrap produced.
    assert!(
        status.as_u16() == 200 || status.as_u16() == 404,
        "expected a card or a clean not-found, got {status}: {body}"
    );
    assert!(
        !body.contains("Failed to create agent registry"),
        "the registry must build once an agent is configured: {body}"
    );
    Ok(())
}
