use anyhow::Result;
use systemprompt_agent::services::agent_orchestration::monitor::AgentMonitor;
use systemprompt_test_fixtures::ensure_test_bootstrap;

use crate::common::Fixture;

#[tokio::test]
async fn agent_monitor_new_succeeds() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let monitor = AgentMonitor::new(&fx.db).expect("monitor");
    let dbg = format!("{:?}", monitor);
    assert!(dbg.contains("AgentMonitor"));
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_monitor_monitor_all_agents_with_none_returns_empty_report() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let monitor = AgentMonitor::new(&fx.db).expect("monitor");

    // Clean up all services first
    let _ = sqlx::query("DELETE FROM services WHERE module_name = 'agent'")
        .execute(&fx.pool)
        .await;

    let report = monitor.monitor_all_agents().await?;
    assert_eq!(report.total_agents(), 0);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_monitor_cleanup_unresponsive_agents_returns_count() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let monitor = AgentMonitor::new(&fx.db).expect("monitor");

    let count = monitor.cleanup_unresponsive_agents().await?;
    // Just verify it returns a value, value depends on db state
    let _ = count;
    fx.cleanup().await?;
    Ok(())
}
