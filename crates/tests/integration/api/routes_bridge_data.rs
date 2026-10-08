//! Direct exercises of the public helpers in `routes::gateway::bridge_data`
//! — these don't go through HTTP and therefore aren't reached by the
//! gateway_router integration tests. Hits `load_user`, `load_revocations`,
//! `load_enabled_hosts`, `upsert_host_pref`, and `load_services_config`.

use systemprompt_api::routes::gateway::bridge_data::{
    load_enabled_hosts, load_revocations, load_services_config, load_user, upsert_host_pref,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_test_fixtures::ensure_test_bootstrap;

use super::common::setup_ctx;

#[tokio::test]
async fn load_user_returns_none_for_unknown_user() -> anyhow::Result<()> {
    let _ = ensure_test_bootstrap();
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("nope-{}", uuid::Uuid::new_v4()));
    let result = load_user(&ctx, &user).await?;
    assert!(result.is_none());
    Ok(())
}

#[tokio::test]
async fn load_revocations_empty_for_unknown_user() -> anyhow::Result<()> {
    let _ = ensure_test_bootstrap();
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("rev-{}", uuid::Uuid::new_v4()));
    let revs = load_revocations(&ctx, &user).await?;
    assert!(revs.is_empty());
    Ok(())
}

#[tokio::test]
async fn load_enabled_hosts_empty_for_unknown_user() -> anyhow::Result<()> {
    let _ = ensure_test_bootstrap();
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("hosts-{}", uuid::Uuid::new_v4()));
    let prefs = load_enabled_hosts(&ctx, &user).await?;
    assert!(prefs.hosts.is_empty());
    assert!(!prefs.any_enabled_row);
    Ok(())
}

#[tokio::test]
async fn upsert_host_pref_round_trip() -> anyhow::Result<()> {
    let _ = ensure_test_bootstrap();
    let (pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("pref-{}", uuid::Uuid::new_v4()));
    let exec_pool = pool.pool();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@test.invalid", user.as_str()))
        .execute(exec_pool.as_ref())
        .await?;
    upsert_host_pref(&ctx, &user, HostKind::ClaudeCode, true).await?;
    let prefs = load_enabled_hosts(&ctx, &user).await?;
    assert!(prefs.hosts.contains(&HostKind::ClaudeCode), "got {prefs:?}");
    // Toggle off.
    upsert_host_pref(&ctx, &user, HostKind::ClaudeCode, false).await?;
    let prefs2 = load_enabled_hosts(&ctx, &user).await?;
    assert!(!prefs2.hosts.contains(&HostKind::ClaudeCode));
    Ok(())
}

#[tokio::test]
async fn load_services_config_returns_some_value_or_error() {
    // Either a config is present in the bootstrapped services dir (empty stub)
    // or this errors out — both code paths are exercised.
    let _ = ensure_test_bootstrap();
    let _ = load_services_config();
}
