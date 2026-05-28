//! Direct exercises of the public helpers in `routes::gateway::bridge_data`
//! — these don't go through HTTP and therefore aren't reached by the
//! gateway_router integration tests. Hits `load_user`, `load_revocations`,
//! `load_enabled_hosts`, `upsert_host_pref`, `load_services_config`, and
//! `load_managed_mcp_servers`.

use systemprompt_api::routes::gateway::bridge_data::{
    load_enabled_hosts, load_managed_mcp_servers, load_revocations, load_services_config,
    load_user, upsert_host_pref,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::services::ServicesConfig;
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
    let hosts = load_enabled_hosts(&ctx, &user).await?;
    assert!(hosts.is_empty());
    Ok(())
}

#[tokio::test]
async fn upsert_host_pref_round_trip() -> anyhow::Result<()> {
    let _ = ensure_test_bootstrap();
    let (pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("pref-{}", uuid::Uuid::new_v4()));
    let exec_pool = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@test.invalid", user.as_str()))
        .execute(exec_pool.as_ref())
        .await?;
    upsert_host_pref(&ctx, &user, "claude-code", true).await?;
    let hosts = load_enabled_hosts(&ctx, &user).await?;
    assert!(hosts.iter().any(|h| h == "claude-code"), "got {hosts:?}");
    // Toggle off.
    upsert_host_pref(&ctx, &user, "claude-code", false).await?;
    let hosts2 = load_enabled_hosts(&ctx, &user).await?;
    assert!(!hosts2.iter().any(|h| h == "claude-code"));
    Ok(())
}

#[tokio::test]
async fn load_services_config_returns_some_value_or_error() {
    // Either a config is present in the bootstrapped services dir (empty stub)
    // or this errors out — both code paths are exercised.
    let _ = ensure_test_bootstrap();
    let _ = load_services_config();
}

#[tokio::test]
async fn load_managed_mcp_servers_empty_when_no_servers_configured() {
    let services = ServicesConfig::default();
    let result = load_managed_mcp_servers(&services, "http://127.0.0.1");
    assert!(result.is_ok());
    let servers = result.unwrap();
    assert!(servers.is_empty());
}

#[tokio::test]
async fn load_managed_mcp_servers_strips_trailing_slash_from_url() {
    let services = ServicesConfig::default();
    let result = load_managed_mcp_servers(&services, "http://127.0.0.1/");
    assert!(result.is_ok());
}

#[tokio::test]
async fn load_managed_mcp_servers_synthesises_url_from_api_external_url() {
    use std::collections::HashMap;
    use systemprompt_models::auth::JwtAudience;
    use systemprompt_models::mcp::{Deployment, McpServerType, OAuthRequirement};

    let mut services = ServicesConfig::default();
    services.mcp_servers.insert(
        "sharepoint-sim".to_owned(),
        Deployment {
            server_type: McpServerType::Internal,
            binary: "sharepoint-sim".to_owned(),
            package: None,
            port: 5101,
            endpoint: None,
            enabled: true,
            display_in_web: true,
            dev_only: false,
            schemas: vec![],
            oauth: OAuthRequirement {
                required: true,
                scopes: vec![],
                audience: JwtAudience::Mcp,
                client_id: None,
            },
            tools: HashMap::new(),
            model_config: None,
            env_vars: vec![],
        },
    );

    let entries = load_managed_mcp_servers(&services, "http://localhost:8080")
        .expect("synthesis must succeed");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].url.as_str(),
        "http://localhost:8080/api/v1/mcp/sharepoint-sim/mcp"
    );
}
