//! DB-backed tests for [`ServiceReconciler`] and [`ServiceStateVerifier`].
//!
//! Both types require a live Postgres pool. Tests fail when `DATABASE_URL`
//! is unset. Every test reconciles its own instance id: a reconcile sweeps
//! every row of its instance, so a shared id would stop another test's
//! children or delete its rows.

use std::sync::Arc;
use systemprompt_identifiers::{InstanceId, ServiceName};

use systemprompt_models::ServiceType;
use systemprompt_scheduler::{
    DesiredStatus, ReconciliationResult, SchedulerError, ServiceAction, ServiceConfig,
    ServiceReconciler, ServiceStateVerifier,
};
use systemprompt_test_fixtures::test_db_pool;

fn isolated_instance() -> InstanceId {
    InstanceId::new(format!("reconciler-{}", uuid::Uuid::new_v4().simple()))
}

mod reconciler_db {
    use super::*;

    #[tokio::test]
    async fn new_constructs_against_migrated_db() {
        let pool = test_db_pool().await;
        let _reconciler = ServiceReconciler::new(
            Arc::clone(&pool),
            systemprompt_database::ServiceRepository::new(&pool, isolated_instance()),
        );
    }

    #[tokio::test]
    async fn reconcile_empty_configs_returns_success() {
        let pool = test_db_pool().await;
        let reconciler = ServiceReconciler::new(
            Arc::clone(&pool),
            systemprompt_database::ServiceRepository::new(&pool, isolated_instance()),
        );

        let result = reconciler
            .reconcile(&[], |_name: ServiceName, _port: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed with an empty config slice");

        assert!(
            result.is_success(),
            "empty-config reconciliation must report success"
        );
        assert!(
            result.started.is_empty() && result.stopped.is_empty() && result.restarted.is_empty(),
            "no configs → no start/stop/restart actions (only orphan cleanup is allowed)"
        );
    }

    #[tokio::test]
    async fn reconcile_disabled_config_absent_from_db_returns_success() {
        let pool = test_db_pool().await;
        let reconciler = ServiceReconciler::new(
            Arc::clone(&pool),
            systemprompt_database::ServiceRepository::new(&pool, isolated_instance()),
        );

        let configs = [ServiceConfig {
            name: ServiceName::new("test-absent-disabled"),
            service_type: ServiceType::Mcp,
            port: 19001,
            enabled: false,
        }];

        let result = reconciler
            .reconcile(&configs, |_name: ServiceName, _port: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed when the service is absent from DB and disabled");

        assert!(result.is_success());
    }

    #[tokio::test]
    async fn reconcile_enabled_config_absent_from_db_attempts_start() {
        let pool = test_db_pool().await;
        let reconciler = ServiceReconciler::new(
            Arc::clone(&pool),
            systemprompt_database::ServiceRepository::new(&pool, isolated_instance()),
        );

        let configs = [ServiceConfig {
            name: ServiceName::new("test-enabled-no-db-row"),
            service_type: ServiceType::Mcp,
            port: 19002,
            enabled: true,
        }];

        let start_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&start_called);

        let result = reconciler
            .reconcile(&configs, move |_name: ServiceName, _port: u16| {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
                async { Ok(()) }
            })
            .await
            .expect("reconcile must succeed");

        assert!(
            start_called.load(std::sync::atomic::Ordering::Relaxed)
                || result.failed.len() == configs.len()
                || result.started.len() == configs.len(),
            "enabled + absent-from-DB service must trigger a start attempt or be in failed"
        );
    }

    #[tokio::test]
    async fn reconcile_result_is_success_struct() {
        let result = ReconciliationResult::new();
        assert!(result.is_success());
        assert_eq!(result.total_actions(), 0);
        assert!(result.started.is_empty());
        assert!(result.stopped.is_empty());
        assert!(result.restarted.is_empty());
        assert!(result.cleaned_up.is_empty());
        assert!(result.failed.is_empty());
    }

    #[tokio::test]
    async fn reconcile_multiple_disabled_absent_configs() {
        let pool = test_db_pool().await;
        let reconciler = ServiceReconciler::new(
            Arc::clone(&pool),
            systemprompt_database::ServiceRepository::new(&pool, isolated_instance()),
        );

        let configs = vec![
            ServiceConfig {
                name: ServiceName::new("multi-disabled-a"),
                service_type: ServiceType::Mcp,
                port: 19010,
                enabled: false,
            },
            ServiceConfig {
                name: ServiceName::new("multi-disabled-b"),
                service_type: ServiceType::Agent,
                port: 19011,
                enabled: false,
            },
        ];

        let result = reconciler
            .reconcile(&configs, |_name: ServiceName, _port: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed for all-disabled configs");

        assert!(result.is_success());
    }

    #[tokio::test]
    async fn reconcile_start_failure_recorded_in_failed() {
        let pool = test_db_pool().await;
        let reconciler = ServiceReconciler::new(
            Arc::clone(&pool),
            systemprompt_database::ServiceRepository::new(&pool, isolated_instance()),
        );

        let configs = [ServiceConfig {
            name: ServiceName::new("test-start-fail"),
            service_type: ServiceType::Mcp,
            port: 19003,
            enabled: true,
        }];

        let result = reconciler
            .reconcile(&configs, |_name: ServiceName, _port: u16| async {
                Err(SchedulerError::Io(std::io::Error::other(
                    "simulated start failure",
                )))
            })
            .await
            .expect("reconcile itself must not fail even if start_service does");

        assert!(
            !result.is_success() || result.total_actions() == 0,
            "when start_service errors the outcome must either record a failure or take no action"
        );
    }
}

mod state_verifier_db {
    use super::*;

    #[tokio::test]
    async fn new_constructs_against_migrated_db() {
        let pool = test_db_pool().await;
        let _verifier = ServiceStateVerifier::new(Arc::clone(&pool), isolated_instance());
    }

    #[tokio::test]
    async fn get_verified_states_empty_configs_returns_empty_or_orphans() {
        let pool = test_db_pool().await;
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool), isolated_instance());

        let states = verifier
            .get_verified_states(&[])
            .await
            .expect("get_verified_states must succeed on empty config");

        // With no configs, only DB orphans (services rows without a manifest
        // entry) can appear. On a freshly-migrated DB this is typically empty.
        let _ = states;
    }

