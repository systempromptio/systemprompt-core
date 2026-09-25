use anyhow::Result;
use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::monitor::AgentMonitor;
use systemprompt_test_fixtures::ensure_test_bootstrap;

use crate::common::Fixture;


#[tokio::test]
async fn agent_monitor_monitor_all_agents_with_none_returns_empty_report() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let monitor = AgentMonitor::new(
        AgentServiceRepository::new(&fx.db, crate::common::unique_instance()).expect("repo"),
    )
    .expect("monitor");

    let report = monitor.monitor_all_agents().await?;
    assert_eq!(report.total_agents(), 0);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_monitor_cleanup_unresponsive_agents_returns_count() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let monitor = AgentMonitor::new(
        AgentServiceRepository::new(&fx.db, crate::common::unique_instance()).expect("repo"),
    )
    .expect("monitor");

    let count = monitor.cleanup_unresponsive_agents().await?;
    assert_eq!(count, 0);
    fx.cleanup().await?;
    Ok(())
}
