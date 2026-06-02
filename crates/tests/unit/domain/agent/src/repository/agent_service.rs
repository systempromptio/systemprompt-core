use super::{repos, try_pool};
use uuid::Uuid;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

#[tokio::test]
async fn register_and_get_status_running() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let name = unique_name("svc-running");

    let returned = r
        .agent_services
        .register_agent(&name, 4242, 9100)
        .await
        .expect("register");
    assert_eq!(returned, name);

    let status = r
        .agent_services
        .get_agent_status(&name)
        .await
        .expect("status")
        .expect("row present");
    assert_eq!(status.name, name);
    assert_eq!(status.status, "running");
    assert_eq!(status.pid, Some(4242));
    assert_eq!(status.port, 9100);

    r.agent_services
        .remove_agent_service(&name)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn register_starting_then_mark_running() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let name = unique_name("svc-starting");

    r.agent_services
        .register_agent_starting(&name, 1, 9101)
        .await
        .expect("register starting");
    let status = r
        .agent_services
        .get_agent_status(&name)
        .await
        .expect("status")
        .expect("row");
    assert_eq!(status.status, "starting");

    r.agent_services
        .mark_running(&name)
        .await
        .expect("mark running");
    let status = r
        .agent_services
        .get_agent_status(&name)
        .await
        .expect("status")
        .expect("row");
    assert_eq!(status.status, "running");

    r.agent_services.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn mark_crashed_clears_pid() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let name = unique_name("svc-crash");
    r.agent_services
        .register_agent(&name, 99, 9102)
        .await
        .expect("register");

    r.agent_services.mark_crashed(&name).await.expect("crash");
    let status = r
        .agent_services
        .get_agent_status(&name)
        .await
        .expect("status")
        .expect("row");
    assert_eq!(status.status, "error");
    assert_eq!(status.pid, None);

    r.agent_services.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn mark_stopped_and_error_clear_pid() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);

    let stopped = unique_name("svc-stop");
    r.agent_services
        .register_agent(&stopped, 5, 9103)
        .await
        .expect("register");
    r.agent_services.mark_stopped(&stopped).await.expect("stop");
    let s = r
        .agent_services
        .get_agent_status(&stopped)
        .await
        .expect("status")
        .expect("row");
    assert_eq!(s.status, "stopped");
    assert_eq!(s.pid, None);

    let errored = unique_name("svc-err");
    r.agent_services
        .register_agent(&errored, 6, 9104)
        .await
        .expect("register");
    r.agent_services.mark_error(&errored).await.expect("error");
    let s = r
        .agent_services
        .get_agent_status(&errored)
        .await
        .expect("status")
        .expect("row");
    assert_eq!(s.status, "error");

    r.agent_services.remove_agent_service(&stopped).await.ok();
    r.agent_services.remove_agent_service(&errored).await.ok();
}

#[tokio::test]
async fn update_health_status_sets_arbitrary_status() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let name = unique_name("svc-health");
    r.agent_services
        .register_agent(&name, 7, 9105)
        .await
        .expect("register");

    r.agent_services
        .update_health_status(&name, "degraded")
        .await
        .expect("update health");
    let s = r
        .agent_services
        .get_agent_status(&name)
        .await
        .expect("status")
        .expect("row");
    assert_eq!(s.status, "degraded");

    r.agent_services.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn list_running_agents_and_pids_include_registered() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let name = unique_name("svc-list");
    r.agent_services
        .register_agent(&name, 31337, 9106)
        .await
        .expect("register");

    let running = r
        .agent_services
        .list_running_agents()
        .await
        .expect("list running");
    assert!(running.iter().any(|a| a.name == name));

    let pids = r
        .agent_services
        .list_running_agent_pids()
        .await
        .expect("list pids");
    assert!(pids.iter().any(|a| a.name == name && a.pid == 31337));

    r.agent_services.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn get_status_unknown_returns_none() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let result = r
        .agent_services
        .get_agent_status(&unique_name("never-registered"))
        .await
        .expect("status");
    assert!(result.is_none());
}

#[tokio::test]
async fn register_twice_upserts() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let name = unique_name("svc-upsert");
    r.agent_services
        .register_agent(&name, 1, 9107)
        .await
        .expect("first");
    r.agent_services
        .register_agent(&name, 2, 9108)
        .await
        .expect("second");
    let s = r
        .agent_services
        .get_agent_status(&name)
        .await
        .expect("status")
        .expect("row");
    assert_eq!(s.pid, Some(2));
    assert_eq!(s.port, 9108);

    r.agent_services.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn remove_unknown_is_ok() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    r.agent_services
        .remove_agent_service(&unique_name("ghost"))
        .await
        .expect("remove ok");
}
