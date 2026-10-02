//! Behavioural DB-backed tests for [`ServiceManagementService`] stop and
//! cleanup paths.
//!
//! Direct stop cases use unique rows in the shared fixture database and only
//! dead PIDs. Bulk orphan sweeps use a private disposable database so they
//! cannot discover another test's owned child. Their API-wide fallback runs
//! through a PATH-scoped recording `pkill` shim, while the live-service case
//! may signal only the exact marked child process that the test spawned and
//! reaps during teardown. Assertions cover both durable service state and
//! cleanup dispositions.

use systemprompt_database::{
    CreateServiceInput, ServiceConfig, ServiceModule, ServiceRepository, ServiceStatus,
};
use systemprompt_identifiers::ServiceName;
use systemprompt_scheduler::{OrphanDisposition, SchedulerError, ServiceManagementService};
use systemprompt_test_fixtures::test_db_pool;

// A PID that is never a live process: kill(2) on i32::MAX fails with ESRCH.
const DEAD_PID: i32 = i32::MAX;

#[cfg(unix)]
struct PkillShim {
    original_path: Option<std::ffi::OsString>,
    invocation: std::path::PathBuf,
    _directory: tempfile::TempDir,
}

#[cfg(unix)]
impl PkillShim {
    fn install() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().expect("private pkill shim directory");
        let invocation = directory.path().join("pkill-invocation");
        let executable = directory.path().join("pkill");
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 1\n",
                invocation.display()
            ),
        )
        .expect("write pkill shim");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .expect("make pkill shim executable");
        let original_path = std::env::var_os("PATH");
        let mut paths = vec![directory.path().to_path_buf()];
        if let Some(path) = &original_path {
            paths.extend(std::env::split_paths(path));
        }
        let path = std::env::join_paths(paths).expect("compose shim PATH");
        unsafe { std::env::set_var("PATH", path) };
        Self {
            original_path,
            invocation,
            _directory: directory,
        }
    }

    fn assert_unsafe_api_pattern_was_not_dispatched(&self) {
        assert!(
            !self.invocation.exists(),
            "the space-containing API process pattern must be rejected as unsafe before invoking pkill"
        );
    }
}

#[cfg(unix)]
impl Drop for PkillShim {
    fn drop(&mut self) {
        match self.original_path.take() {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }
    }
}

fn unique_name(prefix: &str) -> ServiceName {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    ServiceName::new(format!("{prefix}-{}-{}", std::process::id(), n))
}

fn config_with_pid(
    name: &ServiceName,
    module: ServiceModule,
    port: i32,
    pid: Option<i32>,
) -> ServiceConfig {
    ServiceConfig {
        instance_id: systemprompt_identifiers::InstanceId::new("test-instance"),
        name: name.clone(),
        module_name: module,
        status: ServiceStatus::Running,
        pid,
        port,
        binary_mtime: None,
        created_at: String::new(),
        heartbeat_at: String::new(),
        updated_at: String::new(),
    }
}

async fn seed_running_row(
    repo: &ServiceRepository,
    name: &ServiceName,
    module: ServiceModule,
    port: u16,
    pid: Option<i32>,
) {
    repo.create_service(CreateServiceInput {
        name,
        module_name: module,
        status: ServiceStatus::Running,
        port,
        binary_mtime: None,
    })
    .await
    .expect("seed service row");
    if let Some(pid) = pid {
        repo.update_service_pid(name, pid)
            .await
            .expect("seed service pid");
    }
}

mod service_management_behaviour_db {
    use super::*;

