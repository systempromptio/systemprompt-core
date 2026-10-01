//! Deterministic coverage for the behavioural-detection input collection.
//!
//! `middleware_analytics_detection` drives the fire-and-forget middleware task;
//! this suite calls `collect_analysis_input` directly so the fingerprint-stats
//! and session-timeline query branches execute without racing a spawned task:
//! one call with a seeded session and a fingerprint (the populated paths), one
//! with an unknown session and no fingerprint (the early-return / fallback
//! paths).

use systemprompt_api::services::middleware::analytics::detection::{
    DetectionSubject, collect_analysis_input,
};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_test_fixtures::{seed_user_row, seed_user_session};

use super::common::setup_ctx;

#[tokio::test]
async fn collect_input_with_seeded_session_and_fingerprint() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let repositories = systemprompt_test_fixtures::fixture_analytics_repositories(ctx.db_pool())?;

    let user = UserId::new(format!("bd-{}", uuid::Uuid::new_v4()));
    let session = SessionId::generate();
    seed_user_row(&db, &user, &format!("{}@example.com", user.as_str())).await?;
    seed_user_session(&db, &user, &session).await?;

    let input = collect_analysis_input(
        &*repositories.session_store,
        &repositories.session_signals,
        DetectionSubject {
            session_id: session.clone(),
            fingerprint_hash: Some(format!("fp-{}", uuid::Uuid::new_v4())),
            user_agent: Some("Mozilla/5.0 test".to_owned()),
            request_count: 7,
        },
    )
    .await;

    assert_eq!(input.session_id, session);
    assert!(input.fingerprint_hash.is_some());
    Ok(())
}

#[tokio::test]
async fn collect_input_unknown_session_no_fingerprint_uses_fallbacks() -> anyhow::Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let repositories = systemprompt_test_fixtures::fixture_analytics_repositories(ctx.db_pool())?;

    let session = SessionId::generate();
    let input = collect_analysis_input(
        &*repositories.session_store,
        &repositories.session_signals,
        DetectionSubject {
            session_id: session.clone(),
            fingerprint_hash: None,
            user_agent: None,
            request_count: 3,
        },
    )
    .await;

    assert_eq!(input.session_id, session);
    assert!(input.fingerprint_hash.is_none());
    assert_eq!(input.request_count, 3);
    assert_eq!(input.fingerprint_session_count, 1);
    Ok(())
}
