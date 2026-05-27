use anyhow::Result;
use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use uuid::Uuid;

use crate::common::Fixture;

fn unique_agent_name(suffix: &str) -> String {
    format!("agent_{}_{}", suffix, Uuid::new_v4().simple())
}

async fn cleanup_agent(pool: &sqlx::PgPool, name: &str) {
    let _ = sqlx::query("DELETE FROM services WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await;
}

#[tokio::test]
async fn register_and_get_agent_status_running() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("reg");

    let stored = repo.register_agent(&name, 12345, 9001).await?;
    assert_eq!(stored, name);

    let row = repo.get_agent_status(&name).await?.expect("row");
    assert_eq!(row.status, "running");
    assert_eq!(row.pid, Some(12345));
    assert_eq!(row.port, 9001);

    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn register_agent_starting_status() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("start");

    repo.register_agent_starting(&name, 22222, 9002).await?;
    let row = repo.get_agent_status(&name).await?.expect("row");
    assert_eq!(row.status, "starting");

    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn mark_running_transitions_status() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("mark");
    repo.register_agent_starting(&name, 11, 9003).await?;
    repo.mark_running(&name).await?;
    let row = repo.get_agent_status(&name).await?.unwrap();
    assert_eq!(row.status, "running");
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn mark_crashed_clears_pid_and_sets_error() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("crash");
    repo.register_agent(&name, 33, 9004).await?;
    repo.mark_crashed(&name).await?;
    let row = repo.get_agent_status(&name).await?.unwrap();
    assert_eq!(row.status, "error");
    assert!(row.pid.is_none());
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn mark_stopped_clears_pid() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("stop");
    repo.register_agent(&name, 44, 9005).await?;
    repo.mark_stopped(&name).await?;
    let row = repo.get_agent_status(&name).await?.unwrap();
    assert_eq!(row.status, "stopped");
    assert!(row.pid.is_none());
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn mark_error_sets_error_status() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("err");
    repo.register_agent(&name, 55, 9006).await?;
    repo.mark_error(&name).await?;
    let row = repo.get_agent_status(&name).await?.unwrap();
    assert_eq!(row.status, "error");
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn get_agent_status_unknown_returns_none() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let row = repo.get_agent_status("__no_such_agent_xyzzz").await?;
    assert!(row.is_none());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn list_running_agents_includes_registered() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("listrun");
    repo.register_agent(&name, 66, 9007).await?;
    let list = repo.list_running_agents().await?;
    assert!(list.iter().any(|r| r.name == name));
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn list_running_agent_pids_includes_registered_with_pid() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("listpid");
    repo.register_agent(&name, 7777, 9008).await?;
    let list = repo.list_running_agent_pids().await?;
    let found = list.iter().find(|r| r.name == name).expect("found");
    assert_eq!(found.pid, 7777);
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn remove_agent_service_removes_entry() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("rm");
    repo.register_agent(&name, 11, 9009).await?;
    repo.remove_agent_service(&name).await?;
    let row = repo.get_agent_status(&name).await?;
    assert!(row.is_none());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn update_health_status_changes_status() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("hth");
    repo.register_agent(&name, 11, 9010).await?;
    repo.update_health_status(&name, "degraded").await?;
    let row = repo.get_agent_status(&name).await?.unwrap();
    assert_eq!(row.status, "degraded");
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn register_agent_overwrites_existing_via_upsert() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = AgentServiceRepository::new(&fx.db)?;
    let name = unique_agent_name("upsert");
    repo.register_agent(&name, 100, 9011).await?;
    repo.register_agent(&name, 200, 9012).await?;
    let row = repo.get_agent_status(&name).await?.unwrap();
    assert_eq!(row.pid, Some(200));
    assert_eq!(row.port, 9012);
    cleanup_agent(&fx.pool, &name).await;
    fx.cleanup().await?;
    Ok(())
}
