//! Behavioural DB-backed tests for [`ServiceManagementService`] stop/cleanup
//! paths that mutate the shared `services` table.
//!
//! These drive the decision logic in `services/service_management.rs` without
//! ever signalling a real, unrelated process: every seeded PID is either absent
//! (no stored PID) or a guaranteed-dead PID (`i32::MAX`), so the
//! `process_exists` / `pid_is_our_service` guards short-circuit before any
//! `kill(2)`. We then assert the observable outcome — the DB row is marked
//! `stopped` and the returned [`OrphanCleanupReport`] records the disposition.
//!
//! Tests seed and tear down their own rows and join the serialized
//! `scheduler-services-db` nextest group (the `services` table is shared and
//! `cleanup_all_orphans` sweeps it). They early-return when `DATABASE_URL` is
//! unset.

use systemprompt_database::{CreateServiceInput, ServiceConfig, ServiceRepository};
use systemprompt_scheduler::{OrphanDisposition, ServiceManagementService};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

// A PID that is never a live process: kill(2) on i32::MAX fails with ESRCH.
const DEAD_PID: i32 = i32::MAX;

macro_rules! pool_or_skip {
    () => {{
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(pool) = fixture_db_pool(&url).await else {
            return;
        };
        pool
    }};
}

fn unique_name(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{}", std::process::id(), n)
}

