use std::sync::OnceLock;
use std::time::Duration;

use systemprompt_api::services::server::lifecycle::process_ownership::ProcessKey;
use systemprompt_api::services::server::{bind_and_serve, run_server};
use systemprompt_database::{CoordinationRepositories, ServiceModule};
use systemprompt_identifiers::{InstanceId, LeaseKey, ServiceName};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_config, init_isolated_bootstrap, test_db_pool,
};
use systemprompt_traits::LeaseRepository;

const AGENT: &str = "leased_unstartable_agent";
const AGENT_PORT: u16 = 4937;

fn services_config() -> String {
    format!(
        r#"agents:
  {AGENT}:
    name: {AGENT}
    port: {AGENT_PORT}
    endpoint: http://127.0.0.1:{AGENT_PORT}
    enabled: true
    card:
      protocolVersion: "0.3.0"
      displayName: Leased Unstartable Agent
      description: Declared enabled with no binary, so every leased start fails.
      version: "1.0.0"
    metadata: {{}}
    oauth:
      required: false
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
    "#
    )
}

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", &services_config()))
}

fn lease_key() -> LeaseKey {
    ProcessKey::new(ServiceModule::Agent, ServiceName::new(AGENT)).lease_key()
}

async fn worker_server() -> anyhow::Result<(
    systemprompt_database::DbPool,
    tokio::task::JoinHandle<anyhow::Result<()>>,
)> {
    let b = boot();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&b.database_url);
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];
    config.role = NodeRole::Worker;
    let ctx = systemprompt_test_fixtures::fixture_app_context_on_bootstrap(
        b,
        &pool,
        config,
        std::sync::Arc::new(systemprompt_marketplace::AllowAllFilter),
    )?;
    let early = bind_and_serve(
        "127.0.0.1:0",
        None,
        systemprompt_runtime::ShutdownRequest::default(),
        systemprompt_manifest::profile::ShutdownConfig::default(),
    )
    .await?;
    let readiness = std::sync::Arc::clone(early.readiness());
    let server = tokio::spawn(run_server((*ctx).clone(), None, early));
    assert!(
        readiness.wait_for_ready(60).await,
        "a worker comes up without starting its processes inline"
    );
    Ok((pool, server))
}

async fn terminate(server: tokio::task::JoinHandle<anyhow::Result<()>>) -> anyhow::Result<()> {
    let status = std::process::Command::new("kill")
        .args(["-TERM", &std::process::id().to_string()])
        .status()?;
    assert!(status.success(), "kill -TERM failed: {status}");
    let result = tokio::time::timeout(Duration::from_secs(60), server).await??;
    assert!(result.is_ok(), "the worker drains cleanly: {result:?}");
    Ok(())
}

#[tokio::test]
async fn a_worker_claims_the_lease_and_releases_it_when_the_start_fails() -> anyhow::Result<()> {
    let (pool, server) = worker_server().await?;
    let observer = CoordinationRepositories::new(&pool, InstanceId::new("lease_observer"));
    let key = lease_key();

    let mut claimed_by = None;
    let mut released = false;
    for _ in 0..1200 {
        match observer.leases.holder(&key).await? {
            Some(lease) => claimed_by = Some(lease.owner),
            None if claimed_by.is_some() => {
                released = true;
                break;
            },
            None => {},
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let owner = claimed_by.expect("the worker claims the agent's lease");
    assert_ne!(owner.as_str(), "lease_observer");
    assert!(released, "a failed start releases the lease");

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        observer.leases.holder(&key).await?.is_none(),
        "the failed start backs off instead of reclaiming at once"
    );
    terminate(server).await
}