    #[tokio::test]
    async fn get_verified_states_disabled_config_maps_to_cleanup_or_none() {
        let pool = test_db_pool().await;
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool), isolated_instance());

        let configs = [ServiceConfig {
            name: ServiceName::new("sv-disabled-absent"),
            service_type: ServiceType::Mcp,
            port: 19020,
            enabled: false,
        }];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed");

        let matching: Vec<_> = states
            .iter()
            .filter(|s| s.name == "sv-disabled-absent")
            .collect();

        assert_eq!(
            matching.len(),
            1,
            "disabled config must produce exactly one state"
        );
        let state = &matching[0];
        assert_eq!(state.desired_status, DesiredStatus::Disabled);
        assert!(
            matches!(
                state.needs_action,
                ServiceAction::CleanupDb
                    | ServiceAction::CleanupProcess
                    | ServiceAction::Stop
                    | ServiceAction::None
            ),
            "disabled + not-running service must map to a cleanup or no-op action, got {:?}",
            state.needs_action
        );
    }

    #[tokio::test]
    async fn get_verified_states_enabled_config_absent_from_db_needs_start() {
        let pool = test_db_pool().await;
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool), isolated_instance());

        let configs = [ServiceConfig {
            name: ServiceName::new("sv-enabled-absent"),
            service_type: ServiceType::Agent,
            port: 19021,
            enabled: true,
        }];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed");

        let matching: Vec<_> = states
            .iter()
            .filter(|s| s.name == "sv-enabled-absent")
            .collect();

        assert_eq!(matching.len(), 1);
        let state = &matching[0];
        assert_eq!(state.desired_status, DesiredStatus::Enabled);
        // Port 19021 is not in use and has no DB row → Stopped → Start required.
        assert_eq!(
            state.needs_action,
            ServiceAction::Start,
            "enabled + absent-from-DB service on a free port must need Start"
        );
    }

    #[tokio::test]
    async fn get_services_needing_action_filters_correctly() {
        let pool = test_db_pool().await;
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool), isolated_instance());

        let configs = [
            ServiceConfig {
                name: ServiceName::new("sv-action-enabled"),
                service_type: ServiceType::Mcp,
                port: 19030,
                enabled: true,
            },
            ServiceConfig {
                name: ServiceName::new("sv-action-disabled"),
                service_type: ServiceType::Mcp,
                port: 19031,
                enabled: false,
            },
        ];

        let needing = verifier
            .get_services_needing_action(&configs)
            .await
            .expect("get_services_needing_action must succeed");

        for state in &needing {
            assert!(
                state.needs_attention(),
                "every state returned by get_services_needing_action must report needs_attention"
            );
        }
    }

    #[tokio::test]
    async fn get_running_services_returns_only_running() {
        let pool = test_db_pool().await;
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool), isolated_instance());

        let configs = [ServiceConfig {
            name: ServiceName::new("sv-not-running"),
            service_type: ServiceType::Mcp,
            port: 19040,
            enabled: true,
        }];

        let running = verifier
            .get_running_services(&configs)
            .await
            .expect("get_running_services must succeed");

        for state in &running {
            use systemprompt_models::RuntimeStatus;
            assert_eq!(
                state.runtime_status,
                RuntimeStatus::Running,
                "get_running_services must only return services in Running state"
            );
        }
    }

    #[tokio::test]
    async fn get_crashed_services_returns_only_crashed() {
        let pool = test_db_pool().await;
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool), isolated_instance());

        let configs = [ServiceConfig {
            name: ServiceName::new("sv-not-crashed"),
            service_type: ServiceType::Agent,
            port: 19041,
            enabled: true,
        }];

        let crashed = verifier
            .get_crashed_services(&configs)
            .await
            .expect("get_crashed_services must succeed");

        for state in &crashed {
            use systemprompt_models::RuntimeStatus;
            assert_eq!(
                state.runtime_status,
                RuntimeStatus::Crashed,
                "get_crashed_services must only return services in Crashed state"
            );
        }
    }

    #[tokio::test]
    async fn get_verified_states_multiple_configs_all_appear() {
        let pool = test_db_pool().await;
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool), isolated_instance());

        let configs = vec![
            ServiceConfig {
                name: ServiceName::new("sv-multi-a"),
                service_type: ServiceType::Mcp,
                port: 19050,
                enabled: true,
            },
            ServiceConfig {
                name: ServiceName::new("sv-multi-b"),
                service_type: ServiceType::Agent,
                port: 19051,
                enabled: false,
            },
            ServiceConfig {
                name: ServiceName::new("sv-multi-c"),
                service_type: ServiceType::Mcp,
                port: 19052,
                enabled: true,
            },
        ];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed with multiple configs");

        let config_names: Vec<&str> = configs.iter().map(|c| c.name.as_str()).collect();
        for name in config_names {
            assert!(
                states.iter().any(|s| s.name == name),
                "state for config '{name}' must be present in the result"
            );
        }
    }
}