fn config_with_pid(name: &str, module: &str, port: i32, pid: Option<i32>) -> ServiceConfig {
    ServiceConfig {
        name: name.to_owned(),
        module_name: module.to_owned(),
        status: "running".to_owned(),
        pid,
        port,
        binary_mtime: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

async fn seed_running_row(
    repo: &ServiceRepository,
    name: &str,
    module: &str,
    port: u16,
    pid: Option<i32>,
) {
    repo.create_service(CreateServiceInput {
        name,
        module_name: module,
        status: "running",
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
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("construct");
        let repo = ServiceRepository::new(&pool).expect("repo");

        let name = unique_name("stop-no-pid");
        seed_running_row(&repo, &name, "mcp", 0, None).await;

        let config = config_with_pid(&name, "mcp", 0, None);
        svc.stop_service(&config, false)
            .await
            .expect("stop_service must succeed for a pid-less service");

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("read back")
            .expect("row must still exist");
        assert_eq!(
            row.status, "stopped",
            "stop_service must mark a pid-less service stopped"
        );

        repo.delete_service(&name).await.expect("cleanup");
    }

    #[tokio::test]
    async fn stop_service_with_dead_pid_marks_row_stopped() {
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("construct");
        let repo = ServiceRepository::new(&pool).expect("repo");

        let name = unique_name("stop-dead-pid");
        seed_running_row(&repo, &name, "mcp", 0, Some(DEAD_PID)).await;

        // DEAD_PID does not exist → process_exists short-circuits, no signal is
        // sent, and the row is still transitioned to stopped.
        let config = config_with_pid(&name, "mcp", 0, Some(DEAD_PID));
        svc.stop_service(&config, true)
            .await
            .expect("stop_service must succeed even with force and a dead pid");

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("read back")
            .expect("row exists");
        assert_eq!(row.status, "stopped");

        repo.delete_service(&name).await.expect("cleanup");
    }

    #[tokio::test]
    async fn stop_service_unknown_module_does_not_signal_and_marks_stopped() {
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("construct");
        let repo = ServiceRepository::new(&pool).expect("repo");

        // module "worker" has no subprocess identity marker → pid_is_our_service
        // returns false → the stored PID is cleared without signalling. We use
        // our OWN live PID to prove the unknown-module guard, not pid liveness,
        // is what suppresses the signal: the process must survive.
        let name = unique_name("stop-unknown-mod");
        let own_pid = i32::try_from(std::process::id()).expect("pid fits i32");
        seed_running_row(&repo, &name, "worker", 0, Some(own_pid)).await;

        let config = config_with_pid(&name, "worker", 0, Some(own_pid));
        svc.stop_service(&config, true)
            .await
            .expect("stop_service must succeed for an unknown module type");

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("read back")
            .expect("row exists");
        assert_eq!(row.status, "stopped");

        repo.delete_service(&name).await.expect("cleanup");
    }

    #[tokio::test]
    async fn cleanup_orphaned_service_without_pid_returns_false() {
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("construct");

        let config = config_with_pid("orphan-no-pid-never-seeded", "mcp", 0, None);
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
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("construct");
        let repo = ServiceRepository::new(&pool).expect("repo");

        let name = unique_name("orphan-dead-pid");
        seed_running_row(&repo, &name, "agent", 0, Some(DEAD_PID)).await;

        let config = config_with_pid(&name, "agent", 0, Some(DEAD_PID));
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
        assert_eq!(row.status, "stopped");

        repo.delete_service(&name).await.expect("cleanup");
    }

    #[tokio::test]
    async fn cleanup_all_orphans_reports_stale_entry_for_dead_pid_row() {
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("construct");
        let repo = ServiceRepository::new(&pool).expect("repo");

        let name = unique_name("orphans-stale");
        seed_running_row(&repo, &name, "mcp", 0, Some(DEAD_PID)).await;

        // Sweep on a port nothing in this test holds; the seeded row has a dead
        // stored PID so it is classified as a StaleEntry and marked stopped.
        let report = svc
            .cleanup_all_orphans(0)
            .await
            .expect("cleanup_all_orphans must succeed");

        let outcome = report
            .outcomes
            .iter()
            .find(|o| o.name == name)
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
            row.status, "stopped",
            "cleanup_all_orphans must mark the stale running row stopped"
        );

        // services_cleaned counts outcomes plus an api_stopped flag.
        assert!(
            report.services_cleaned() >= report.outcomes.len(),
            "services_cleaned must be at least the number of outcomes"
        );

        repo.delete_service(&name).await.expect("cleanup");
    }

    #[tokio::test]
    async fn stop_api_by_port_on_free_port_reports_no_listener() {
        let _pool = pool_or_skip!();

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
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("service");
        let repo = ServiceRepository::new(&pool).expect("repo");

        let name = unique_name("smb-live-graceful");
        let mut child = spawn_marked_sleep(&name);
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, "agent", 27201, Some(pid)).await;

        svc.stop_service(&config_with_pid(&name, "agent", 27201, Some(pid)), false)
            .await
            .expect("stop_service");

        wait_until_dead(&mut child).await;

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("find service")
            .expect("row present");
        assert_eq!(row.status, "stopped");

        repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn stop_service_force_kills_a_marked_live_child() {
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("service");
        let repo = ServiceRepository::new(&pool).expect("repo");

        let name = unique_name("smb-live-force");
        let mut child = spawn_marked_sleep(&name);
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, "agent", 27202, Some(pid)).await;

        svc.stop_service(&config_with_pid(&name, "agent", 27202, Some(pid)), true)
            .await
            .expect("stop_service force");

        wait_until_dead(&mut child).await;

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("find service")
            .expect("row present");
        assert_eq!(row.status, "stopped");

        repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn stop_service_refuses_to_signal_a_live_pid_without_spawn_markers() {
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("service");
        let repo = ServiceRepository::new(&pool).expect("repo");

        let name = unique_name("smb-live-unmarked");
        let mut child = spawn_unmarked_sleep();
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, "agent", 27203, Some(pid)).await;

        svc.stop_service(&config_with_pid(&name, "agent", 27203, Some(pid)), false)
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
        assert_eq!(row.status, "stopped", "the row is still marked stopped");

        child.kill().ok();
        let _ = child.wait();
        repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn cleanup_orphaned_service_terminates_a_marked_live_child() {
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("service");
        let repo = ServiceRepository::new(&pool).expect("repo");

        let name = unique_name("smb-live-orphan");
        let mut child = spawn_marked_sleep(&name);
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, "agent", 27204, Some(pid)).await;

        let acted = svc
            .cleanup_orphaned_service(&config_with_pid(&name, "agent", 27204, Some(pid)))
            .await
            .expect("cleanup_orphaned_service");
        assert!(acted, "a live orphan must be reported as acted upon");

        wait_until_dead(&mut child).await;

        let row = repo
            .find_service_by_name(&name)
            .await
            .expect("find service")
            .expect("row present");
        assert_eq!(row.status, "stopped");

        repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn cleanup_all_orphans_stops_a_row_with_a_live_marked_pid() {
        let pool = pool_or_skip!();
        let svc = ServiceManagementService::new(&pool).expect("service");
        let repo = ServiceRepository::new(&pool).expect("repo");

        let name = unique_name("smb-live-sweep");
        let mut child = spawn_marked_sleep(&name);
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, "agent", 27205, Some(pid)).await;

        let report = svc
            .cleanup_all_orphans(27206)
            .await
            .expect("cleanup_all_orphans");

        let outcome = report
            .outcomes
            .iter()
            .find(|o| o.name == name)
            .expect("the live-pid row must appear in the report");
        assert_eq!(
            outcome.disposition,
            OrphanDisposition::Stopped,
            "a row whose PID is a live verified child is Stopped, not StaleEntry"
        );

        wait_until_dead(&mut child).await;
        repo.delete_service(&name).await.expect("cleanup row");
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

    #[tokio::test]
    async fn stop_api_by_port_terminates_the_listener_gracefully() {
        let pool = pool_or_skip!();
        let _ = pool;

        let (mut child, port) = spawn_port_holder();
        let pid = child.id();

        let stopped = ServiceManagementService::stop_api_by_port(port, false)
            .await
            .expect("stop_api_by_port must free the port");
        assert_eq!(stopped, Some(pid), "the listener PID must be reported");

        wait_until_dead(&mut child).await;
    }

    #[tokio::test]
    async fn stop_api_by_port_force_kills_the_listener() {
        let pool = pool_or_skip!();
        let _ = pool;

        let (mut child, port) = spawn_port_holder();
        let pid = child.id();

        let stopped = ServiceManagementService::stop_api_by_port(port, true)
            .await
            .expect("forced stop_api_by_port must free the port");
        assert_eq!(stopped, Some(pid));

        wait_until_dead(&mut child).await;
    }
}

mod dead_pool_degradation {
    use super::*;
    use systemprompt_test_fixtures::closed_db_pool;

    #[tokio::test]
    async fn stop_service_still_succeeds_when_the_row_update_fails() {
        let Ok(_url) = fixture_database_url() else {
            return;
        };
        let closed = closed_db_pool().await;
        let svc = ServiceManagementService::new(&closed).expect("service");

        svc.stop_service(
            &config_with_pid("smb-dead-pool-stop", "agent", 27301, None),
            false,
        )
        .await
        .expect("a failed stopped-mark is logged, not propagated");
    }

    #[tokio::test]
    async fn cleanup_orphaned_service_reports_action_when_the_row_update_fails() {
        let Ok(_url) = fixture_database_url() else {
            return;
        };
        let closed = closed_db_pool().await;
        let svc = ServiceManagementService::new(&closed).expect("service");

        let acted = svc
            .cleanup_orphaned_service(&config_with_pid(
                "smb-dead-pool-orphan",
                "agent",
                27302,
                Some(DEAD_PID),
            ))
            .await
            .expect("a failed stopped-mark is logged, not propagated");
        assert!(acted, "a dead-PID orphan still counts as acted upon");
    }
}
