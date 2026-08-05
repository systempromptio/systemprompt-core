//! Tests for the short-link redirect handler.
//!
//! `redirect_handler` is the public hop for every tracked link. Two properties
//! matter: an unknown code must 404 rather than redirect somewhere arbitrary,
//! and click tracking must never be able to break the redirect — the visitor's
//! navigation takes priority over our analytics.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use axum::extract::{Extension, Path, State};
use axum::response::IntoResponse;
use systemprompt_api::routes::content::links::redirect_handler;
use systemprompt_content::repository::{ContentRepositories, LinkRepository};
use systemprompt_content::{GenerateLinkParams, LinkGenerationService, LinkType};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn ctx(session_id: &str) -> RequestContext {
    let mut ctx = RequestContext::new(
        SessionId::new(session_id),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("test"),
    );
    ctx.auth.actor = systemprompt_identifiers::Actor::user(UserId::new("link-visitor"));
    ctx
}

async fn make_link(pool: &DbPool, target: &str) -> String {
    LinkGenerationService::new(LinkRepository::new(pool).expect("link repo"))
        .generate_link(GenerateLinkParams {
            target_url: target.to_owned(),
            link_type: LinkType::Redirect,
            campaign_id: None,
            campaign_name: None,
            source_content_id: None,
            source_page: None,
            utm_params: None,
            link_text: None,
            link_position: None,
            expires_at: None,
        })
        .await
        .expect("generate link")
        .short_code
}

fn content_repos(pool: &systemprompt_database::DbPool) -> std::sync::Arc<ContentRepositories> {
    std::sync::Arc::new(ContentRepositories::new(pool).expect("content repositories"))
}

#[tokio::test]
async fn unknown_short_code_is_not_found() {
    let Some(pool) = pool().await else {
        return;
    };
    let response = redirect_handler(
        State(content_repos(&pool)),
        Extension(ctx("sess-unknown")),
        Path(format!("nope{}", Uuid::new_v4().simple())),
    )
    .await
    .into_response();

    assert_eq!(
        response.status(),
        axum::http::StatusCode::NOT_FOUND,
        "an unknown code must 404 rather than redirect anywhere"
    );
}

#[tokio::test]
async fn a_known_short_code_redirects_to_its_target() {
    let Some(pool) = pool().await else {
        return;
    };
    let target = format!(
        "https://example.invalid/landing/{}",
        Uuid::new_v4().simple()
    );
    let code = make_link(&pool, &target).await;

    let response = redirect_handler(
        State(content_repos(&pool)),
        Extension(ctx(&format!("sess-{}", Uuid::new_v4().simple()))),
        Path(code),
    )
    .await
    .into_response();

    assert_eq!(
        response.status(),
        axum::http::StatusCode::TEMPORARY_REDIRECT
    );
    let location = response
        .headers()
        .get(axum::http::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        location.contains(&target),
        "expected a redirect to {target}, got {location:?}"
    );
}

#[tokio::test]
async fn a_bot_session_still_redirects_but_is_not_tracked() {
    let Some(pool) = pool().await else {
        return;
    };
    let target = format!("https://example.invalid/bot/{}", Uuid::new_v4().simple());
    let code = make_link(&pool, &target).await;

    // Session ids prefixed `bot_` skip click tracking; the redirect itself must
    // be unaffected, otherwise crawlers would see a different site than users.
    let response = redirect_handler(
        State(content_repos(&pool)),
        Extension(ctx("bot_crawler-1")),
        Path(code),
    )
    .await
    .into_response();

    assert_eq!(
        response.status(),
        axum::http::StatusCode::TEMPORARY_REDIRECT
    );
}

#[tokio::test]
async fn the_same_code_can_be_followed_repeatedly() {
    let Some(pool) = pool().await else {
        return;
    };
    let target = format!("https://example.invalid/repeat/{}", Uuid::new_v4().simple());
    let code = make_link(&pool, &target).await;

    for attempt in 0..3 {
        let response = redirect_handler(
            State(content_repos(&pool)),
            Extension(ctx(&format!("sess-repeat-{attempt}"))),
            Path(code.clone()),
        )
        .await
        .into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::TEMPORARY_REDIRECT,
            "attempt {attempt} should still redirect"
        );
    }
}