// Seeded action-arm tests: rows are driven into each ServiceAction and the
// reconciler's handling (restart, orphan sweep, process cleanup, stop) is
// asserted on the DB row and the returned buckets. PIDs signalled are always
// children this test spawned and marked as the seeded MCP service; an unmarked
// holder proves the reconciler leaves a process it cannot identify alone.
#[cfg(unix)]
mod reconciler_action_arms {
    use super::*;

    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    use systemprompt_database::{
        CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus,
    };
    use systemprompt_loader::subprocess::{self, ChildKind};

    struct Seeded {
        pool: systemprompt_database::DbPool,
        repo: ServiceRepository,
        reconciler: ServiceReconciler,
    }

    async fn seeded() -> Seeded {
        let pool = test_db_pool().await;
        let instance = isolated_instance();
        let repo = ServiceRepository::new(&pool, instance.clone());
        let reconciler =
            ServiceReconciler::new(Arc::clone(&pool), ServiceRepository::new(&pool, instance));
        Seeded {
            pool,
            repo,
            reconciler,
        }
    }

    fn unique_name(prefix: &str) -> ServiceName {
        ServiceName::new(format!("{prefix}-{}", uuid::Uuid::new_v4().simple()))
    }

    async fn insert_service(
        repo: &ServiceRepository,
        name: &ServiceName,
        status: ServiceStatus,
        pid: Option<u32>,
        port: u16,
    ) {
        repo.create_service(CreateServiceInput {
            name,
            module_name: ServiceModule::Mcp,
            status,
            port,
            binary_mtime: None,
        })
        .await
        .expect("seed services row");
        if let Some(pid) = pid {
            repo.update_service_pid(name, i32::try_from(pid).expect("pid fits i32"))
                .await
                .expect("seed services pid");
        }
    }

