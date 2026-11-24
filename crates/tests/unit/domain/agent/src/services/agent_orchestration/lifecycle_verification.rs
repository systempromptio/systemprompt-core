// Startup-verification tests for AgentLifecycle: prerequisite port checks
// against free and occupied ports (an occupied port is held by this test
// process, which is not an agent, so cleanup must refuse), TCP readiness
// probing that accepts only the spawned, marked agent listening on its port
// and refuses a port answered by a foreign process, and the failure path that
// marks the agent errored and logs the startup diagnosis for each stored
// status.

use std::sync::Arc;
use systemprompt_identifiers::{AgentName, InstanceId, SpawnNonce};

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::OrchestrationError;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::lifecycle::AgentLifecycle;
use systemprompt_config::paths::AppPaths;
use tokio::net::TcpListener;

use systemprompt_test_fixtures::{test_db_pool, unique_agent_name, unique_instance};

const AGENT_HELPER: &str = "services::agent_orchestration::process_stop_live::marker_helper";

// Why: `services.pid` is an `INTEGER` column, so a dead pid must fit i32 while
// still lying far above any pid_max a kernel will hand out.
const DEAD_PID: u32 = 2_000_000_000;

fn app_paths() -> Arc<AppPaths> {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    Arc::new(bootstrap.app_paths.clone())
}

fn lifecycle(pool: &systemprompt_database::DbPool, instance: &InstanceId) -> AgentLifecycle {
    AgentLifecycle::new(
        AgentServiceRepository::new(pool, instance.clone()),
        app_paths(),
        systemprompt_test_fixtures::test_spawner(),
        crate::spawn_environment(),
        &systemprompt_test_fixtures::fixture_services_view(),
    )
}

fn db_service(pool: &systemprompt_database::DbPool, instance: &InstanceId) -> AgentDatabaseService {
    let repo = AgentServiceRepository::new(pool, instance.clone());
    AgentDatabaseService::new(repo, &systemprompt_test_fixtures::fixture_services_view())
}

async fn ephemeral_listener() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

#[tokio::test]
async fn child_grace_is_the_profiles_shutdown_child_grace() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let lc = lifecycle(&pool, &instance);

    assert_eq!(
        lc.child_grace(),
        crate::spawn_environment().config.shutdown.child_grace()
    );
}

#[tokio::test]
async fn validate_prerequisites_free_port_is_ok() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool, &instance);

    let (listener, port) = ephemeral_listener().await;
    drop(listener);

    lc.validate_prerequisites(&AgentName::new("lc_prereq"), port)
        .await
        .expect("free port");
}

#[tokio::test]
async fn validate_prerequisites_port_held_by_non_agent_fails() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool, &instance);

    let (listener, port) = ephemeral_listener().await;
    let result = lc
        .validate_prerequisites(&AgentName::new("lc_prereq"), port)
        .await;
    drop(listener);

    assert!(
        result.is_err(),
        "a port held by a non-agent process must not be reclaimed"
    );
}

#[tokio::test]
async fn verify_startup_refuses_a_port_answered_by_a_foreign_process() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool, &instance);

    let (listener, port) = ephemeral_listener().await;
    let accept_loop = tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });

    let err = lc
        .verify_startup(&AgentName::new("lc_verify_foreign"), DEAD_PID, port)
        .await
        .expect_err("a listener that is not the spawned agent is not readiness");
    accept_loop.abort();
    match err {
        OrchestrationError::PortHeldByForeignProcess {
            port: held, pid, ..
        } => {
            assert_eq!(held, port);
            assert_eq!(pid, std::process::id());
        },
        other => panic!("expected PortHeldByForeignProcess, got {other:?}"),
    }
}

#[cfg(unix)]
const LISTENING_AGENT_HELPER: &str =
    "services::agent_orchestration::lifecycle_verification::listening_agent_helper";

