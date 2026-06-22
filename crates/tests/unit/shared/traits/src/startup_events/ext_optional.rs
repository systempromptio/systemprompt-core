//! Tests for the `Option<&StartupEventSender>` mirror of `StartupEventExt`.
//!
//! Each method is exercised twice: once through `Some(&tx)` (asserting the
//! concrete event is emitted) and once through `None` (asserting nothing is
//! emitted and the stream is closed once the sender is dropped).

use futures::stream::StreamExt;
use std::time::Duration;
use systemprompt_traits::{
    ModuleInfo, OptionalStartupEventExt, Phase, ServiceInfo, ServiceState, ServiceType,
    StartupEvent, StartupEventSender, startup_channel,
};

fn none_sender() -> Option<&'static StartupEventSender> {
    None
}

#[tokio::test]
async fn phase_started_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).phase_started(Phase::PreFlight);
    assert!(matches!(
        rx.next().await.unwrap(),
        StartupEvent::PhaseStarted {
            phase: Phase::PreFlight
        }
    ));
}

#[tokio::test]
async fn phase_completed_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).phase_completed(Phase::Database);
    assert!(matches!(
        rx.next().await.unwrap(),
        StartupEvent::PhaseCompleted {
            phase: Phase::Database
        }
    ));
}

#[tokio::test]
async fn phase_failed_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).phase_failed(Phase::McpServers, "boom");
    match rx.next().await.unwrap() {
        StartupEvent::PhaseFailed { phase, error } => {
            assert_eq!(phase, Phase::McpServers);
            assert_eq!(error, "boom");
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn port_available_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).port_available(8080);
    assert!(matches!(
        rx.next().await.unwrap(),
        StartupEvent::PortAvailable { port: 8080 }
    ));
}

