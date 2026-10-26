//! `is_human_traffic()` and the canonical traffic views agree on every
//! combination of the four bot flags: a session is in `v_clean_traffic`
//! exactly when the function says it is human, and in `v_engaged_traffic`
//! exactly when it is human with a landing page and at least one request.

use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
struct Flags {
    is_bot: bool,
    is_ai_crawler: bool,
    is_scanner: bool,
    is_behavioral_bot: bool,
}

impl Flags {
    fn matrix() -> impl Iterator<Item = Self> {
        (0u8..16).map(|bits| Self {
            is_bot: bits & 1 != 0,
            is_ai_crawler: bits & 2 != 0,
            is_scanner: bits & 4 != 0,
            is_behavioral_bot: bits & 8 != 0,
        })
    }

    const fn human(self) -> bool {
        !(self.is_bot || self.is_ai_crawler || self.is_scanner || self.is_behavioral_bot)
    }
}

async fn seed(pool: &sqlx::PgPool, flags: Flags, engaged: bool) -> String {
    let session_id = format!("sess-parity-{}", Uuid::new_v4());
    let landing_page = engaged.then_some("/");
    let request_count = i32::from(engaged);
    sqlx::query(
        "INSERT INTO user_sessions (session_id, is_bot, is_ai_crawler, is_scanner, \
         is_behavioral_bot, landing_page, request_count) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&session_id)
    .bind(flags.is_bot)
    .bind(flags.is_ai_crawler)
    .bind(flags.is_scanner)
    .bind(flags.is_behavioral_bot)
    .bind(landing_page)
    .bind(request_count)
    .execute(pool)
    .await
    .expect("seed matrix row");
    session_id
}

async fn in_view(pool: &sqlx::PgPool, view: &str, session_id: &str) -> bool {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {view} WHERE session_id = $1)");
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(session_id)
        .fetch_one(pool)
        .await
        .expect("probe view")
}

async fn function_says_human(pool: &sqlx::PgPool, session_id: &str) -> bool {
    sqlx::query_scalar(
        "SELECT is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot) \
         FROM user_sessions WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .expect("evaluate predicate")
}

#[tokio::test]
async fn function_and_views_agree_on_the_full_flag_matrix() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let pg = pool.write_pool();
    let mut seeded = Vec::new();

    for flags in Flags::matrix() {
        for engaged in [false, true] {
            let session_id = seed(&pg, flags, engaged).await;
            let human = function_says_human(&pg, &session_id).await;
            assert_eq!(human, flags.human(), "{flags:?}");
            assert_eq!(
                in_view(&pg, "v_clean_traffic", &session_id).await,
                human,
                "v_clean_traffic disagrees with is_human_traffic for {flags:?}"
            );
            assert_eq!(
                in_view(&pg, "v_engaged_traffic", &session_id).await,
                human && engaged,
                "v_engaged_traffic disagrees for {flags:?} engaged={engaged}"
            );
            seeded.push(session_id);
        }
    }

    for session_id in seeded {
        sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
            .bind(&session_id)
            .execute(&*pg)
            .await
            .ok();
    }
}
