// DB-backed bridge per-host preference tests (upsert + list-enabled).

use systemprompt_oauth::repository::BridgeHostPrefsRepository;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

#[tokio::test]
async fn upsert_then_list_enabled() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = BridgeHostPrefsRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("bhp");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");

    let host_a = format!("host-{}", Uuid::new_v4().simple());
    let host_b = format!("host-{}", Uuid::new_v4().simple());

    repo.upsert(&user_id, &host_a, true)
        .await
        .expect("enable a");
    repo.upsert(&user_id, &host_b, false)
        .await
        .expect("disable b");

    let enabled = repo.list_enabled(&user_id).await.expect("list");
    assert!(enabled.contains(&host_a));
    assert!(!enabled.contains(&host_b));
}

#[tokio::test]
async fn upsert_toggles_enabled_flag() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = BridgeHostPrefsRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("bhp");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");

    let host = format!("host-{}", Uuid::new_v4().simple());
    repo.upsert(&user_id, &host, true).await.expect("enable");
    assert!(
        repo.list_enabled(&user_id)
            .await
            .expect("list")
            .contains(&host)
    );

    repo.upsert(&user_id, &host, false).await.expect("disable");
    assert!(
        !repo
            .list_enabled(&user_id)
            .await
            .expect("list")
            .contains(&host)
    );
}

#[tokio::test]
async fn list_enabled_empty_for_unknown_user() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = BridgeHostPrefsRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("bhp-unknown");
    let enabled = repo.list_enabled(&user_id).await.expect("list");
    assert!(enabled.is_empty());
}
