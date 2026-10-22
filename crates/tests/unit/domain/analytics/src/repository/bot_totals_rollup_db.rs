//! `get_bot_totals` reads the users-owned traffic rollup and agrees with the
//! `report_user_sessions` FILTER counts on a seeded matrix: a soft-deleted
//! user's sessions are excluded, a session reclassified human→bot moves
//! between classes, and a deleted session leaves the counts.

use std::sync::Arc;

use chrono::{DateTime, Duration, DurationRound, Utc};
use systemprompt_analytics::repository::TrafficAnalyticsRepository;
use systemprompt_test_fixtures::{ensure_test_bootstrap, seed_user_row, test_db_pool};
use systemprompt_users::SessionTrafficRollupRepository;
use uuid::Uuid;

struct Totals {
    human: i64,
    ghost: i64,
    bot: i64,
}

async fn report_totals(pool: &sqlx::PgPool, start: DateTime<Utc>, end: DateTime<Utc>) -> Totals {
    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT \
           COUNT(*) FILTER (WHERE is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot) AND landing_page IS NOT NULL AND request_count > 0)::bigint, \
           COUNT(*) FILTER (WHERE is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot) AND (landing_page IS NULL OR request_count = 0))::bigint, \
           COUNT(*) FILTER (WHERE NOT is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot))::bigint \
         FROM report_user_sessions WHERE started_at >= $1 AND started_at < $2",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await
    .expect("report totals");
    Totals {
        human: row.0,
        ghost: row.1,
        bot: row.2,
    }
}

async fn seed(
    pool: &sqlx::PgPool,
    user_id: Option<&str>,
    started_at: DateTime<Utc>,
    is_bot: bool,
    engaged: bool,
) -> String {
    let session_id = format!("sess-rollup-{}", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO user_sessions (session_id, user_id, started_at, is_bot, landing_page, request_count) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&session_id)
    .bind(user_id)
    .bind(started_at)
    .bind(is_bot)
    .bind(engaged.then_some("/"))
    .bind(i32::from(engaged))
    .execute(pool)
    .await
    .expect("seed session");
    session_id
}

async fn assert_agree(
    repo: &TrafficAnalyticsRepository,
    pool: &sqlx::PgPool,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    stage: &str,
) {
    let rollup = repo
        .get_bot_totals(start, end)
        .await
        .expect("rollup totals");
    let report = report_totals(pool, start, end).await;
    assert_eq!(
        (rollup.human, rollup.ghost, rollup.bot),
        (report.human, report.ghost, report.bot),
        "{stage}: rollup must equal the report view counts"
    );
}

#[tokio::test]
async fn bot_totals_match_the_report_view_through_deletion_and_reclassification() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let pg = pool.write_pool();
    let repo = TrafficAnalyticsRepository::new(
        &pool,
        Arc::new(SessionTrafficRollupRepository::new(&pool)),
    );

    // Why: the rollup is hourly, so the window is a whole, far-past hour no
    // other test seeds into.
    let start = (Utc::now() - Duration::days(3000 + i64::from(rand_offset())))
        .duration_trunc(Duration::hours(1))
        .expect("hour truncation");
    let end = start + Duration::hours(1);
    let inside = start + Duration::minutes(10);

    let keep_user =
        systemprompt_identifiers::UserId::new(format!("rollup-keep-{}", Uuid::new_v4()));
    let gone_user =
        systemprompt_identifiers::UserId::new(format!("rollup-gone-{}", Uuid::new_v4()));
    seed_user_row(
        &pool,
        &keep_user,
        &format!("{}@rollup.invalid", keep_user.as_str()),
    )
    .await
    .expect("seed keep user");
    seed_user_row(
        &pool,
        &gone_user,
        &format!("{}@rollup.invalid", gone_user.as_str()),
    )
    .await
    .expect("seed gone user");

    let human = seed(&pg, Some(keep_user.as_str()), inside, false, true).await;
    let ghost = seed(&pg, None, inside, false, false).await;
    let bot = seed(&pg, None, inside, true, true).await;
    let doomed_user_session = seed(&pg, Some(gone_user.as_str()), inside, false, true).await;
    let doomed_session = seed(&pg, None, inside, false, true).await;
    assert_agree(&repo, &pg, start, end, "after seeding").await;

    sqlx::query("UPDATE users SET status = 'deleted' WHERE id = $1")
        .bind(gone_user.as_str())
        .execute(&*pg)
        .await
        .expect("soft-delete user");
    assert_agree(&repo, &pg, start, end, "after soft-deleting a user").await;

    sqlx::query("UPDATE user_sessions SET is_behavioral_bot = true WHERE session_id = $1")
        .bind(&human)
        .execute(&*pg)
        .await
        .expect("reclassify");
    assert_agree(&repo, &pg, start, end, "after reclassifying human to bot").await;

    sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
        .bind(&doomed_session)
        .execute(&*pg)
        .await
        .expect("delete session");
    assert_agree(&repo, &pg, start, end, "after deleting a session").await;

    sqlx::query("UPDATE users SET status = 'active' WHERE id = $1")
        .bind(gone_user.as_str())
        .execute(&*pg)
        .await
        .expect("restore user");
    assert_agree(&repo, &pg, start, end, "after restoring the user").await;

    for session in [&human, &ghost, &bot, &doomed_user_session] {
        sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
            .bind(session)
            .execute(&*pg)
            .await
            .ok();
    }
    for user in [&keep_user, &gone_user] {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user.as_str())
            .execute(&*pg)
            .await
            .ok();
    }
    assert_agree(&repo, &pg, start, end, "after cleanup").await;
}

fn rand_offset() -> u16 {
    u16::try_from(Uuid::new_v4().as_u128() % 3000).unwrap_or(0)
}