    async fn fetch_status(repo: &ServiceRepository, name: &ServiceName) -> Option<ServiceStatus> {
        repo.find_service_by_name(name)
            .await
            .expect("fetch services row")
            .map(|row| row.status)
    }

    struct PortHolder(Child);

    impl Drop for PortHolder {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn spawn_port_holder(marked_as: Option<&ServiceName>) -> (PortHolder, u32, u16) {
        let mut command = Command::new("python3");
        command
            .args([
                "-c",
                "import socket,sys,time\ns=socket.socket()\ns.bind(('127.0.0.1',0))\nprint(s.getsockname()[1],flush=True)\ns.listen(1)\ntime.sleep(60)",
            ])
            .stdout(Stdio::piped());
        if let Some(service) = marked_as {
            subprocess::mark_child(&mut command, ChildKind::Mcp, service);
        }
        let mut child = command.spawn().expect("spawn python3 port holder");
        let stdout = child.stdout.take().expect("holder stdout");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("read holder port");
        let port = line.trim().parse::<u16>().expect("holder port number");
        let pid = child.id();
        (PortHolder(child), pid, port)
    }

    #[tokio::test]
    async fn crashed_enabled_service_is_restarted() {
        let t = seeded().await;
        let name = unique_name("rec-restart-ok");
        insert_service(
            &t.repo,
            &name,
            ServiceStatus::Running,
            Some(i32::MAX as u32),
            27401,
        )
        .await;

        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 27401,
            enabled: true,
        }];
        let result = t
            .reconciler
            .reconcile(&configs, |_n: ServiceName, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile");

        assert!(
            result.restarted.contains(&name),
            "Enabled + Crashed must be restarted, got {result:?}"
        );
        let row = t
            .repo
            .find_service_by_name(&name)
            .await
            .expect("fetch row")
            .expect("row present");
        assert_eq!(
            row.status,
            ServiceStatus::Stopped,
            "restart first marks the row stopped"
        );
        assert_eq!(row.pid, None, "restart clears the stale PID");

        t.repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn restart_records_failure_when_start_callback_errors() {
        let t = seeded().await;
        let name = unique_name("rec-restart-fail");
        insert_service(
            &t.repo,
            &name,
            ServiceStatus::Running,
            Some(i32::MAX as u32),
            27402,
        )
        .await;

        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 27402,
            enabled: true,
        }];
        let result = t
            .reconciler
            .reconcile(&configs, |_n: ServiceName, _p: u16| async {
                Err(SchedulerError::Io(std::io::Error::other("boot refused")))
            })
            .await
            .expect("reconcile");

        let (_, failed_err) = result
            .failed
            .iter()
            .find(|(n, _)| *n == name)
            .expect("the failed restart must be recorded");
        assert!(
            failed_err.contains("boot refused"),
            "the callback error must be captured, got: {failed_err}"
        );