#[tokio::test]
async fn port_conflict_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).port_conflict(3000, 42);
    match rx.next().await.unwrap() {
        StartupEvent::PortConflict { port, pid } => {
            assert_eq!(port, 3000);
            assert_eq!(pid, 42);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn modules_loaded_some_emits() {
    let (tx, mut rx) = startup_channel();
    let modules = vec![ModuleInfo {
        name: "auth".to_string(),
        category: "security".to_string(),
    }];
    Some(&tx).modules_loaded(1, modules);
    match rx.next().await.unwrap() {
        StartupEvent::ModulesLoaded { count, modules } => {
            assert_eq!(count, 1);
            assert_eq!(modules.len(), 1);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn mcp_starting_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).mcp_starting("svc", 5000);
    match rx.next().await.unwrap() {
        StartupEvent::McpServerStarting { name, port } => {
            assert_eq!(name, "svc");
            assert_eq!(port, 5000);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn mcp_health_check_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).mcp_health_check("svc", 1, 3);
    match rx.next().await.unwrap() {
        StartupEvent::McpServerHealthCheck {
            name,
            attempt,
            max_attempts,
        } => {
            assert_eq!(name, "svc");
            assert_eq!(attempt, 1);
            assert_eq!(max_attempts, 3);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn mcp_ready_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).mcp_ready("svc", 5000, Duration::from_millis(250), 7);
    match rx.next().await.unwrap() {
        StartupEvent::McpServerReady {
            name,
            port,
            startup_time,
            tools,
        } => {
            assert_eq!(name, "svc");
            assert_eq!(port, 5000);
            assert_eq!(startup_time, Duration::from_millis(250));
            assert_eq!(tools, 7);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn mcp_failed_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).mcp_failed("svc", "no port");
    match rx.next().await.unwrap() {
        StartupEvent::McpServerFailed { name, error } => {
            assert_eq!(name, "svc");
            assert_eq!(error, "no port");
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn mcp_service_cleanup_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).mcp_service_cleanup("svc", "stale");
    match rx.next().await.unwrap() {
        StartupEvent::McpServiceCleanup { name, reason } => {
            assert_eq!(name, "svc");
            assert_eq!(reason, "stale");
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn mcp_reconciliation_complete_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).mcp_reconciliation_complete(2, 3);
    match rx.next().await.unwrap() {
        StartupEvent::McpReconciliationComplete { running, required } => {
            assert_eq!(running, 2);
            assert_eq!(required, 3);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn agent_starting_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).agent_starting("agent", 6000);
    match rx.next().await.unwrap() {
        StartupEvent::AgentStarting { name, port } => {
            assert_eq!(name, "agent");
            assert_eq!(port, 6000);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn agent_ready_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).agent_ready("agent", 6000, Duration::from_secs(2));
    match rx.next().await.unwrap() {
        StartupEvent::AgentReady {
            name,
            port,
            startup_time,
        } => {
            assert_eq!(name, "agent");
            assert_eq!(port, 6000);
            assert_eq!(startup_time, Duration::from_secs(2));
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn agent_failed_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).agent_failed("agent", "crashed");
    match rx.next().await.unwrap() {
        StartupEvent::AgentFailed { name, error } => {
            assert_eq!(name, "agent");
            assert_eq!(error, "crashed");
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn agent_cleanup_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).agent_cleanup("agent", "stopped");
    match rx.next().await.unwrap() {
        StartupEvent::AgentCleanup { name, reason } => {
            assert_eq!(name, "agent");
            assert_eq!(reason, "stopped");
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn server_listening_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).server_listening("0.0.0.0:8080", 99);
    match rx.next().await.unwrap() {
        StartupEvent::ServerListening { address, pid } => {
            assert_eq!(address, "0.0.0.0:8080");
            assert_eq!(pid, 99);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn scheduler_initializing_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).scheduler_initializing();
    assert!(matches!(
        rx.next().await.unwrap(),
        StartupEvent::SchedulerInitializing
    ));
}

#[tokio::test]
async fn scheduler_ready_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).scheduler_ready(4);
    match rx.next().await.unwrap() {
        StartupEvent::SchedulerReady { job_count } => assert_eq!(job_count, 4),
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn bootstrap_job_started_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).bootstrap_job_started("migrate");
    match rx.next().await.unwrap() {
        StartupEvent::BootstrapJobStarted { name } => assert_eq!(name, "migrate"),
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn bootstrap_job_completed_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).bootstrap_job_completed("migrate", true, Some("done".to_string()));
    match rx.next().await.unwrap() {
        StartupEvent::BootstrapJobCompleted {
            name,
            success,
            message,
        } => {
            assert_eq!(name, "migrate");
            assert!(success);
            assert_eq!(message.as_deref(), Some("done"));
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn warning_some_emits_without_context() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).warning("deprecated");
    match rx.next().await.unwrap() {
        StartupEvent::Warning { message, context } => {
            assert_eq!(message, "deprecated");
            assert!(context.is_none());
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn warning_with_context_some_emits_context() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).warning_with_context("deprecated", "use v2");
    match rx.next().await.unwrap() {
        StartupEvent::Warning { message, context } => {
            assert_eq!(message, "deprecated");
            assert_eq!(context.as_deref(), Some("use v2"));
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn info_some_emits() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).info("loading");
    match rx.next().await.unwrap() {
        StartupEvent::Info { message } => assert_eq!(message, "loading"),
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn error_some_emits_fatal_flag() {
    let (tx, mut rx) = startup_channel();
    Some(&tx).error("fatal boom", true);
    match rx.next().await.unwrap() {
        StartupEvent::Error { message, fatal } => {
            assert_eq!(message, "fatal boom");
            assert!(fatal);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn startup_complete_some_emits() {
    let (tx, mut rx) = startup_channel();
    let services = vec![ServiceInfo {
        name: "api".to_string(),
        service_type: ServiceType::Api,
        port: Some(8080),
        state: ServiceState::Running,
        startup_time: Some(Duration::from_secs(1)),
    }];
    Some(&tx).startup_complete(Duration::from_secs(3), "http://localhost:8080", services);
    match rx.next().await.unwrap() {
        StartupEvent::StartupComplete {
            duration,
            api_url,
            services,
        } => {
            assert_eq!(duration, Duration::from_secs(3));
            assert_eq!(api_url, "http://localhost:8080");
            assert_eq!(services.len(), 1);
        },
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn none_sender_emits_nothing_across_all_methods() {
    let (tx, mut rx) = startup_channel();
    let none = none_sender();

    none.phase_started(Phase::PreFlight);
    none.phase_completed(Phase::Database);
    none.phase_failed(Phase::McpServers, "e");
    none.port_available(1);
    none.port_conflict(1, 2);
    none.modules_loaded(0, Vec::new());
    none.mcp_starting("s", 1);
    none.mcp_health_check("s", 1, 1);
    none.mcp_ready("s", 1, Duration::ZERO, 0);
    none.mcp_failed("s", "e");
    none.mcp_service_cleanup("s", "r");
    none.mcp_reconciliation_complete(0, 0);
    none.agent_starting("a", 1);
    none.agent_ready("a", 1, Duration::ZERO);
    none.agent_failed("a", "e");
    none.agent_cleanup("a", "r");
    none.server_listening("addr", 1);
    none.scheduler_initializing();
    none.scheduler_ready(0);
    none.bootstrap_job_started("j");
    none.bootstrap_job_completed("j", false, None);
    none.warning("w");
    none.warning_with_context("w", "c");
    none.info("i");
    none.error("e", false);
    none.startup_complete(Duration::ZERO, "url", Vec::new());

    // Dropping the live sender closes the stream; the `None` calls above must
    // not have queued anything, so the very first poll yields end-of-stream.
    drop(tx);
    assert!(rx.next().await.is_none());
}
