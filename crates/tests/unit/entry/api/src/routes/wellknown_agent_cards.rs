//! `/.well-known` agent-card endpoints against an empty agent registry.
//!
//! The fixture services tree declares no agents, which is the shape every one
//! of these three handlers has to survive: a card asked for by name, the
//! default card, and the listing. Two of them must refuse, and the listing must
//! answer with an empty collection rather than an error — a deployment with no
//! agents yet is not a broken deployment.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_api::routes::wellknown::agent_cards::wellknown_router;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};
use tower::ServiceExt;

async fn router() -> axum::Router {
    let boot = ensure_test_bootstrap();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("test database");
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");
    wellknown_router(&ctx)
}

async fn get(uri: &str) -> axum::http::Response<Body> {
    router()
        .await
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("router answers")
}

#[tokio::test]
async fn a_card_asked_for_by_a_name_no_agent_holds_is_a_not_found() {
    let response = get("/.well-known/agent-cards/no-such-agent").await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "an unknown agent name must not resolve to some other agent's card"
    );
}

#[tokio::test]
async fn the_json_suffix_is_stripped_before_the_lookup() {
    let response = get("/.well-known/agent-cards/no-such-agent.json").await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "`.json` is a representation suffix, not part of the agent name"
    );
}

#[tokio::test]
async fn the_default_card_is_a_not_found_when_no_agent_is_configured() {
    let response = get("/.well-known/agent-card.json").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn listing_cards_with_no_agents_configured_is_an_empty_list_not_an_error() {
    let response = get("/.well-known/agent-cards").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body reads");
    let cards: serde_json::Value = serde_json::from_slice(&body).expect("body is JSON");
    assert_eq!(
        cards,
        serde_json::json!([]),
        "an empty registry lists no cards"
    );
}
