//! Every `BehavioralBotReason` the repository writes lands on the row as the
//! enum's string and score; the ghost and no-JavaScript sweeps stamp theirs
//! only on candidate web sessions.

use systemprompt_database::DbPool;
use systemprompt_identifiers::SessionId;
use systemprompt_models::analytics::BehavioralBotReason;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_users::SessionRepository;
use uuid::Uuid;

use super::session_support::{delete_session, seed_session, unique_session_id};

struct StampedRow {
    is_behavioral_bot: bool,
    reason: Option<String>,
    score: i32,
}

async fn stamped(pool: &DbPool, session_id: &SessionId) -> StampedRow {
    let row: (bool, Option<String>, i32) = sqlx::query_as(
        "SELECT is_behavioral_bot, behavioral_bot_reason, behavioral_bot_score \
         FROM user_sessions WHERE session_id = $1",
    )
    .bind(session_id.as_str())
    .fetch_one(&*pool.write_pool())
    .await
    .expect("read stamped row");
    StampedRow {
        is_behavioral_bot: row.0,
        reason: row.1,
        score: row.2,
    }
}

async fn age_session(pool: &DbPool, session_id: &SessionId, minutes: i32, request_count: i32) {
    sqlx::query(
        "UPDATE user_sessions SET started_at = NOW() - make_interval(mins => $2), \
         request_count = $3 WHERE session_id = $1",
    )
    .bind(session_id.as_str())
    .bind(minutes)
    .bind(request_count)
    .execute(&*pool.write_pool())
    .await
    .expect("age session");
}

#[tokio::test]
async fn each_reason_writes_its_own_string_and_score() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = SessionRepository::new(&pool);

    for reason in BehavioralBotReason::ALL {
        let sid = unique_session_id();
        seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;

        repo.mark_as_behavioral_bot(&sid, reason)
            .await
            .expect("mark reason");

        let row = stamped(&pool, &sid).await;
        assert!(row.is_behavioral_bot, "{reason} must set the flag");
        assert_eq!(row.reason.as_deref(), Some(reason.as_str()));
        assert_eq!(row.score, reason.score(), "{reason} score");

        delete_session(&pool, &sid).await;
    }
}

#[tokio::test]
async fn request_count_threshold_stamps_request_count_exceeded() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = SessionRepository::new(&pool);
    let sid = unique_session_id();
    seed_session(&repo, &sid, &format!("fp-{}", Uuid::new_v4())).await;
    age_session(&pool, &sid, 1, 10).await;

    assert!(
        repo.check_and_mark_behavioral_bot(&sid, 3)
            .await
            .expect("threshold check")
    );
    let row = stamped(&pool, &sid).await;
    let reason = BehavioralBotReason::RequestCountExceeded;
    assert_eq!(row.reason.as_deref(), Some(reason.as_str()));
    assert_eq!(row.score, reason.score());

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn ghost_sweep_stamps_ghost_session_on_a_stale_empty_web_session() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = SessionRepository::new(&pool);
    let ghost = unique_session_id();
    seed_session(&repo, &ghost, &format!("fp-{}", Uuid::new_v4())).await;
    age_session(&pool, &ghost, 20, 0).await;
    let fresh = unique_session_id();
    seed_session(&repo, &fresh, &format!("fp-{}", Uuid::new_v4())).await;

    repo.mark_ghost_sessions_as_bots()
        .await
        .expect("ghost sweep");

    let reason = BehavioralBotReason::GhostSession;
    let row = stamped(&pool, &ghost).await;
    assert!(row.is_behavioral_bot);
    assert_eq!(row.reason.as_deref(), Some(reason.as_str()));
    assert_eq!(row.score, reason.score());
    assert!(
        !stamped(&pool, &fresh).await.is_behavioral_bot,
        "a session younger than five minutes is not judged"
    );

    delete_session(&pool, &ghost).await;
    delete_session(&pool, &fresh).await;
}

#[tokio::test]
async fn no_js_sweep_stamps_no_javascript_and_skips_engaged_sessions() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = SessionRepository::new(&pool);
    let silent = unique_session_id();
    seed_session(&repo, &silent, &format!("fp-{}", Uuid::new_v4())).await;
    age_session(&pool, &silent, 20, 4).await;
    let engaged = unique_session_id();
    seed_session(&repo, &engaged, &format!("fp-{}", Uuid::new_v4())).await;
    age_session(&pool, &engaged, 20, 4).await;

    repo.mark_no_js_sessions_as_bots(std::slice::from_ref(&engaged))
        .await
        .expect("no-js sweep");

    let reason = BehavioralBotReason::NoJavascript;
    let row = stamped(&pool, &silent).await;
    assert!(row.is_behavioral_bot);
    assert_eq!(row.reason.as_deref(), Some(reason.as_str()));
    assert_eq!(row.score, reason.score());
    assert!(
        !stamped(&pool, &engaged).await.is_behavioral_bot,
        "a session the caller reports as engaged is excluded"
    );

    delete_session(&pool, &silent).await;
    delete_session(&pool, &engaged).await;
}