#[cfg(unix)]
const LISTEN_PORT_FILE_ENV: &str = "SYSTEMPROMPT_TEST_AGENT_PORT_FILE";

#[cfg(unix)]
#[test]
#[ignore = "re-executed as a marked agent that listens on a port"]
fn listening_agent_helper() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let port_file = std::env::var(LISTEN_PORT_FILE_ENV).expect("port file path");
    std::fs::write(port_file, port.to_string()).expect("write port");
    systemprompt_test_fixtures::announce_helper_ready();
    std::thread::sleep(std::time::Duration::from_secs(120));
    drop(listener);
}

#[cfg(unix)]
#[tokio::test]
async fn verify_startup_accepts_the_spawned_agent_listening_on_its_port() {
    use std::process::{Command, Stdio};

    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool, &instance);
    let name = unique_agent_name("lc_verify_owned");

    let helper = systemprompt_test_fixtures::helper(LISTENING_AGENT_HELPER);
    let port_dir = tempfile::tempdir().expect("tempdir");
    let port_file = port_dir.path().join("port");
    let mut child = Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", LISTENING_AGENT_HELPER, "--ignored"])
        .env(
            systemprompt_test_fixtures::HELPER_READY_ENV,
            helper.ready_path(),
        )
        .env(LISTEN_PORT_FILE_ENV, &port_file)
        .env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1")
        .env(
            systemprompt_models::subprocess::AGENT_NAME_ENV,
            name.as_str(),
        )
        .env(
            systemprompt_models::subprocess::SPAWN_NONCE_ENV,
            systemprompt_test_fixtures::test_spawner().nonce().as_str(),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn listening agent helper");
    helper.await_ready();
    let port: u16 = std::fs::read_to_string(&port_file)
        .expect("read port")
        .parse()
        .expect("port");

    let verdict = lc.verify_startup(&name, child.id(), port).await;
    child.kill().expect("kill the listening agent helper");
    child.wait().expect("reap the listening agent helper");
    verdict.expect("the spawned, marked agent listening on its port is ready");
}

#[tokio::test]
async fn verify_startup_times_out_without_clearing_the_pid() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool, &instance);
    let db = db_service(&pool, &instance);

    let name = unique_agent_name("lc_verify_dead");
    db.register_agent_starting(&name, DEAD_PID, &SpawnNonce::generate(), 39461)
        .await
        .expect("register");

    let (listener, port) = ephemeral_listener().await;
    drop(listener);

    let err = lc
        .verify_startup(&name, DEAD_PID, port)
        .await
        .expect_err("nothing listens on the probed port");
    assert!(err.to_string().contains(name.as_str()));

    let row = db
        .repository
        .find_agent_status(&name)
        .await
        .expect("status")
        .expect("row");
    assert_eq!(
        row.pid,
        Some(i32::try_from(DEAD_PID).expect("pid fits")),
        "the caller reaps the spawned process before the PID is cleared"
    );

    db.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn log_startup_failure_covers_stored_statuses() {
    let pool = test_db_pool().await;
    let instance = unique_instance();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let lc = lifecycle(&pool, &instance);
    let db = db_service(&pool, &instance);

    lc.log_startup_failure(&AgentName::new("lc_log_missing"), 39462)
        .await;

    let running = unique_agent_name("lc_log_running");
    let agent = systemprompt_test_fixtures::spawn_marked_agent(AGENT_HELPER, running.as_str());
    db.register_agent(&running, agent.pid(), &agent.nonce, 39463)
        .await
        .expect("register running");
    lc.log_startup_failure(&running, 39463).await;
    db.remove_agent_service(&running).await.ok();

    let dead = unique_agent_name("lc_log_dead");
    db.register_agent(&dead, DEAD_PID, &SpawnNonce::generate(), 39464)
        .await
        .expect("register dead");
    lc.log_startup_failure(&dead, 39464).await;
    db.remove_agent_service(&dead).await.ok();
}
