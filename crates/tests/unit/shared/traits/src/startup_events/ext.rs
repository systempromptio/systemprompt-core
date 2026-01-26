//! Tests for startup_events extension traits.

use futures::stream::StreamExt;
use std::time::Duration;
use systemprompt_traits::{
    startup_channel, ModuleInfo, OptionalStartupEventExt, Phase, ServiceInfo, ServiceState,
    ServiceType, StartupEvent, StartupEventExt,
};

mod startup_event_ext_tests {
    use super::*;

    #[tokio::test]
    async fn phase_started_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.phase_started(Phase::PreFlight);

        let event = rx.next().await.unwrap();
        assert!(matches!(
            event,
            StartupEvent::PhaseStarted { phase: Phase::PreFlight }
        ));
    }

    #[tokio::test]
    async fn phase_completed_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.phase_completed(Phase::Database);

        let event = rx.next().await.unwrap();
        assert!(matches!(
            event,
            StartupEvent::PhaseCompleted { phase: Phase::Database }
        ));
    }

    #[tokio::test]
    async fn phase_failed_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.phase_failed(Phase::McpServers, "Connection refused");

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::PhaseFailed { phase, error } => {
                assert_eq!(phase, Phase::McpServers);
                assert_eq!(error, "Connection refused");
            }
            _ => panic!("Expected PhaseFailed event"),
        }
    }

    #[tokio::test]
    async fn port_available_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.port_available(8080);

        let event = rx.next().await.unwrap();
        assert!(matches!(event, StartupEvent::PortAvailable { port: 8080 }));
    }

    #[tokio::test]
    async fn port_conflict_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.port_conflict(3000, 12345);

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::PortConflict { port, pid } => {
                assert_eq!(port, 3000);
                assert_eq!(pid, 12345);
            }
            _ => panic!("Expected PortConflict event"),
        }
    }

    #[tokio::test]
    async fn modules_loaded_sends_event() {
        let (tx, mut rx) = startup_channel();

        let modules = vec![
            ModuleInfo {
                name: "auth".to_string(),
                category: "security".to_string(),
            },
            ModuleInfo {
                name: "content".to_string(),
                category: "domain".to_string(),
            },
        ];

        tx.modules_loaded(2, modules);

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::ModulesLoaded { count, modules } => {
                assert_eq!(count, 2);
                assert_eq!(modules.len(), 2);
            }
            _ => panic!("Expected ModulesLoaded event"),
        }
    }

    #[tokio::test]
    async fn mcp_starting_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.mcp_starting("test-mcp", 5000);

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::McpServerStarting { name, port } => {
                assert_eq!(name, "test-mcp");
                assert_eq!(port, 5000);
            }
            _ => panic!("Expected McpServerStarting event"),
        }
    }

    #[tokio::test]
    async fn mcp_health_check_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.mcp_health_check("test-mcp", 2, 5);

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::McpServerHealthCheck {
                name,
                attempt,
                max_attempts,
            } => {
                assert_eq!(name, "test-mcp");
                assert_eq!(attempt, 2);
                assert_eq!(max_attempts, 5);
            }
            _ => panic!("Expected McpServerHealthCheck event"),
        }
    }

    #[tokio::test]
    async fn mcp_ready_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.mcp_ready("test-mcp", 5000, Duration::from_millis(500), 10);

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::McpServerReady {
                name,
                port,
                startup_time,
                tools,
            } => {
                assert_eq!(name, "test-mcp");
                assert_eq!(port, 5000);
                assert_eq!(startup_time, Duration::from_millis(500));
                assert_eq!(tools, 10);
            }
            _ => panic!("Expected McpServerReady event"),
        }
    }

    #[tokio::test]
    async fn mcp_failed_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.mcp_failed("test-mcp", "Failed to start");

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::McpServerFailed { name, error } => {
                assert_eq!(name, "test-mcp");
                assert_eq!(error, "Failed to start");
            }
            _ => panic!("Expected McpServerFailed event"),
        }
    }

    #[tokio::test]
    async fn agent_starting_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.agent_starting("test-agent", 6000);

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::AgentStarting { name, port } => {
                assert_eq!(name, "test-agent");
                assert_eq!(port, 6000);
            }
            _ => panic!("Expected AgentStarting event"),
        }
    }

    #[tokio::test]
    async fn agent_ready_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.agent_ready("test-agent", 6000, Duration::from_secs(1));

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::AgentReady {
                name,
                port,
                startup_time,
            } => {
                assert_eq!(name, "test-agent");
                assert_eq!(port, 6000);
                assert_eq!(startup_time, Duration::from_secs(1));
            }
            _ => panic!("Expected AgentReady event"),
        }
    }

    #[tokio::test]
    async fn agent_failed_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.agent_failed("test-agent", "Initialization error");

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::AgentFailed { name, error } => {
                assert_eq!(name, "test-agent");
                assert_eq!(error, "Initialization error");
            }
            _ => panic!("Expected AgentFailed event"),
        }
    }

    #[tokio::test]
    async fn server_listening_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.server_listening("0.0.0.0:8080", 54321);

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::ServerListening { address, pid } => {
                assert_eq!(address, "0.0.0.0:8080");
                assert_eq!(pid, 54321);
            }
            _ => panic!("Expected ServerListening event"),
        }
    }

    #[tokio::test]
    async fn warning_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.warning("Deprecated configuration");

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::Warning { message, context } => {
                assert_eq!(message, "Deprecated configuration");
                assert!(context.is_none());
            }
            _ => panic!("Expected Warning event"),
        }
    }

    #[tokio::test]
    async fn info_sends_event() {
        let (tx, mut rx) = startup_channel();

        tx.info("Loading configuration");

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::Info { message } => {
                assert_eq!(message, "Loading configuration");
            }
            _ => panic!("Expected Info event"),
        }
    }

    #[tokio::test]
    async fn startup_complete_sends_event() {
        let (tx, mut rx) = startup_channel();

        let services = vec![ServiceInfo {
            name: "api".to_string(),
            service_type: ServiceType::Api,
            port: Some(8080),
            state: ServiceState::Running,
            startup_time: Some(Duration::from_secs(2)),
        }];

        tx.startup_complete(Duration::from_secs(5), "http://localhost:8080", services);

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::StartupComplete {
                duration,
                api_url,
                services,
            } => {
                assert_eq!(duration, Duration::from_secs(5));
                assert_eq!(api_url, "http://localhost:8080");
                assert_eq!(services.len(), 1);
            }
            _ => panic!("Expected StartupComplete event"),
        }
    }
}

