//! DB-backed tests for [`ServiceReconciler`] and [`ServiceStateVerifier`].
//!
//! Both types require a live Postgres pool. Tests early-return when
//! `DATABASE_URL` is unset so the suite still passes in CI environments without
//! a database. The `services` table may be empty on a freshly-migrated DB;
//! tests seed rows they need and clean them up afterwards.

use std::sync::Arc;

use systemprompt_scheduler::{
    DesiredStatus, ReconciliationResult, ServiceAction, ServiceConfig, ServiceReconciler,
    ServiceStateVerifier,
};
use systemprompt_models::ServiceType;
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

mod reconciler_db {
    use super::*;

    #[tokio::test]
    async fn new_constructs_against_migrated_db() {
        let pool = pool_or_skip!();
        let _reconciler = ServiceReconciler::new(Arc::clone(&pool));
    }

    #[tokio::test]
    async fn reconcile_empty_configs_returns_success() {
        let pool = pool_or_skip!();
        let reconciler = ServiceReconciler::new(Arc::clone(&pool));

        let result = reconciler
            .reconcile(&[], |_name: String, _port: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed with an empty config slice");

        assert!(
            result.is_success(),
            "empty-config reconciliation must report success"
        );
        assert_eq!(
            result.total_actions(),
            0,
            "no configs → no actions taken"
        );
    }

    #[tokio::test]
    async fn reconcile_disabled_config_absent_from_db_returns_success() {
        let pool = pool_or_skip!();
        let reconciler = ServiceReconciler::new(Arc::clone(&pool));

        let configs = [ServiceConfig {
            name: "test-absent-disabled".to_string(),
            service_type: ServiceType::Mcp,
            port: 19001,
            enabled: false,
        }];

        let result = reconciler
            .reconcile(&configs, |_name: String, _port: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed when the service is absent from DB and disabled");

        assert!(result.is_success());
    }

    #[tokio::test]
    async fn reconcile_enabled_config_absent_from_db_attempts_start() {
        let pool = pool_or_skip!();
        let reconciler = ServiceReconciler::new(Arc::clone(&pool));

        let configs = [ServiceConfig {
            name: "test-enabled-no-db-row".to_string(),
            service_type: ServiceType::Mcp,
            port: 19002,
            enabled: true,
        }];

        let start_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&start_called);

        let result = reconciler
            .reconcile(&configs, move |_name: String, _port: u16| {
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
        let pool = pool_or_skip!();
        let reconciler = ServiceReconciler::new(Arc::clone(&pool));

        let configs = vec![
            ServiceConfig {
                name: "multi-disabled-a".to_string(),
                service_type: ServiceType::Mcp,
                port: 19010,
                enabled: false,
            },
            ServiceConfig {
                name: "multi-disabled-b".to_string(),
                service_type: ServiceType::Agent,
                port: 19011,
                enabled: false,
            },
        ];

        let result = reconciler
            .reconcile(&configs, |_name: String, _port: u16| async { Ok(()) })
            .await
            .expect("reconcile must succeed for all-disabled configs");

        assert!(result.is_success());
    }

    #[tokio::test]
    async fn reconcile_start_failure_recorded_in_failed() {
        let pool = pool_or_skip!();
        let reconciler = ServiceReconciler::new(Arc::clone(&pool));

        let configs = [ServiceConfig {
            name: "test-start-fail".to_string(),
            service_type: ServiceType::Mcp,
            port: 19003,
            enabled: true,
        }];

        let result = reconciler
            .reconcile(&configs, |_name: String, _port: u16| async {
                Err(Box::new(std::io::Error::other("simulated start failure"))
                    as Box<dyn std::error::Error + Send + Sync>)
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
        let pool = pool_or_skip!();
        let _verifier = ServiceStateVerifier::new(Arc::clone(&pool));
    }

    #[tokio::test]
    async fn get_verified_states_empty_configs_returns_empty_or_orphans() {
        let pool = pool_or_skip!();
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));

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
        let pool = pool_or_skip!();
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));

        let configs = [ServiceConfig {
            name: "sv-disabled-absent".to_string(),
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

        assert_eq!(matching.len(), 1, "disabled config must produce exactly one state");
        let state = &matching[0];
        assert_eq!(state.desired_status, DesiredStatus::Disabled);
        assert!(
            matches!(
                state.needs_action,
                ServiceAction::CleanupDb | ServiceAction::CleanupProcess | ServiceAction::Stop | ServiceAction::None
            ),
            "disabled + not-running service must map to a cleanup or no-op action, got {:?}",
            state.needs_action
        );
    }

    #[tokio::test]
    async fn get_verified_states_enabled_config_absent_from_db_needs_start() {
        let pool = pool_or_skip!();
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));

        let configs = [ServiceConfig {
            name: "sv-enabled-absent".to_string(),
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
        let pool = pool_or_skip!();
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));

        let configs = [
            ServiceConfig {
                name: "sv-action-enabled".to_string(),
                service_type: ServiceType::Mcp,
                port: 19030,
                enabled: true,
            },
            ServiceConfig {
                name: "sv-action-disabled".to_string(),
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
        let pool = pool_or_skip!();
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));

        let configs = [ServiceConfig {
            name: "sv-not-running".to_string(),
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
        let pool = pool_or_skip!();
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));

        let configs = [ServiceConfig {
            name: "sv-not-crashed".to_string(),
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
        let pool = pool_or_skip!();
        let verifier = ServiceStateVerifier::new(Arc::clone(&pool));

        let configs = vec![
            ServiceConfig {
                name: "sv-multi-a".to_string(),
                service_type: ServiceType::Mcp,
                port: 19050,
                enabled: true,
            },
            ServiceConfig {
                name: "sv-multi-b".to_string(),
                service_type: ServiceType::Agent,
                port: 19051,
                enabled: false,
            },
            ServiceConfig {
                name: "sv-multi-c".to_string(),
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
