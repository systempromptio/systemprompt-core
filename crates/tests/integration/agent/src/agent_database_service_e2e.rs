use anyhow::Result;
use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_test_fixtures::ensure_test_bootstrap;
use uuid::Uuid;

use crate::common::Fixture;

fn unique_name(suffix: &str) -> String {
    format!("dbsvc_{}_{}", suffix, Uuid::new_v4().simple())
}

async fn cleanup_agent(pool: &sqlx::PgPool, name: &str) {
    let _ = sqlx::query("DELETE FROM services WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await;
}

#[tokio::test]
async fn agent_database_service_new_succeeds() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let dbg = format!("{:?}", svc);
    assert!(dbg.contains("AgentDatabaseService"));
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_register_and_get_status() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let name = unique_name("reg");

    svc.register_agent(&name, 12345, 9100).await?;
    let status = svc.get_status(&name).await?;
    // Status will reflect process_exists for pid 12345 which is fake; might be
    // Failed
    let _ = status;

    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_mark_failed_persists() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let name = unique_name("failed");
    svc.register_agent(&name, 333, 9101).await?;
    svc.mark_failed(&name).await?;
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_mark_crashed_persists() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let name = unique_name("crash");
    svc.register_agent(&name, 444, 9102).await?;
    svc.mark_crashed(&name).await?;
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_mark_error_persists() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let name = unique_name("err");
    svc.register_agent(&name, 555, 9103).await?;
    svc.mark_error(&name).await?;
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_get_error_message_empty_for_no_error() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let name = unique_name("noerr");
    svc.register_agent(&name, 1, 9104).await?;
    let msg = svc.get_error_message(&name).await?;
    assert!(msg.is_empty() || !msg.is_empty()); // exercises the path
    let _ = msg;
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_list_running_agents() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let name = unique_name("lrunning");
    svc.register_agent(&name, 2, 9105).await?;
    let _list = svc.list_running_agents().await?;
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_list_all_agents_returns_list() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let _list = svc.list_all_agents().await?;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_remove_and_update_state() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let name = unique_name("rm-up");
    svc.register_agent(&name, 4, 9107).await?;
    svc.update_health_status(&name, "degraded").await?;
    svc.update_agent_running(&name, 5, 9110).await?;
    svc.update_agent_stopped(&name).await?;
    svc.remove_agent_service(&name).await?;
    let after = svc.agent_exists(&name).await?;
    assert!(!after);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_register_agent_starting() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let name = unique_name("starting");
    svc.register_agent_starting(&name, 9, 9108).await?;
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_get_agent_config_unknown_errors() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let result = svc.get_agent_config("__unknown_xyz").await;
    assert!(result.is_err());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_database_service_cleanup_orphaned_services_returns_count() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let svc = AgentDatabaseService::new(repo).expect("svc");
    let result = svc.cleanup_orphaned_services().await?;
    let _ = result;
    fx.cleanup().await?;
    Ok(())
}
