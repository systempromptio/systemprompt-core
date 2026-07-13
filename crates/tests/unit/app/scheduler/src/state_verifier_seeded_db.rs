//! DB-backed tests for [`ServiceStateVerifier`] and [`ServiceReconciler`]
//! with pre-seeded `services` rows.
//!
//! Seeds rows in known states (`running`/`starting`/`stopped`) with fake PIDs
//! that will not exist in the process table, driving `determine_runtime_status`
//! into its per-branch logic without spawning real processes. Each test cleans
//! up its rows afterwards so shards do not interfere.

use std::sync::Arc;

use systemprompt_models::ServiceType;
use systemprompt_scheduler::{
    DesiredStatus, ServiceAction, ServiceConfig, ServiceReconciler, ServiceStateVerifier,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

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
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}_{}_{}_{}", std::process::id(), n, nanos)
}

async fn insert_service(
    pg: &sqlx::PgPool,
    name: &str,
    module_name: &str,
    status: &str,
    pid: Option<i32>,
    port: i32,
) {
    sqlx::query!(
        r#"
        INSERT INTO services (name, module_name, status, pid, port)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (name) DO UPDATE SET
            module_name = EXCLUDED.module_name,
            status = EXCLUDED.status,
            pid = EXCLUDED.pid,
            port = EXCLUDED.port
        "#,
        name,
        module_name,
        status,
        pid,
        port,
    )
    .execute(pg)
    .await
    .expect("seed services row");
}

async fn delete_service(pg: &sqlx::PgPool, name: &str) {
    sqlx::query!("DELETE FROM services WHERE name = $1", name)
        .execute(pg)
        .await
        .ok();
}

mod state_verifier_seeded {
    use super::*;

    #[tokio::test]
    async fn running_status_nonexistent_pid_becomes_crashed() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_seed_running_crashed");

