//! `user_sessions.session_source` defaults to the value `SessionSource`
//! names as its own default: a row inserted without a source reads back as
//! `unknown`, never `web`.

use systemprompt_identifiers::SessionSource;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use uuid::Uuid;

#[tokio::test]
async fn a_session_inserted_without_a_source_reads_back_as_the_enum_default() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let pg = pool.write_pool();
    let session_id = format!("sess-default-{}", Uuid::new_v4());

    sqlx::query("INSERT INTO user_sessions (session_id) VALUES ($1)")
        .bind(&session_id)
        .execute(&*pg)
        .await
        .expect("insert without session_source");

    let stored: Option<String> =
        sqlx::query_scalar("SELECT session_source FROM user_sessions WHERE session_id = $1")
            .bind(&session_id)
            .fetch_one(&*pg)
            .await
            .expect("read session_source");

    assert_eq!(stored.as_deref(), Some(SessionSource::default().as_str()));
    assert_eq!(stored.as_deref(), Some("unknown"));

    sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
        .bind(&session_id)
        .execute(&*pg)
        .await
        .ok();
}
