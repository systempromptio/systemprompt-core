//! Behavioural-detection input collection when the queries behind it fail.
//!
//! This runs in a detached task after the response has already been sent, so
//! it has no caller to return an error to: every lookup falls back to a
//! neutral value instead. That makes the fallbacks load-bearing — a wrong
//! default here does not surface as a failed request, it silently scores a
//! real visitor as a bot (or a bot as a visitor). A closed pool fails every
//! lookup at once, which is the shape of the outage the fallbacks exist for.

use std::sync::Arc;

use systemprompt_analytics::SessionRepository;
use systemprompt_api::services::middleware::analytics::test_api::collect_analysis_input;
use systemprompt_identifiers::SessionId;
use systemprompt_test_fixtures::{closed_db_pool, ensure_test_bootstrap, fixture_db_pool};

async fn dead_repo() -> Arc<SessionRepository> {
    let pool = closed_db_pool().await;
    Arc::new(SessionRepository::new(&pool).expect("repository construction is not a query"))
}

async fn live_repo() -> Arc<SessionRepository> {
    let boot = ensure_test_bootstrap();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("test database");
    Arc::new(SessionRepository::new(&pool).expect("repository"))
}

#[tokio::test]
async fn every_lookup_failing_yields_a_neutral_input_rather_than_no_analysis() {
    let repo = dead_repo().await;
    let session_id = SessionId::generate();

    let input = collect_analysis_input(
        &repo,
        session_id.clone(),
        Some("fp-unreachable".to_owned()),
        Some("curl/8".to_owned()),
        7,
    )
    .await;

    assert_eq!(input.session_id, session_id);
    assert!(
        input.endpoints_accessed.is_empty() && input.request_timestamps.is_empty(),
        "a failed history lookup must read as no history, never as fabricated history"
    );
    assert_eq!(
        input.total_site_pages, 100,
        "the page-count fallback stands in for the site size the coverage ratio divides by; zero \
         would make every session look exhaustive"
    );
    assert!(
        !input.has_javascript_events,
        "an unreadable events table must not be taken as evidence of a real browser"
    );
    assert_eq!(
        input.fingerprint_session_count, 1,
        "the session being analysed is itself one session"
    );
    assert_eq!(input.fingerprint_unique_ip_count, 0);
    assert_eq!(input.fingerprint_engagement_event_count, 0);
    assert!(input.fingerprint_session_starts.is_empty());
}

// Why: no fingerprint is the ordinary case for a first request, not a failure,
// and it must not cost four queries to establish that.
#[tokio::test]
async fn a_request_carrying_no_fingerprint_skips_the_fingerprint_queries_entirely() {
    let repo = dead_repo().await;

    let input = collect_analysis_input(&repo, SessionId::generate(), None, None, 1).await;

    assert!(input.fingerprint_hash.is_none());
    assert_eq!(input.fingerprint_session_count, 1);
    assert!(input.fingerprint_session_starts.is_empty());
}

#[tokio::test]
async fn a_session_with_no_row_is_timed_from_the_request_count_it_was_given() {
    let repo = live_repo().await;

    let input = collect_analysis_input(&repo, SessionId::generate(), None, None, 42).await;

    assert_eq!(
        input.request_count, 42,
        "with no session row to read, the caller's count is the only truth available"
    );
    assert!(
        input.last_activity_at >= input.started_at,
        "a synthesised timeline must not run backwards"
    );
    assert!(input.landing_page.is_none() && input.entry_url.is_none());
}