    #[tokio::test]
    async fn stop_service_without_pid_marks_row_stopped() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));
        let repo = ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        );

        let name = unique_name("stop-no-pid");
        seed_running_row(&repo, &name, ServiceModule::Mcp, 0, None).await;

        let config = config_with_pid(&name, ServiceModule::Mcp, 0, None);
        svc.stop_service(&config, false)
            .await
            .expect("stop_service must succeed for a pid-less service");

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("read back")
            .expect("row must still exist");
        assert_eq!(
            row.status,
            ServiceStatus::Stopped,
            "stop_service must mark a pid-less service stopped"
        );

        repo.delete_service(&name).await.expect("cleanup");
    }

    #[tokio::test]
    async fn stop_service_with_dead_pid_marks_row_stopped() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));
        let repo = ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        );

        let name = unique_name("stop-dead-pid");
        seed_running_row(&repo, &name, ServiceModule::Mcp, 0, Some(DEAD_PID)).await;

        // DEAD_PID does not exist → process_exists short-circuits, no signal is
        // sent, and the row is still transitioned to stopped.
        let config = config_with_pid(&name, ServiceModule::Mcp, 0, Some(DEAD_PID));
        svc.stop_service(&config, true)
            .await
            .expect("stop_service must succeed even with force and a dead pid");

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("read back")
            .expect("row exists");
        assert_eq!(row.status, ServiceStatus::Stopped);

        repo.delete_service(&name).await.expect("cleanup");
    }

    #[tokio::test]
    async fn cleanup_orphaned_service_without_pid_returns_false() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));

        let config = config_with_pid(
            &ServiceName::new("orphan-no-pid-never-seeded"),
            ServiceModule::Mcp,
            0,
            None,
        );
        let cleaned = svc
            .cleanup_orphaned_service(&config)
            .await
            .expect("cleanup_orphaned_service must succeed");

        assert!(
            !cleaned,
            "a service with no stored PID is not an orphan to clean up"
        );
    }

    #[tokio::test]
    async fn cleanup_orphaned_service_with_dead_pid_marks_stopped_and_returns_true() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));
        let repo = ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        );

        let name = unique_name("orphan-dead-pid");
        seed_running_row(&repo, &name, ServiceModule::Agent, 0, Some(DEAD_PID)).await;

        let config = config_with_pid(&name, ServiceModule::Agent, 0, Some(DEAD_PID));
        let cleaned = svc
            .cleanup_orphaned_service(&config)
            .await
            .expect("cleanup_orphaned_service must succeed");

        assert!(
            cleaned,
            "a stored-but-dead PID is a stale orphan and must be reported cleaned"
        );
        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("read back")
            .expect("row exists");
        assert_eq!(row.status, ServiceStatus::Stopped);

        repo.delete_service(&name).await.expect("cleanup");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cleanup_all_orphans_reports_stale_entry_for_dead_pid_row() {
        let pkill = PkillShim::install();
        let database =
            systemprompt_test_fixtures::DisposableDb::with_schema("scheduler_orphans_stale").await;
        let pool = database.test_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));
        let repo = ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        );

        let name = unique_name("orphans-stale");
        seed_running_row(&repo, &name, ServiceModule::Mcp, 0, Some(DEAD_PID)).await;

        // Sweep on a port nothing in this test holds; the seeded row has a dead
        // stored PID so it is classified as a StaleEntry and marked stopped.
        let report = svc
            .cleanup_all_orphans(0)
            .await
            .expect("cleanup_all_orphans must succeed");
        pkill.assert_unsafe_api_pattern_was_not_dispatched();

        let outcome = report
            .outcomes
            .iter()
            .find(|o| o.name == name.as_str())
            .expect("our seeded running row must appear in the orphan outcomes");
        assert_eq!(
            outcome.disposition,
            OrphanDisposition::StaleEntry,
            "a row whose stored PID is dead must be classified StaleEntry"
        );
        assert_eq!(outcome.pid, DEAD_PID);

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("read back")
            .expect("row exists");
        assert_eq!(
            row.status,
            ServiceStatus::Stopped,
            "cleanup_all_orphans must mark the stale running row stopped"
        );

        // services_cleaned counts outcomes plus an api_stopped flag.
        assert!(
            report.services_cleaned() >= report.outcomes.len(),
            "services_cleaned must be at least the number of outcomes"
        );

        repo.delete_service(&name).await.expect("cleanup");
        drop(svc);
        drop(repo);
        pool.write_pool().close().await;
        drop(pool);
        database.drop_now().await;
    }

    #[tokio::test]
    async fn stop_api_by_port_on_free_port_reports_no_listener() {
        let _pool = test_db_pool().await;

        // Port 1 is privileged and effectively never bound by this test process,
        // so the static stop-by-port helper finds no listener and returns None
        // after confirming the port is free.
        let listener = ServiceManagementService::stop_api_by_port(1, false)
            .await
            .expect("stop_api_by_port on a free port must succeed");
        assert!(
            listener.is_none(),
            "no process holds port 1, so stop_api_by_port must report no listener"
        );
    }
}