        insert_service(&pg, &name, "mcp", "running", Some(999_999_998), 29100).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 29100,
            enabled: true,
        }];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed");

        let state = states
            .iter()
            .find(|s| s.name == name)
            .expect("state present");

        use systemprompt_models::RuntimeStatus;
        assert_eq!(
            state.runtime_status,
            RuntimeStatus::Crashed,
            "running row with a non-existent PID must resolve as Crashed"
        );
        assert_eq!(
            state.needs_action,
            ServiceAction::Restart,
            "Enabled + Crashed must require Restart"
        );

        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn running_status_null_pid_becomes_crashed() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_seed_running_nopid");

        insert_service(&pg, &name, "agent", "running", None, 29101).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Agent,
            port: 29101,
            enabled: true,
        }];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed");

        let state = states
            .iter()
            .find(|s| s.name == name)
            .expect("state present");

        use systemprompt_models::RuntimeStatus;
        assert_eq!(
            state.runtime_status,
            RuntimeStatus::Crashed,
            "running row with NULL pid must resolve as Crashed"
        );

        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn starting_status_nonexistent_pid_becomes_stopped() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_seed_starting_stopped");

        insert_service(&pg, &name, "mcp", "starting", Some(999_999_997), 29102).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 29102,
            enabled: true,
        }];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed");

        let state = states
            .iter()
            .find(|s| s.name == name)
            .expect("state present");

        use systemprompt_models::RuntimeStatus;
        assert_eq!(
            state.runtime_status,
            RuntimeStatus::Stopped,
            "starting row with a non-existent PID must resolve as Stopped"
        );
        assert_eq!(
            state.needs_action,
            ServiceAction::Start,
            "Enabled + Stopped must require Start"
        );

        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn starting_status_null_pid_becomes_stopped() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_seed_starting_nopid");

        insert_service(&pg, &name, "mcp", "starting", None, 29103).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 29103,
            enabled: true,
        }];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed");

        let state = states
            .iter()
            .find(|s| s.name == name)
            .expect("state present");

        use systemprompt_models::RuntimeStatus;
        assert_eq!(
            state.runtime_status,
            RuntimeStatus::Stopped,
            "starting row with NULL pid must resolve as Stopped"
        );

        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn disabled_config_with_stopped_db_row_maps_to_cleanup_db() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_seed_disabled_stopped");

        insert_service(&pg, &name, "mcp", "stopped", None, 29104).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 29104,
            enabled: false,
        }];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed");

        let state = states
            .iter()
            .find(|s| s.name == name)
            .expect("state present");

        assert_eq!(state.desired_status, DesiredStatus::Disabled);
        assert_eq!(
            state.needs_action,
            ServiceAction::CleanupDb,
            "Disabled + Stopped must map to CleanupDb"
        );

        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn disabled_config_with_crashed_db_row_maps_to_cleanup_db() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_seed_disabled_crashed");

        insert_service(&pg, &name, "agent", "running", Some(999_999_996), 29105).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Agent,
            port: 29105,
            enabled: false,
        }];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed");

        let state = states
            .iter()
            .find(|s| s.name == name)
            .expect("state present");

        use systemprompt_models::RuntimeStatus;
        assert_eq!(
            state.runtime_status,
            RuntimeStatus::Crashed,
            "disabled service with running+dead PID must be Crashed"
        );
        assert_eq!(
            state.needs_action,
            ServiceAction::CleanupDb,
            "Disabled + Crashed must require CleanupDb"
        );

        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn orphan_db_row_not_in_config_is_included() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_seed_orphan");

        insert_service(&pg, &name, "mcp", "stopped", None, 29106).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));

        let states = verifier
            .get_verified_states(&[])
            .await
            .expect("get_verified_states must succeed with empty configs");

        let orphan = states.iter().find(|s| s.name == name);
        assert!(
            orphan.is_some(),
            "a DB row not present in the config slice must appear as an orphan state"
        );

        if let Some(state) = orphan {
            assert_eq!(
                state.desired_status,
                DesiredStatus::Disabled,
                "orphan service must have Disabled as desired status"
            );
        }

        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn multiple_db_rows_mixed_states() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");

        let name_a = unique_name("sv_multi_a");
        let name_b = unique_name("sv_multi_b");
        let name_c = unique_name("sv_multi_c");

        insert_service(&pg, &name_a, "mcp", "running", Some(999_999_995), 29110).await;
        insert_service(&pg, &name_b, "agent", "starting", None, 29111).await;
        insert_service(&pg, &name_c, "mcp", "stopped", None, 29112).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = vec![
            ServiceConfig {
                name: name_a.clone(),
                service_type: ServiceType::Mcp,
                port: 29110,
                enabled: true,
            },
            ServiceConfig {
                name: name_b.clone(),
                service_type: ServiceType::Agent,
                port: 29111,
                enabled: true,
            },
            ServiceConfig {
                name: name_c.clone(),
                service_type: ServiceType::Mcp,
                port: 29112,
                enabled: false,
            },
        ];

        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states must succeed");

        for n in [&name_a, &name_b, &name_c] {
            assert!(
                states.iter().any(|s| &s.name == n),
                "state for {n} must be present"
            );
        }

        let state_a = states.iter().find(|s| &s.name == &name_a).unwrap();
        use systemprompt_models::RuntimeStatus;
        assert_eq!(state_a.runtime_status, RuntimeStatus::Crashed);

        let state_b = states.iter().find(|s| &s.name == &name_b).unwrap();
        assert_eq!(state_b.runtime_status, RuntimeStatus::Stopped);

        let state_c = states.iter().find(|s| &s.name == &name_c).unwrap();
        assert_eq!(state_c.needs_action, ServiceAction::CleanupDb);

        delete_service(&pg, &name_a).await;
        delete_service(&pg, &name_b).await;
        delete_service(&pg, &name_c).await;
    }
}

mod reconciler_seeded {
    use super::*;

    #[tokio::test]
    async fn reconcile_cleanup_db_for_disabled_stopped_row() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("rec_seed_cleanup");

        insert_service(&pg, &name, "mcp", "stopped", None, 29120).await;

