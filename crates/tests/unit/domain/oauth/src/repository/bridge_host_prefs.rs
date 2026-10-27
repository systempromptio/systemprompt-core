// DB-backed bridge per-host preference tests (set_enabled + list-enabled).

use systemprompt_models::bridge::host::{HostKind, InstanceHosts};
use systemprompt_oauth::repository::{BridgeHostPrefsRepository, EnabledHostPrefs, HostPrefWrite};

fn hosts(enabled: &[HostKind], opt_in: &[HostKind]) -> InstanceHosts {
    InstanceHosts {
        enabled: enabled.to_vec(),
        opt_in: opt_in.iter().copied().collect(),
    }
}

fn instance() -> InstanceHosts {
    hosts(
        &[HostKind::ClaudeCode, HostKind::ClaudeDesktop, HostKind::OpenCode],
        &[],
    )
}
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};

#[tokio::test]
async fn set_enabled_then_list_enabled() {
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

    repo.set_enabled(&user_id, host_a, true, &instance())
        .await
        .expect("enable a");
    repo.set_enabled(&user_id, host_b, false, &instance())
        .await
        .expect("disable b");

    let enabled = repo.get_enabled_prefs(&user_id).await.expect("list");
    assert!(enabled.hosts.contains(&host_a));
    assert!(!enabled.hosts.contains(&host_b));
}

#[tokio::test]
async fn disabling_the_last_enabled_host_is_refused() {
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
    repo.set_enabled(&user_id, host, true, &instance())
        .await
        .expect("enable");
    assert!(
        repo.get_enabled_prefs(&user_id)
            .await
            .expect("list")
            .hosts
            .contains(&host)
    );

    let outcome = repo
        .set_enabled(&user_id, host, false, &instance())
        .await
        .expect("disable");
    assert_eq!(outcome, HostPrefWrite::WouldDisableAll);
    assert_eq!(
        repo.get_enabled_prefs(&user_id).await.expect("list").hosts,
        vec![host]
    );
}

#[tokio::test]
async fn disabling_one_host_with_no_rows_keeps_the_other_instance_hosts() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp-norows");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");

    let outcome = repo
        .set_enabled(&user_id, HostKind::OpenCode, false, &instance())
        .await
        .expect("disable opencode");
    assert_eq!(outcome, HostPrefWrite::Written);
    let prefs = repo.get_enabled_prefs(&user_id).await.expect("list");
    assert!(prefs.admits(HostKind::ClaudeCode, &instance()));
    assert!(prefs.admits(HostKind::ClaudeDesktop, &instance()));
    assert!(!prefs.admits(HostKind::OpenCode, &instance()));
}

#[tokio::test]
async fn disabling_the_only_instance_host_with_no_rows_is_refused() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp-only");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");

    let outcome = repo
        .set_enabled(
            &user_id,
            HostKind::ClaudeCode,
            false,
            &hosts(&[HostKind::ClaudeCode], &[]),
        )
        .await
        .expect("disable");
    assert_eq!(outcome, HostPrefWrite::WouldDisableAll);
    assert!(
        !repo
            .get_enabled_prefs(&user_id)
            .await
            .expect("list")
            .chose_default_hosts(&instance())
    );
}

#[tokio::test]
async fn list_enabled_empty_for_unknown_user() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp-unknown");
    let enabled = repo.get_enabled_prefs(&user_id).await.expect("list");
    assert!(enabled.hosts.is_empty());
    assert!(!enabled.chose_default_hosts(&instance()));
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
        !repo
            .get_enabled_prefs(&user_id)
            .await
            .expect("list")
            .chose_default_hosts(&instance()),
        "model-filter override must not register an enable-state row"
    );
}