// Live-child tests: every signalled PID is a `sleep`/`python3` child this test
// spawned itself, so the PID-identity guard is exercised against real spawn
// markers without ever touching an unrelated process.
#[cfg(unix)]
mod live_child_stop_paths {
    use super::*;

    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use systemprompt_scheduler::ProcessCleanup;

    fn spawn_marked_sleep(service_name: &str) -> Child {
        Command::new("sleep")
            .arg("30")
            .env("SYSTEMPROMPT_SUBPROCESS", "1")
            .env("AGENT_NAME", service_name)
            .spawn()
            .expect("spawn marked sleep child")
    }

    fn spawn_unmarked_sleep() -> Child {
        Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn unmarked sleep child")
    }

    // A killed child is a zombie until reaped (kill(pid, 0) still succeeds),
    // so death is observed via try_wait, never process_exists.
    async fn wait_until_dead(child: &mut Child) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait().expect("try_wait").is_some() {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "child was not terminated within the deadline"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    #[tokio::test]
    async fn stop_service_gracefully_terminates_a_marked_live_child() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));
        let repo = ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        );

        let name = unique_name("smb-live-graceful");
        let mut child = spawn_marked_sleep(name.as_str());
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27201, Some(pid)).await;

        let config = config_with_pid(&name, ServiceModule::Agent, 27201, Some(pid));
        svc.stop_service(&config, false)
            .await
            .expect("stop_service");

        wait_until_dead(&mut child).await;

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("find service")
            .expect("row present");
        assert_eq!(row.status, ServiceStatus::Stopped);

        repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn stop_service_force_kills_a_marked_live_child() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));
        let repo = ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        );

        let name = unique_name("smb-live-force");
        let mut child = spawn_marked_sleep(name.as_str());
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27202, Some(pid)).await;

        let config = config_with_pid(&name, ServiceModule::Agent, 27202, Some(pid));
        svc.stop_service(&config, true)
            .await
            .expect("stop_service force");

        wait_until_dead(&mut child).await;

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("find service")
            .expect("row present");
        assert_eq!(row.status, ServiceStatus::Stopped);

        repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn stop_service_refuses_to_signal_a_live_pid_without_spawn_markers() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));
        let repo = ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        );

        let name = unique_name("smb-live-unmarked");
        let mut child = spawn_unmarked_sleep();
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27203, Some(pid)).await;

        let config = config_with_pid(&name, ServiceModule::Agent, 27203, Some(pid));
        svc.stop_service(&config, false)
            .await
            .expect("stop_service");

        assert!(
            ProcessCleanup::process_exists(pid as u32),
            "a live PID without our spawn markers must never be signalled"
        );
        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("find service")
            .expect("row present");
        assert_eq!(
            row.status,
            ServiceStatus::Stopped,
            "the row is still marked stopped"
        );

        child.kill().ok();
        let _ = child.wait();
        repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn cleanup_orphaned_service_terminates_a_marked_live_child() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));
        let repo = ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        );

        let name = unique_name("smb-live-orphan");
        let mut child = spawn_marked_sleep(name.as_str());
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27204, Some(pid)).await;

        let config = config_with_pid(&name, ServiceModule::Agent, 27204, Some(pid));
        let acted = svc
            .cleanup_orphaned_service(&config)
            .await
            .expect("cleanup_orphaned_service");
        assert!(acted, "a live orphan must be reported as acted upon");

        wait_until_dead(&mut child).await;

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("find service")
            .expect("row present");
        assert_eq!(row.status, ServiceStatus::Stopped);

        repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn cleanup_all_orphans_stops_a_row_with_a_live_marked_pid() {
        let pkill = PkillShim::install();
        let database =
            systemprompt_test_fixtures::DisposableDb::with_schema("scheduler_orphans_live").await;
        let pool = database.test_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));
        let repo = ServiceRepository::new(
            &pool,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        );

        let name = unique_name("smb-live-sweep");
        let mut child = spawn_marked_sleep(name.as_str());
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27205, Some(pid)).await;

        let report = svc
            .cleanup_all_orphans(0)
            .await
            .expect("cleanup_all_orphans");
        pkill.assert_unsafe_api_pattern_was_not_dispatched();

        let outcome = report
            .outcomes
            .iter()
            .find(|o| o.name == name.as_str())
            .expect("the live-pid row must appear in the report");
        assert_eq!(
            outcome.disposition,
            OrphanDisposition::Stopped,
            "a row whose PID is a live verified child is Stopped, not StaleEntry"
        );

        wait_until_dead(&mut child).await;
        repo.delete_service(&name).await.expect("cleanup row");
        drop(svc);
        drop(repo);
        pool.write_pool().close().await;
        drop(pool);
        database.drop_now().await;
    }

    fn spawn_port_holder() -> (Child, u16) {
        let mut child = Command::new("python3")
            .args([
                "-c",
                "import socket,sys,time\ns=socket.socket()\ns.bind(('127.0.0.1',0))\nprint(s.getsockname()[1],flush=True)\ns.listen(1)\ntime.sleep(60)",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn python3 port holder");
        let stdout = child.stdout.take().expect("holder stdout");
        let port = {
            use std::io::{BufRead, BufReader};
            let mut line = String::new();
            BufReader::new(stdout)
                .read_line(&mut line)
                .expect("read holder port");
            line.trim().parse::<u16>().expect("holder port number")
        };
        (child, port)
    }

    async fn assert_foreign_holder_is_refused(force: bool) {
        let (mut child, port) = spawn_port_holder();
        let pid = child.id();

        let err = ServiceManagementService::stop_api_by_port(port, force)
            .await
            .expect_err("a holder that is not a peer instance must be refused");
        assert!(
            matches!(
                err,
                SchedulerError::ForeignPortHolder { port: p, pid: holder } if p == port && holder == pid
            ),
            "the refusal must name the port and its holder: {err}"
        );
        assert!(
            child.try_wait().expect("poll holder").is_none(),
            "a refused holder must not be signalled"
        );

        let _ = child.kill();
        let _ = child.wait();
    }

    #[tokio::test]
    async fn stop_api_by_port_refuses_a_listener_that_is_not_a_peer_instance() {
        assert_foreign_holder_is_refused(false).await;
    }

    #[tokio::test]
    async fn forced_stop_api_by_port_refuses_a_listener_that_is_not_a_peer_instance() {
        assert_foreign_holder_is_refused(true).await;
    }
}

mod dead_pool_degradation {
    use super::*;
    use systemprompt_test_fixtures::closed_db_pool;

    #[tokio::test]
    async fn stop_service_still_succeeds_when_the_row_update_fails() {
        let closed = closed_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &closed,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));

        svc.stop_service(
            &config_with_pid(
                &ServiceName::new("smb-dead-pool-stop"),
                ServiceModule::Agent,
                27301,
                None,
            ),
            false,
        )
        .await
        .expect("a failed stopped-mark is logged, not propagated");
    }

    #[tokio::test]
    async fn cleanup_orphaned_service_reports_action_when_the_row_update_fails() {
        let closed = closed_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &closed,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ));

        let acted = svc
            .cleanup_orphaned_service(&config_with_pid(
                &ServiceName::new("smb-dead-pool-orphan"),
                ServiceModule::Agent,
                27302,
                Some(DEAD_PID),
            ))
            .await
            .expect("a failed stopped-mark is logged, not propagated");
        assert!(acted, "a dead-PID orphan still counts as acted upon");
    }
}