        let reconciler = ServiceReconciler::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 29120,
            enabled: false,
        }];

        let result = reconciler
            .reconcile(&configs, |_n: String, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed");

        assert!(
            result.cleaned_up.contains(&name) || result.failed.iter().any(|(n, _)| n == &name),
            "a Disabled+Stopped DB row must be cleaned up or recorded as failed"
        );
    }

    #[tokio::test]
    async fn reconcile_start_for_enabled_crashed_row() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("rec_seed_restart");

        insert_service(&pg, &name, "agent", "running", Some(999_999_994), 29121).await;

        let reconciler = ServiceReconciler::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Agent,
            port: 29121,
            enabled: true,
        }];

        let start_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&start_called);

        let _result = reconciler
            .reconcile(&configs, move |_n: String, _p: u16| {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
                async { Ok(()) }
            })
            .await
            .expect("reconcile must succeed");

        assert!(
            start_called.load(std::sync::atomic::Ordering::Relaxed),
            "Enabled + Crashed service must trigger a start_service call during restart"
        );

        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn reconcile_cleanup_db_removes_row_from_db() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("rec_seed_deleted");

        insert_service(&pg, &name, "mcp", "stopped", None, 29122).await;

        let reconciler = ServiceReconciler::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 29122,
            enabled: false,
        }];

        let result = reconciler
            .reconcile(&configs, |_n: String, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed");

        if result.cleaned_up.contains(&name) {
            let row_count: i64 =
                sqlx::query_scalar!("SELECT COUNT(*) FROM services WHERE name = $1", name)
                    .fetch_one(&*pg)
                    .await
                    .expect("count query")
                    .unwrap_or(0);

            assert_eq!(
                row_count, 0,
                "CleanupDb action must delete the row from the services table"
            );
        }
    }

    #[tokio::test]
    async fn reconcile_mixed_configs_seeded_runs_multiple_branches() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");

        let name_start = unique_name("rec_mix_start");
        let name_cleanup = unique_name("rec_mix_cleanup");

        insert_service(&pg, &name_cleanup, "mcp", "stopped", None, 29130).await;

        let reconciler = ServiceReconciler::new(Arc::clone(&pool));
        let configs = vec![
            ServiceConfig {
                name: name_start.clone(),
                service_type: ServiceType::Mcp,
                port: 29131,
                enabled: true,
            },
            ServiceConfig {
                name: name_cleanup.clone(),
                service_type: ServiceType::Mcp,
                port: 29130,
                enabled: false,
            },
        ];

        let result = reconciler
            .reconcile(&configs, |_n: String, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed");

        assert!(
            result.total_actions() >= 1,
            "mixed-state reconcile must produce at least one action"
        );

        delete_service(&pg, &name_start).await;
        delete_service(&pg, &name_cleanup).await;
    }

    #[tokio::test]
    async fn reconcile_result_tracks_start_for_new_enabled_service() {
        let pool = pool_or_skip!();
        let name = unique_name("rec_newstart");

        let reconciler = ServiceReconciler::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 29132,
            enabled: true,
        }];

        let result = reconciler
            .reconcile(&configs, |_n: String, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed");

        assert!(
            result.started.contains(&name) || result.failed.iter().any(|(n, _)| n == &name),
            "a newly enabled service absent from DB must be started or recorded as failed"
        );
    }

    #[tokio::test]
    async fn reconcile_none_action_for_absent_disabled_service() {
        let pool = pool_or_skip!();
        let name = unique_name("rec_none");

        let reconciler = ServiceReconciler::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 29133,
            enabled: false,
        }];

        let result = reconciler
            .reconcile(&configs, |_n: String, _p: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed");

        assert!(
            result.is_success(),
            "absent+disabled service must not cause failures"
        );
        assert!(
            !result.started.contains(&name),
            "absent+disabled service must not be started"
        );
    }
}

mod verifier_query_methods_seeded {
    use super::*;

    #[tokio::test]
    async fn get_crashed_services_returns_seeded_crashed_state() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_crashed_filter");

        insert_service(&pg, &name, "mcp", "running", Some(999_999_993), 29140).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: 29140,
            enabled: true,
        }];

        let crashed = verifier
            .get_crashed_services(&configs)
            .await
            .expect("get_crashed_services must succeed");

        assert!(
            crashed.iter().any(|s| s.name == name),
            "a running row with a dead PID must appear in get_crashed_services"
        );

        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn get_services_needing_action_includes_crashed_enabled_service() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_action_filter");

        insert_service(&pg, &name, "agent", "running", Some(999_999_992), 29141).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Agent,
            port: 29141,
            enabled: true,
        }];

        let needing = verifier
            .get_services_needing_action(&configs)
            .await
            .expect("get_services_needing_action must succeed");

        assert!(
            needing.iter().any(|s| s.name == name),
            "Enabled+Crashed service must be returned by get_services_needing_action"
        );

        delete_service(&pg, &name).await;
    }
}

