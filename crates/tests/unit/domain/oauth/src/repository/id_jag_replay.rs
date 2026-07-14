// Atomic single-use semantics of the ID-JAG jti replay store.

use chrono::{Duration, Utc};
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn repo() -> Option<OAuthRepository> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    Some(OAuthRepository::new(&pool).expect("repo"))
}

#[tokio::test]
async fn first_presentation_consumes_and_replay_is_rejected() {
    let Some(repo) = repo().await else { return };
    let jti = format!("jag-{}", Uuid::new_v4().simple());
    let expires = Utc::now() + Duration::minutes(5);

    assert!(
        repo.consume_id_jag_jti(&jti, expires).await.expect("first"),
        "first presentation must consume"
    );
    assert!(
        !repo
            .consume_id_jag_jti(&jti, expires)
            .await
            .expect("second"),
        "replay must be rejected"
    );
}

#[tokio::test]
async fn distinct_jtis_do_not_interfere() {
    let Some(repo) = repo().await else { return };
    let expires = Utc::now() + Duration::minutes(5);
    let a = format!("jag-{}", Uuid::new_v4().simple());
    let b = format!("jag-{}", Uuid::new_v4().simple());

    assert!(repo.consume_id_jag_jti(&a, expires).await.expect("a"));
    assert!(repo.consume_id_jag_jti(&b, expires).await.expect("b"));
}
