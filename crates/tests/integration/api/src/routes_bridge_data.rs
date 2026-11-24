//! The reads and writes behind the bridge manifest's per-user inputs, on the
//! repositories the `gateway-http` member builds: the user row, the revoked
//! API keys and the per-host enablement round trip.

use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::host::{HostKind, InstanceHosts};
use systemprompt_oauth::repository::BridgeHostPrefsRepository;
use systemprompt_test_fixtures::{AppContextSeeds as _, ensure_test_bootstrap};
use systemprompt_users::UserRepository;

use super::common::setup_ctx;

#[tokio::test]
async fn load_user_returns_none_for_unknown_user() -> anyhow::Result<()> {
    let _ = ensure_test_bootstrap();
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("nope-{}", uuid::Uuid::new_v4()));
    let result = UserRepository::new(ctx.db_pool()).find_by_id(&user).await?;
    assert!(result.is_none());
    Ok(())
}

#[tokio::test]
async fn load_revocations_empty_for_unknown_user() -> anyhow::Result<()> {
    let _ = ensure_test_bootstrap();
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("rev-{}", uuid::Uuid::new_v4()));
    let revs = UserRepository::new(ctx.db_pool())
        .list_revoked_api_key_ids_for_user(&user)
        .await?;
    assert!(revs.is_empty());
    Ok(())
}

#[tokio::test]
async fn load_enabled_hosts_empty_for_unknown_user() -> anyhow::Result<()> {
    let _ = ensure_test_bootstrap();
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("hosts-{}", uuid::Uuid::new_v4()));
    let prefs = BridgeHostPrefsRepository::new(ctx.db_pool())
        .get_enabled_prefs(&user)
        .await?;
    assert!(prefs.hosts.is_empty());
    assert!(!prefs.unknown_enabled_row);
    Ok(())
}

#[tokio::test]
async fn set_host_enabled_round_trip() -> anyhow::Result<()> {
    let _ = ensure_test_bootstrap();
    let (pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("pref-{}", uuid::Uuid::new_v4()));
    let exec_pool = pool.pool();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@test.invalid", user.as_str()))
        .execute(exec_pool.as_ref())
        .await?;
    let instance = InstanceHosts {
        enabled: vec![HostKind::ClaudeCode, HostKind::OpenCode],
        opt_in: Default::default(),
    };
    let repo = BridgeHostPrefsRepository::new(ctx.db_pool());
    repo.set_enabled(&user, HostKind::ClaudeCode, true, &instance)
        .await?;
    let prefs = repo.get_enabled_prefs(&user).await?;
    assert!(prefs.hosts.contains(&HostKind::ClaudeCode), "got {prefs:?}");
    repo.set_enabled(&user, HostKind::OpenCode, true, &instance)
        .await?;
    repo.set_enabled(&user, HostKind::ClaudeCode, false, &instance)
        .await?;
    let prefs2 = repo.get_enabled_prefs(&user).await?;
    assert!(!prefs2.hosts.contains(&HostKind::ClaudeCode));
    assert!(prefs2.hosts.contains(&HostKind::OpenCode));
    Ok(())
}
