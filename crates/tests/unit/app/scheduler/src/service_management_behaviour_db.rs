//! Behavioural DB-backed tests for [`ServiceManagementService`] stop and
//! cleanup paths.
//!
//! Direct stop cases use unique rows in the shared fixture database and only
//! dead PIDs. Bulk orphan sweeps use a private disposable database so they
//! cannot discover another test's owned child. The live-service cases signal
//! only the exact marked child process the test spawned; a stop that returns
//! has already reaped it, so death is asserted with `is_running`, never by
//! waiting on the `Child`. Assertions cover the typed stop outcome, durable
//! service state and cleanup dispositions.

use systemprompt_database::{
    CreateServiceInput, ServiceConfig, ServiceModule, ServiceRepository, ServiceStatus,
};
use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome, Termination};
use systemprompt_scheduler::{ApiListenerStop, OrphanDisposition, ServiceManagementService};
use systemprompt_test_fixtures::{test_db_pool, unique_instance};

// A PID that is never a live process: kill(2) on i32::MAX fails with ESRCH.
const DEAD_PID: i32 = i32::MAX;

fn unique_name(prefix: &str) -> ServiceName {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    ServiceName::new(format!("{prefix}-{}-{}", std::process::id(), n))
}

fn config_with_pid(
    instance: &InstanceId,
    name: &ServiceName,
    module: ServiceModule,
    port: i32,
    pid: Option<i32>,
) -> ServiceConfig {
    ServiceConfig {
        instance_id: instance.clone(),
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
        let test_instance = unique_instance();
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        ));
        let repo = ServiceRepository::new(&pool, test_instance.clone());

        let name = unique_name("stop-no-pid");
        seed_running_row(&repo, &name, ServiceModule::Mcp, 0, None).await;

        let config = config_with_pid(&test_instance, &name, ServiceModule::Mcp, 0, None);
        let outcome = svc
            .stop_service(&config, false)
            .await
            .expect("stop_service must succeed for a pid-less service");
        assert_eq!(outcome, StopOutcome::NotRunning);

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
        let test_instance = unique_instance();
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        ));
        let repo = ServiceRepository::new(&pool, test_instance.clone());

        let name = unique_name("stop-dead-pid");
        seed_running_row(&repo, &name, ServiceModule::Mcp, 0, Some(DEAD_PID)).await;

        let config = config_with_pid(&test_instance, &name, ServiceModule::Mcp, 0, Some(DEAD_PID));
        let outcome = svc
            .stop_service(&config, true)
            .await
            .expect("stop_service must succeed even with force and a dead pid");
        assert_eq!(
            outcome,
            StopOutcome::NotRunning,
            "a dead pid is never signalled; the row is still transitioned to stopped"
        );

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
            unique_instance(),
        ));

        let config = config_with_pid(
            &unique_instance(),
            &ServiceName::new("orphan-no-pid-never-seeded"),
            ServiceModule::Mcp,
            0,
            None,
        );
        let outcome = svc
            .cleanup_orphaned_service(&config)
            .await
            .expect("cleanup_orphaned_service must succeed");

        assert_eq!(
            outcome, None,
            "a service with no stored PID is not an orphan to clean up"
        );
    }

    #[tokio::test]
    async fn cleanup_orphaned_service_with_dead_pid_marks_stopped_and_returns_true() {
        let test_instance = unique_instance();
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        ));
        let repo = ServiceRepository::new(&pool, test_instance.clone());

        let name = unique_name("orphan-dead-pid");
        seed_running_row(&repo, &name, ServiceModule::Agent, 0, Some(DEAD_PID)).await;

        let config = config_with_pid(
            &test_instance,
            &name,
            ServiceModule::Agent,
            0,
            Some(DEAD_PID),
        );
        let outcome = svc
            .cleanup_orphaned_service(&config)
            .await
            .expect("cleanup_orphaned_service must succeed");

        assert_eq!(
            outcome,
            Some(StopOutcome::NotRunning),
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
        let test_instance = unique_instance();
        let database =
            systemprompt_test_fixtures::DisposableDb::with_schema("scheduler_orphans_stale").await;
        let pool = database.test_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        ));
        let repo = ServiceRepository::new(&pool, test_instance.clone());

        let name = unique_name("orphans-stale");
        seed_running_row(&repo, &name, ServiceModule::Mcp, 0, Some(DEAD_PID)).await;

        // Sweep on a port nothing in this test holds; the seeded row has a dead
        // stored PID so it is classified as a StaleEntry and marked stopped.
        let report = svc
            .cleanup_all_orphans(0)
            .await
            .expect("cleanup_all_orphans must succeed");

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
        let listeners = ServiceManagementService::stop_api_by_port(1, false)
            .await
            .expect("stop_api_by_port on a free port must succeed");
        assert!(
            listeners.is_empty(),
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

    // Why: macOS withholds the environment of hardened system binaries such as
    // `/bin/sleep`, so a marked child must be an ordinary interpreter, and it is
    // returned only once that interpreter (not its launcher) is the live image.
    fn spawn_marked_sleep(service_name: &ServiceName) -> Child {
        use std::io::{BufRead, BufReader};
        let mut command = Command::new("python3");
        command
            .args([
                "-c",
                "import time\nprint('ready', flush=True)\ntime.sleep(30)",
            ])
            .stdout(Stdio::piped());
        subprocess::mark_child(&mut command, ChildKind::Agent, service_name);
        let mut child = command.spawn().expect("spawn marked python child");
        let stdout = child.stdout.take().expect("marked child stdout");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("marked child reports ready");
        child
    }

    fn spawn_unmarked_sleep() -> Child {
        Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn unmarked sleep child")
    }

    async fn assert_stopped(pid: u32) {
        assert!(
            !subprocess::is_running(pid).await,
            "the marked child must be gone once the stop returns"
        );
    }

    #[tokio::test]
    async fn stop_service_gracefully_terminates_a_marked_live_child() {
        let test_instance = unique_instance();
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        ));
        let repo = ServiceRepository::new(&pool, test_instance.clone());

        let name = unique_name("smb-live-graceful");
        let child = spawn_marked_sleep(&name);
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27201, Some(pid)).await;

        let config = config_with_pid(
            &test_instance,
            &name,
            ServiceModule::Agent,
            27201,
            Some(pid),
        );
        let outcome = svc
            .stop_service(&config, false)
            .await
            .expect("stop_service");

        assert!(matches!(outcome, StopOutcome::Stopped(_)), "{outcome:?}");
        drop(child);
        assert_stopped(pid as u32).await;

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
        let test_instance = unique_instance();
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        ));
        let repo = ServiceRepository::new(&pool, test_instance.clone());

        let name = unique_name("smb-live-force");
        let child = spawn_marked_sleep(&name);
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27202, Some(pid)).await;

        let config = config_with_pid(
            &test_instance,
            &name,
            ServiceModule::Agent,
            27202,
            Some(pid),
        );
        let outcome = svc
            .stop_service(&config, true)
            .await
            .expect("stop_service force");

        assert!(matches!(outcome, StopOutcome::Stopped(_)), "{outcome:?}");
        drop(child);
        assert_stopped(pid as u32).await;

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
        let test_instance = unique_instance();
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        ));
        let repo = ServiceRepository::new(&pool, test_instance.clone());

        let name = unique_name("smb-live-unmarked");
        let mut child = spawn_unmarked_sleep();
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27203, Some(pid)).await;

        let config = config_with_pid(
            &test_instance,
            &name,
            ServiceModule::Agent,
            27203,
            Some(pid),
        );
        let outcome = svc
            .stop_service(&config, false)
            .await
            .expect("stop_service");

        assert_eq!(outcome, StopOutcome::NotOurs);
        assert!(
            subprocess::is_running(pid as u32).await,
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
        let test_instance = unique_instance();
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        ));
        let repo = ServiceRepository::new(&pool, test_instance.clone());

        let name = unique_name("smb-live-orphan");
        let child = spawn_marked_sleep(&name);
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27204, Some(pid)).await;

        let config = config_with_pid(
            &test_instance,
            &name,
            ServiceModule::Agent,
            27204,
            Some(pid),
        );
        let outcome = svc
            .cleanup_orphaned_service(&config)
            .await
            .expect("cleanup_orphaned_service");
        assert!(
            matches!(outcome, Some(StopOutcome::Stopped(_))),
            "a live orphan must be reported as stopped, got {outcome:?}"
        );
        drop(child);
        assert_stopped(pid as u32).await;

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
        let test_instance = unique_instance();
        let database =
            systemprompt_test_fixtures::DisposableDb::with_schema("scheduler_orphans_live").await;
        let pool = database.test_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        ));
        let repo = ServiceRepository::new(&pool, test_instance.clone());

        let name = unique_name("smb-live-sweep");
        let child = spawn_marked_sleep(&name);
        let pid = child.id() as i32;
        seed_running_row(&repo, &name, ServiceModule::Agent, 27205, Some(pid)).await;

        let report = svc
            .cleanup_all_orphans(0)
            .await
            .expect("cleanup_all_orphans");

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

        drop(child);
        assert_stopped(pid as u32).await;
        repo.delete_service(&name).await.expect("cleanup row");
        drop(svc);
        drop(repo);
        pool.write_pool().close().await;
        drop(pool);
        database.drop_now().await;
    }

    fn spawn_port_holder(stamped_as_api: bool) -> (Child, u16) {
        let mut command = Command::new("python3");
        command
            .args([
                "-c",
                "import socket,sys,time\ns=socket.socket()\ns.bind(('127.0.0.1',0))\nprint(s.getsockname()[1],flush=True)\ns.listen(1)\ntime.sleep(60)",
            ])
            .stdout(Stdio::piped());
        if stamped_as_api {
            subprocess::mark_child(
                &mut command,
                ChildKind::Api,
                &subprocess::api_server_service(),
            );
        }
        let mut child = command.spawn().expect("spawn python3 port holder");
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
        let pool = test_db_pool().await;
        let _ = pool;

        let (child, port) = spawn_port_holder(true);
        let pid = child.id();

        let stopped = ServiceManagementService::stop_api_by_port(port, false)
            .await
            .expect("stop_api_by_port must free the port");
        assert_eq!(
            stopped,
            vec![ApiListenerStop {
                pid,
                outcome: StopOutcome::Stopped(Termination::Exited),
            }],
            "the stamped listener PID must be reported stopped"
        );

        drop(child);
        assert_stopped(pid).await;
    }

    #[tokio::test]
    async fn stop_api_by_port_force_kills_the_listener() {
        let pool = test_db_pool().await;
        let _ = pool;

        let (child, port) = spawn_port_holder(true);
        let pid = child.id();

        let stopped = ServiceManagementService::stop_api_by_port(port, true)
            .await
            .expect("forced stop_api_by_port must free the port");
        assert!(matches!(
            stopped.as_slice(),
            [ApiListenerStop { pid: stopped_pid, outcome: StopOutcome::Stopped(_) }]
                if *stopped_pid == pid
        ));

        drop(child);
        assert_stopped(pid).await;
    }

    #[tokio::test]
    async fn stop_api_by_port_leaves_an_unverified_listener_running() {
        let (mut child, port) = spawn_port_holder(false);
        let pid = child.id();

        let stopped = ServiceManagementService::stop_api_by_port(port, true)
            .await
            .expect("an unverified listener is reported, not an error");
        assert_eq!(
            stopped,
            vec![ApiListenerStop {
                pid,
                outcome: StopOutcome::NotOurs,
            }]
        );
        assert!(subprocess::is_running(pid).await);

        child.kill().expect("kill the test's own listener");
        child.wait().expect("reap the test's own listener");
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
            unique_instance(),
        ));

        svc.stop_service(
            &config_with_pid(
                &unique_instance(),
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
            unique_instance(),
        ));

        let outcome = svc
            .cleanup_orphaned_service(&config_with_pid(
                &unique_instance(),
                &ServiceName::new("smb-dead-pool-orphan"),
                ServiceModule::Agent,
                27302,
                Some(DEAD_PID),
            ))
            .await
            .expect("a failed stopped-mark is logged, not propagated");
        assert_eq!(
            outcome,
            Some(StopOutcome::NotRunning),
            "a dead-PID orphan still counts as acted upon"
        );
    }
}