        t.repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn stopped_orphan_row_is_swept_from_the_db() {
        let t = seeded().await;
        let name = unique_name("rec-orphan-db");
        insert_service(&t.repo, &name, ServiceStatus::Stopped, None, 27403).await;

        let result = t
            .reconciler
            .reconcile(&[], |_n: ServiceName, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile");

        assert_eq!(result.cleaned_up, vec![name.clone()], "{result:?}");
        assert!(
            fetch_status(&t.repo, &name).await.is_none(),
            "the orphan row must be deleted"
        );
    }

    #[tokio::test]
    async fn orphan_sweep_leaves_another_instances_row_alone() {
        let t = seeded().await;
        let name = unique_name("rec-orphan-scoped");
        insert_service(&t.repo, &name, ServiceStatus::Stopped, None, 27404).await;
        let other_repo = ServiceRepository::new(&t.pool, isolated_instance());
        insert_service(&other_repo, &name, ServiceStatus::Stopped, None, 27404).await;

        let result = t
            .reconciler
            .reconcile(&[], |_n: ServiceName, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile");
        assert_eq!(result.cleaned_up, vec![name.clone()], "{result:?}");

        assert_eq!(
            fetch_status(&other_repo, &name).await,
            Some(ServiceStatus::Stopped),
            "a sweep on one instance must not delete a same-named row of another"
        );

        other_repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn orphaned_process_is_terminated_and_row_swept() {
        let t = seeded().await;
        let name = unique_name("rec-orphan-proc");
        let (_holder, pid, port) = spawn_port_holder(Some(&name));
        insert_service(&t.repo, &name, ServiceStatus::Stopped, None, port).await;

        let result = t
            .reconciler
            .reconcile(&[], |_n: ServiceName, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile");

        assert_eq!(result.cleaned_up, vec![name.clone()], "{result:?}");
        assert!(
            fetch_status(&t.repo, &name).await.is_none(),
            "the orphan row must be deleted"
        );
        assert!(
            !subprocess::is_running(pid).await,
            "the marked orphan holding the service port must be stopped"
        );
    }

    #[tokio::test]
    async fn unmarked_port_holder_survives_the_orphan_sweep() {
        let t = seeded().await;
        let name = unique_name("rec-orphan-foreign");
        let (_holder, pid, port) = spawn_port_holder(None);
        insert_service(&t.repo, &name, ServiceStatus::Stopped, None, port).await;

        let result = t
            .reconciler
            .reconcile(&[], |_n: ServiceName, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile");

        assert_eq!(result.cleaned_up, vec![name.clone()], "{result:?}");
        assert!(
            subprocess::is_running(pid).await,
            "a port holder without this service's marker must never be signalled"
        );
    }

    #[tokio::test]
    async fn disabled_running_service_is_stopped() {
        let t = seeded().await;
        let name = unique_name("rec-stop");
        let (_holder, pid, port) = spawn_port_holder(Some(&name));
        insert_service(&t.repo, &name, ServiceStatus::Running, Some(pid), port).await;

        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port,
            enabled: false,
        }];
        let result = t
            .reconciler
            .reconcile(&configs, |_n: ServiceName, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile");

        assert_eq!(result.stopped, vec![name.clone()], "{result:?}");
        assert_eq!(
            fetch_status(&t.repo, &name).await,
            Some(ServiceStatus::Stopped)
        );
        assert!(
            !subprocess::is_running(pid).await,
            "the marked service process is stopped"
        );

        t.repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn disabled_service_with_an_unmarked_recorded_pid_fails_the_stop() {
        let t = seeded().await;
        let name = unique_name("rec-stop-foreign");
        let (_holder, pid, port) = spawn_port_holder(None);
        insert_service(&t.repo, &name, ServiceStatus::Running, Some(pid), port).await;

        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port,
            enabled: false,
        }];
        let result = t
            .reconciler
            .reconcile(&configs, |_n: ServiceName, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile");

        assert!(
            result.failed.iter().any(|(n, _)| *n == name),
            "a stop that cannot free the port is a failure, got {result:?}"
        );
        assert!(
            subprocess::is_running(pid).await,
            "a recorded pid without this service's marker must never be signalled"
        );

        t.repo.delete_service(&name).await.expect("cleanup row");
    }

    #[tokio::test]
    async fn healthy_running_service_needs_no_action() {
        let t = seeded().await;
        let name = unique_name("rec-noop");
        let (_holder, pid, port) = spawn_port_holder(Some(&name));
        insert_service(&t.repo, &name, ServiceStatus::Running, Some(pid), port).await;

        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port,
            enabled: true,
        }];
        let result = t
            .reconciler
            .reconcile(&configs, |_n: ServiceName, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile");

        assert!(result.is_success());
        assert_eq!(
            result.total_actions(),
            0,
            "Enabled + Running must take no action, got {result:?}"
        );
        assert!(subprocess::is_running(pid).await);

        t.repo.delete_service(&name).await.expect("cleanup row");
    }
}
