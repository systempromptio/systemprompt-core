// DB-backed tests for AgentDatabaseService: status reconciliation, lifecycle
// transitions, and listing. Each test early-returns when no
// test database is configured (mirrors the repository test guard).
//
// PIDs above i32::MAX are non-signalable, so `process::process_exists` returns
// false for them without ever touching a real process. We use such a PID to
// drive the "recorded running but process dead" reconciliation branch
// deterministically.

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::AgentStatus;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_test_fixtures::ensure_test_bootstrap;
use uuid::Uuid;

use systemprompt_test_fixtures::test_db_pool;

// A PID that can never name a live, signalable process (> i32::MAX).
// Why: `services.pid` is an `INTEGER` column, so a dead pid must fit i32 while
// still lying far above any pid_max a kernel will hand out.
const DEAD_PID: u32 = 2_000_000_000;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

async fn service(pool: &systemprompt_database::DbPool) -> AgentDatabaseService {
    ensure_test_bootstrap();
    let _skills = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repo = AgentServiceRepository::new(
        pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .expect("repo");
    AgentDatabaseService::new(repo).expect("db service")
}

#[tokio::test]
async fn register_then_status_reconciles_dead_pid_to_failed() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let name = unique_name("orch-dead");

    svc.register_agent(&name, DEAD_PID, 9300)
        .await
        .expect("register");

    // Stored status is 'running' but the PID is not a live process, so
    // get_status marks it failed and reports Failed.
    let status = svc.get_status(&name).await.expect("status");
    match status {
        AgentStatus::Failed { reason, .. } => {
            assert!(reason.contains("died"));
        },
        other => panic!("expected Failed, got {other:?}"),
    }

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn status_no_record_is_failed() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let status = svc
        .get_status(&unique_name("orch-missing"))
        .await
        .expect("status");
    match status {
        AgentStatus::Failed { reason, .. } => assert!(reason.contains("No service record")),
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test]
async fn status_starting_is_failed_with_starting_reason() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let name = unique_name("orch-starting");
    svc.register_agent_starting(&name, DEAD_PID, 9301)
        .await
        .expect("register starting");

    let status = svc.get_status(&name).await.expect("status");
    match status {
        AgentStatus::Failed { reason, .. } => assert!(reason.contains("starting")),
        other => panic!("expected Failed, got {other:?}"),
    }

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn status_stopped_is_failed() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let name = unique_name("orch-stopped");
    svc.register_agent(&name, DEAD_PID, 9302)
        .await
        .expect("register");
    svc.update_agent_stopped(&name).await.expect("stop");

    let status = svc.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn list_running_agents_includes_registered() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let name = unique_name("orch-listrun");
    svc.register_agent(&name, DEAD_PID, 9304)
        .await
        .expect("register");

    let running = svc.list_running_agents().await.expect("list");
    assert!(running.iter().any(|n| n == &name));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn lifecycle_register_starting_mark_running_then_stopped() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let name = unique_name("orch-lifecycle");

    svc.register_agent_starting(&name, DEAD_PID, 9306)
        .await
        .expect("starting");
    svc.mark_running(&name).await.expect("running");
    // update_agent_running upserts the row back to running.
    svc.update_agent_running(&name, DEAD_PID, 9307)
        .await
        .expect("update running");
    svc.update_agent_stopped(&name).await.expect("stopped");

    let status = svc.get_status(&name).await.expect("status");
    assert!(matches!(status, AgentStatus::Failed { .. }));

    svc.remove_agent_service(&name).await.ok();
}

async fn status_and_stamp(raw: &sqlx::PgPool, name: &str) -> (String, String) {
    sqlx::query_as::<_, (String, String)>(
        "SELECT status, updated_at::text FROM services WHERE instance_id = $1 AND name = $2",
    )
    .bind("test-instance")
    .bind(name)
    .fetch_one(raw)
    .await
    .expect("row")
}