mod optional_startup_event_ext_tests {
    use super::*;

    #[tokio::test]
    async fn phase_started_with_some_sender() {
        let (tx, mut rx) = startup_channel();
        let optional: Option<&_> = Some(&tx);

        optional.phase_started(Phase::Database);

        let event = rx.next().await.unwrap();
        assert!(matches!(
            event,
            StartupEvent::PhaseStarted { phase: Phase::Database }
        ));
    }

    #[tokio::test]
    async fn phase_started_with_none_does_nothing() {
        let optional: Option<&systemprompt_traits::StartupEventSender> = None;

        // Should not panic
        optional.phase_started(Phase::Database);
    }

    #[tokio::test]
    async fn mcp_starting_with_some_sender() {
        let (tx, mut rx) = startup_channel();
        let optional: Option<&_> = Some(&tx);

        optional.mcp_starting("test", 5000);

        let event = rx.next().await.unwrap();
        match event {
            StartupEvent::McpServerStarting { name, port } => {
                assert_eq!(name, "test");
                assert_eq!(port, 5000);
            }
            _ => panic!("Expected McpServerStarting event"),
        }
    }

    #[tokio::test]
    async fn mcp_starting_with_none_does_nothing() {
        let optional: Option<&systemprompt_traits::StartupEventSender> = None;

        // Should not panic
        optional.mcp_starting("test", 5000);
    }
}

mod startup_channel_tests {
    use super::*;

    #[test]
    fn startup_channel_creates_sender_receiver_pair() {
        let (tx, _rx) = startup_channel();
        // Should be able to send without receiver being polled (unbounded channel)
        tx.phase_started(Phase::PreFlight);
        tx.phase_started(Phase::Database);
        tx.phase_started(Phase::McpServers);
    }

    #[tokio::test]
    async fn multiple_events_received_in_order() {
        let (tx, mut rx) = startup_channel();

        tx.phase_started(Phase::PreFlight);
        tx.phase_completed(Phase::PreFlight);
        tx.phase_started(Phase::Database);

        let e1 = rx.next().await.unwrap();
        let e2 = rx.next().await.unwrap();
        let e3 = rx.next().await.unwrap();

        assert!(matches!(
            e1,
            StartupEvent::PhaseStarted { phase: Phase::PreFlight }
        ));
        assert!(matches!(
            e2,
            StartupEvent::PhaseCompleted { phase: Phase::PreFlight }
        ));
        assert!(matches!(
            e3,
            StartupEvent::PhaseStarted { phase: Phase::Database }
        ));
    }
}