#[tokio::test]
async fn an_unknown_host_id_is_refused_by_the_schema_and_valid_rows_still_read() {
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

    let refused_enable = sqlx::query(
        "INSERT INTO bridge_user_host_prefs (user_id, host_id, enabled) VALUES ($1, 'cowork', true)",
    )
    .bind(user_id.as_str())
    .execute(pool.write_pool().as_ref())
    .await
    .expect_err("a host id outside HostKind must be refused");
    assert!(
        refused_enable
            .as_database_error()
            .is_some_and(|e| e.is_check_violation()),
        "expected a check violation, got {refused_enable}"
    );

    let refused_model = sqlx::query(
        "INSERT INTO bridge_user_host_model_prefs (user_id, host_id, model_protocols) \
         VALUES ($1, 'cowork', ARRAY['anthropic'])",
    )
    .bind(user_id.as_str())
    .execute(pool.write_pool().as_ref())
    .await
    .expect_err("a model pref for a host outside HostKind must be refused");
    assert!(
        refused_model
            .as_database_error()
            .is_some_and(|e| e.is_check_violation()),
        "expected a check violation, got {refused_model}"
    );

    let untouched = repo
        .get_enabled_prefs(&user_id)
        .await
        .expect("read after refused inserts");
    assert!(untouched.hosts.is_empty());
    assert!(!untouched.chose_default_hosts(&instance()));

    sqlx::query(
        "INSERT INTO bridge_user_host_prefs (user_id, host_id, enabled) \
         VALUES ($1, 'claude-code', true)",
    )
    .bind(user_id.as_str())
    .execute(pool.write_pool().as_ref())
    .await
    .expect("insert a valid host id");

    let enabled = repo
        .get_enabled_prefs(&user_id)
        .await
        .expect("read valid rows");
    assert!(enabled.chose_default_hosts(&instance()));
    assert!(enabled.admits(HostKind::ClaudeCode, &instance()));
    assert!(!enabled.admits(HostKind::OpenCode, &instance()));
}

#[test]
fn an_opt_in_host_reaches_only_a_user_who_turned_it_on() {
    let instance = hosts(&[HostKind::ClaudeCode, HostKind::OpenCode], &[HostKind::ClaudeCode]);
    let untouched = EnabledHostPrefs::default();
    assert!(!untouched.admits(HostKind::ClaudeCode, &instance));
    assert!(untouched.admits(HostKind::OpenCode, &instance));

    let opted_in = EnabledHostPrefs {
        hosts: vec![HostKind::ClaudeCode],
        unknown_enabled_row: false,
    };
    assert!(opted_in.admits(HostKind::ClaudeCode, &instance));
    assert!(
        opted_in.admits(HostKind::OpenCode, &instance),
        "turning on an opt-in host is not a choice among the default hosts"
    );
}

#[tokio::test]
async fn enabling_an_opt_in_host_keeps_every_default_host() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = BridgeHostPrefsRepository::new(&pool);
    let user_id = unique_user_id("bhp-optin");
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@bhp.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");
    let instance = hosts(
        &[HostKind::ClaudeCode, HostKind::OpenCode, HostKind::OpenCodeCli],
        &[HostKind::ClaudeCode],
    );

    repo.set_enabled(&user_id, HostKind::ClaudeCode, true, &instance)
        .await
        .expect("opt in");
    let prefs = repo.get_enabled_prefs(&user_id).await.expect("read");
    assert!(prefs.admits(HostKind::ClaudeCode, &instance));
    assert!(prefs.admits(HostKind::OpenCode, &instance));
    assert!(prefs.admits(HostKind::OpenCodeCli, &instance));

    repo.set_enabled(&user_id, HostKind::OpenCode, false, &instance)
        .await
        .expect("disable a default host");
    let prefs = repo.get_enabled_prefs(&user_id).await.expect("read");
    assert!(prefs.admits(HostKind::ClaudeCode, &instance));
    assert!(!prefs.admits(HostKind::OpenCode, &instance));
    assert!(
        prefs.admits(HostKind::OpenCodeCli, &instance),
        "the other default hosts are recorded when the first is disabled"
    );
}
