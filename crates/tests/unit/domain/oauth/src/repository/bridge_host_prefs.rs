// DB-backed bridge per-host preference tests (upsert + list-enabled).

use systemprompt_models::bridge::host::HostKind;
use systemprompt_oauth::OauthError;
use systemprompt_oauth::repository::BridgeHostPrefsRepository;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use systemprompt_traits::RepositoryError;

#[tokio::test]
async fn upsert_then_list_enabled() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");

    let host_a = HostKind::ClaudeCode;
    let host_b = HostKind::CodexCli;

    repo.upsert(&user_id, host_a, true).await.expect("enable a");
    repo.upsert(&user_id, host_b, false)
        .await
        .expect("disable b");

    let enabled = repo.list_enabled(&user_id).await.expect("list");
    assert!(enabled.contains(&host_a));
    assert!(!enabled.contains(&host_b));
}

#[tokio::test]
async fn upsert_toggles_enabled_flag() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");

    let host = HostKind::Hermes;
    repo.upsert(&user_id, host, true).await.expect("enable");
    assert!(
        repo.list_enabled(&user_id)
            .await
            .expect("list")
            .contains(&host)
    );

    repo.upsert(&user_id, host, false).await.expect("disable");
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
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp-unknown");
    let enabled = repo.list_enabled(&user_id).await.expect("list");
    assert!(enabled.is_empty());
}

#[tokio::test]
async fn model_protocols_set_load_and_clear() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp-mp");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");

    let host = HostKind::ClaudeDesktop;

    // Absent override: not present in the loaded map.
    assert!(
        repo.load_model_protocols(&user_id)
            .await
            .expect("load")
            .is_empty()
    );

    // Set a concrete list.
    repo.set_model_protocols(&user_id, host, Some(&["anthropic".to_owned()]))
        .await
        .expect("set list");
    let loaded = repo.load_model_protocols(&user_id).await.expect("load");
    assert_eq!(loaded, vec![(host, vec!["anthropic".to_owned()])]);

    // Empty list means "all models" — still a present override (distinct from
    // absent).
    repo.set_model_protocols(&user_id, host, Some(&[]))
        .await
        .expect("set all");
    let loaded = repo.load_model_protocols(&user_id).await.expect("load");
    assert_eq!(loaded, vec![(host, Vec::<String>::new())]);

    // Clearing removes the row entirely.
    repo.set_model_protocols(&user_id, host, None)
        .await
        .expect("clear");
    assert!(
        repo.load_model_protocols(&user_id)
            .await
            .expect("load")
            .is_empty()
    );
}

#[tokio::test]
async fn model_protocols_do_not_perturb_enabled_state() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp-iso");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");

    let host = HostKind::OpenCode;

    // Setting a model filter must not create an enable-pref row (which would
    // flip the "no rows means all hosts enabled" heuristic).
    repo.set_model_protocols(&user_id, host, Some(&["openai-chat".to_owned()]))
        .await
        .expect("set filter");
    assert!(
        repo.list_enabled(&user_id).await.expect("list").is_empty(),
        "model-filter override must not register an enable-state row"
    );
}

#[tokio::test]
async fn an_unknown_stored_host_is_a_decode_error() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp-unknown-host");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");
    sqlx::query(
        "INSERT INTO bridge_user_host_prefs (user_id, host_id, enabled) VALUES ($1, 'codex', true)",
    )
    .bind(user_id.as_str())
    .execute(pool.write_pool().as_ref())
    .await
    .expect("insert a host id outside HostKind");

    let err = repo
        .list_enabled(&user_id)
        .await
        .expect_err("an unknown host id must not decode");
    assert!(
        matches!(err, OauthError::Repository(RepositoryError::Decode { .. })),
        "{err:?}"
    );
}