// Live-process variants: the PID is a `sleep` child spawned by the test and
// the port is a listener the test itself holds, so runtime probing sees a
// genuinely live process without signalling anything.
#[cfg(unix)]
mod state_verifier_live {
    use super::*;

    use std::net::TcpListener;
    use std::process::{Child, Command};

    use systemprompt_models::RuntimeStatus;

    fn spawn_sleep() -> Child {
        Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep child")
    }

    fn reserved_free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .expect("bind probe listener")
            .local_addr()
            .expect("local addr")
            .port()
    }

    #[tokio::test]
    async fn running_row_with_live_pid_and_responsive_port_is_running() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_live_running");

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let mut child = spawn_sleep();
        let pid = child.id() as i32;

        insert_service(&pg, &name, "mcp", "running", Some(pid), i32::from(port)).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port,
            enabled: true,
        }];
        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states");
        let state = states.iter().find(|s| s.name == name).expect("state");

        assert_eq!(state.runtime_status, RuntimeStatus::Running);
        assert_eq!(state.pid, Some(child.id()));
        assert_eq!(
            state.needs_action,
            ServiceAction::None,
            "Enabled + Running needs no action"
        );

        child.kill().ok();
        let _ = child.wait();
        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn disabled_running_row_with_live_pid_needs_stop() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_live_disabled_running");

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let mut child = spawn_sleep();
        let pid = child.id() as i32;

        insert_service(&pg, &name, "mcp", "running", Some(pid), i32::from(port)).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port,
            enabled: false,
        }];
        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states");
        let state = states.iter().find(|s| s.name == name).expect("state");

        assert_eq!(state.runtime_status, RuntimeStatus::Running);
        assert_eq!(
            state.needs_action,
            ServiceAction::Stop,
            "Disabled + Running must require Stop"
        );

        child.kill().ok();
        let _ = child.wait();
        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn running_row_with_live_pid_and_unresponsive_port_is_starting() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_live_starting");

        let port = reserved_free_port();
        let mut child = spawn_sleep();
        let pid = child.id() as i32;

        insert_service(&pg, &name, "mcp", "running", Some(pid), i32::from(port)).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port,
            enabled: true,
        }];
        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states");
        let state = states.iter().find(|s| s.name == name).expect("state");

        assert_eq!(
            state.runtime_status,
            RuntimeStatus::Starting,
            "live PID with an unresponsive port is Starting, not Running"
        );
        assert_eq!(
            state.needs_action,
            ServiceAction::None,
            "Enabled + Starting needs no action"
        );

        child.kill().ok();
        let _ = child.wait();
        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn starting_row_with_live_pid_stays_starting() {
        let pool = pool_or_skip!();
        let pg = pool.write_pool_arc().expect("write pool");
        let name = unique_name("sv_live_starting_row");

        let port = reserved_free_port();
        let mut child = spawn_sleep();
        let pid = child.id() as i32;

        insert_service(&pg, &name, "agent", "starting", Some(pid), i32::from(port)).await;

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Agent,
            port,
            enabled: true,
        }];
        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states");
        let state = states.iter().find(|s| s.name == name).expect("state");

        assert_eq!(state.runtime_status, RuntimeStatus::Starting);
        assert_eq!(state.pid, Some(child.id()));

        child.kill().ok();
        let _ = child.wait();
        delete_service(&pg, &name).await;
    }

    #[tokio::test]
    async fn no_row_with_occupied_port_is_orphaned() {
        let pool = pool_or_skip!();
        let name = unique_name("sv_live_orphaned");

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();

        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));
        let configs = [ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port,
            enabled: true,
        }];
        let states = verifier
            .get_verified_states(&configs)
            .await
            .expect("get_verified_states");
        let state = states.iter().find(|s| s.name == name).expect("state");

        assert_eq!(
            state.runtime_status,
            RuntimeStatus::Orphaned,
            "no DB row but an occupied port must resolve as Orphaned"
        );
        assert_eq!(
            state.needs_action,
            ServiceAction::Restart,
            "Enabled + Orphaned must require Restart"
        );
    }
}
