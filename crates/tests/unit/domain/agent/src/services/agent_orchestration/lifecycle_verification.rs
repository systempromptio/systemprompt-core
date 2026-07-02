// Startup-verification tests for AgentLifecycle: prerequisite port checks
// against free and occupied ports (an occupied port is held by this test
// process, which is not an agent, so cleanup must refuse), TCP readiness
// probing against a live listener, and the failure path that marks the agent
// errored and logs the startup diagnosis for each stored status.

use std::sync::Arc;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::lifecycle::AgentLifecycle;
use systemprompt_models::AppPaths;
use tokio::net::TcpListener;
use uuid::Uuid;

use crate::repository::try_pool;

const DEAD_PID: u32 = 4_000_000_000;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn app_paths() -> Arc<AppPaths> {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    Arc::new(bootstrap.app_paths.clone())
}

fn lifecycle(pool: &systemprompt_database::DbPool) -> AgentLifecycle {
    AgentLifecycle::new(pool, app_paths()).expect("lifecycle")
}

fn db_service(pool: &systemprompt_database::DbPool) -> AgentDatabaseService {
    let repo = AgentServiceRepository::new(pool).expect("repo");
    AgentDatabaseService::new(repo).expect("db service")
}

async fn ephemeral_listener() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

#[tokio::test]
async fn validate_prerequisites_free_port_is_ok() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool);

    let (listener, port) = ephemeral_listener().await;
    drop(listener);

    lc.validate_prerequisites(port).await.expect("free port");
}

#[tokio::test]
async fn validate_prerequisites_port_held_by_non_agent_fails() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool);

    let (listener, port) = ephemeral_listener().await;
    let result = lc.validate_prerequisites(port).await;
    drop(listener);

    assert!(
        result.is_err(),
        "a port held by a non-agent process must not be reclaimed"
    );
}

#[tokio::test]
async fn verify_startup_succeeds_against_live_listener() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool);

    let (listener, port) = ephemeral_listener().await;
    let accept_loop = tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });

    lc.verify_startup("lc_verify_ok", port)
        .await
        .expect("listener answers the readiness probe");
    accept_loop.abort();
}

#[tokio::test]
async fn verify_startup_times_out_and_marks_error() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool);
    let db = db_service(&pool);

    let name = unique_name("lc_verify_dead");
    db.register_agent_starting(&name, DEAD_PID, 39461)
        .await
        .expect("register");

    let (listener, port) = ephemeral_listener().await;
    drop(listener);

    let err = lc
        .verify_startup(&name, port)
        .await
        .expect_err("nothing listens on the probed port");
    assert!(err.to_string().contains(&name));

    db.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn log_startup_failure_covers_stored_statuses() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool);
    let db = db_service(&pool);

    lc.log_startup_failure("lc_log_missing", 39462).await;

    let running = unique_name("lc_log_running");
    db.register_agent(&running, std::process::id(), 39463)
        .await
        .expect("register running");
    lc.log_startup_failure(&running, 39463).await;
    db.remove_agent_service(&running).await.ok();

    let dead = unique_name("lc_log_dead");
    db.register_agent(&dead, DEAD_PID, 39464)
        .await
        .expect("register dead");
    lc.log_startup_failure(&dead, 39464).await;
    db.remove_agent_service(&dead).await.ok();
}