#[tokio::test]
async fn error_row_reads_as_failed_without_rewriting_it() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let raw = pool.pool_arc().expect("raw database pool");
    let name = unique_name("orch-crash");
    svc.register_agent(&name, DEAD_PID, 9308)
        .await
        .expect("register");
    svc.mark_failed(&name).await.expect("mark failed");

    let before = status_and_stamp(raw.as_ref(), &name).await;
    assert_eq!(before.0, "error");

    for _ in 0..2 {
        match svc.get_status(&name).await.expect("status") {
            AgentStatus::Failed { reason, .. } => assert_eq!(reason, "Agent process failed"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }
    assert_eq!(
        status_and_stamp(raw.as_ref(), &name).await,
        before,
        "reading an error row must not write it"
    );

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn mcp_rows_are_invisible_to_agent_supervision() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let services = systemprompt_database::ServiceRepository::new(
        &pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .expect("services repo");
    let name = unique_name("orch-mcp-row");
    services
        .upsert_service_process(systemprompt_database::UpsertServiceProcessInput {
            name: &name,
            module_name: "mcp",
            pid: i32::try_from(DEAD_PID).expect("pid fits"),
            port: 9310,
            status: "running",
        })
        .await
        .expect("seed mcp row");

    let running = svc.list_running_agents().await.expect("list");
    assert!(!running.iter().any(|n| n == &name));

    match svc.get_status(&name).await.expect("status") {
        AgentStatus::Failed { reason, .. } => assert!(reason.contains("No service record")),
        other => panic!("expected Failed, got {other:?}"),
    }
    let row = services
        .find_service_by_name(&name)
        .await
        .expect("find")
        .expect("mcp row");
    assert_eq!(row.status, "running", "agent reads must not mark MCP rows");

    services.delete_service(&name).await.ok();
}

#[tokio::test]
async fn agent_exists_false_for_unconfigured() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let exists = svc
        .agent_exists("__no_such_configured_agent")
        .await
        .expect("exists");
    assert!(!exists);
}

#[tokio::test]
async fn get_agent_config_unknown_errors() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let err = svc
        .get_agent_config("__no_such_agent_cfg")
        .await
        .expect_err("not found");
    assert!(format!("{err}").contains("not found"));
}

#[tokio::test]
async fn list_all_agents_empty_default_config() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    // Default test config has no agents configured.
    let all = svc.list_all_agents().await.expect("list all");
    assert!(all.is_empty());
}

#[tokio::test]
async fn remove_unknown_service_is_ok() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    svc.remove_agent_service(&unique_name("orch-ghost"))
        .await
        .expect("remove ok");
}

#[tokio::test]
async fn status_rejects_corrupt_persisted_process_identifiers_without_rewriting_the_row() {
    let pool = test_db_pool().await;
    let svc = service(&pool).await;
    let raw = pool.pool_arc().expect("raw database pool");
    let name = unique_name("orch-corrupt-process");
    svc.register_agent(&name, std::process::id(), 9309)
        .await
        .expect("register owned process identity");

    sqlx::query("UPDATE services SET pid = -1 WHERE instance_id = $1 AND name = $2")
        .bind("test-instance")
        .bind(&name)
        .execute(raw.as_ref())
        .await
        .expect("inject negative persisted pid");
    let error = svc
        .get_status(&name)
        .await
        .expect_err("negative persisted pid must be rejected");
    assert_eq!(
        error.to_string(),
        "Database error: stored pid -1 is not a process id"
    );
    let state: (Option<i32>, i32, String) = sqlx::query_as(
        "SELECT pid, port, status FROM services WHERE instance_id = $1 AND name = $2",
    )
    .bind("test-instance")
    .bind(&name)
    .fetch_one(raw.as_ref())
    .await
    .expect("persisted corrupt row");
    assert_eq!(state, (Some(-1), 9309, "running".to_owned()));

    sqlx::query("UPDATE services SET pid = $1, port = 70000 WHERE instance_id = $2 AND name = $3")
        .bind(i32::try_from(std::process::id()).expect("current pid fits database"))
        .bind("test-instance")
        .bind(&name)
        .execute(raw.as_ref())
        .await
        .expect("inject out-of-range persisted port");
    let error = svc
        .get_status(&name)
        .await
        .expect_err("out-of-range persisted port must be rejected");
    assert_eq!(
        error.to_string(),
        "Database error: stored port 70000 is not a TCP port"
    );
    let state: (Option<i32>, i32, String) = sqlx::query_as(
        "SELECT pid, port, status FROM services WHERE instance_id = $1 AND name = $2",
    )
    .bind("test-instance")
    .bind(&name)
    .fetch_one(raw.as_ref())
    .await
    .expect("persisted corrupt row");
    assert_eq!(
        state,
        (
            Some(i32::try_from(std::process::id()).unwrap()),
            70000,
            "running".to_owned()
        )
    );

    svc.remove_agent_service(&name)
        .await
        .expect("fixture cleanup");
}
