//! Tests for startup_events event variants.

use std::time::Duration;
use systemprompt_traits::{ModuleInfo, Phase, ServiceInfo, ServiceState, ServiceType, StartupEvent};

mod startup_event_tests {
    use super::*;

    #[test]
    fn phase_started_variant() {
        let event = StartupEvent::PhaseStarted {
            phase: Phase::PreFlight,
        };
        assert!(matches!(
            event,
            StartupEvent::PhaseStarted { phase: Phase::PreFlight }
        ));
    }

    #[test]
    fn phase_completed_variant() {
        let event = StartupEvent::PhaseCompleted {
            phase: Phase::Database,
        };
        assert!(matches!(
            event,
            StartupEvent::PhaseCompleted { phase: Phase::Database }
        ));
    }

    #[test]
    fn phase_failed_variant() {
        let event = StartupEvent::PhaseFailed {
            phase: Phase::McpServers,
            error: "Connection timeout".to_string(),
        };
        match event {
            StartupEvent::PhaseFailed { phase, error } => {
                assert_eq!(phase, Phase::McpServers);
                assert_eq!(error, "Connection timeout");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn port_check_started_variant() {
        let event = StartupEvent::PortCheckStarted { port: 8080 };
        assert!(matches!(event, StartupEvent::PortCheckStarted { port: 8080 }));
    }

    #[test]
    fn port_available_variant() {
        let event = StartupEvent::PortAvailable { port: 3000 };
        assert!(matches!(event, StartupEvent::PortAvailable { port: 3000 }));
    }

    #[test]
    fn port_conflict_variant() {
        let event = StartupEvent::PortConflict { port: 5000, pid: 1234 };
        match event {
            StartupEvent::PortConflict { port, pid } => {
                assert_eq!(port, 5000);
                assert_eq!(pid, 1234);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn mcp_server_starting_variant() {
        let event = StartupEvent::McpServerStarting {
            name: "mcp1".to_string(),
            port: 5001,
        };
        match event {
            StartupEvent::McpServerStarting { name, port } => {
                assert_eq!(name, "mcp1");
                assert_eq!(port, 5001);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn mcp_server_ready_variant() {
        let event = StartupEvent::McpServerReady {
            name: "mcp1".to_string(),
            port: 5001,
            startup_time: Duration::from_millis(500),
            tools: 10,
        };
        match event {
            StartupEvent::McpServerReady {
                name,
                port,
                startup_time,
                tools,
            } => {
                assert_eq!(name, "mcp1");
                assert_eq!(port, 5001);
                assert_eq!(startup_time, Duration::from_millis(500));
                assert_eq!(tools, 10);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn mcp_server_failed_variant() {
        let event = StartupEvent::McpServerFailed {
            name: "mcp1".to_string(),
            error: "Failed to bind".to_string(),
        };
        match event {
            StartupEvent::McpServerFailed { name, error } => {
                assert_eq!(name, "mcp1");
                assert_eq!(error, "Failed to bind");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn agent_starting_variant() {
        let event = StartupEvent::AgentStarting {
            name: "agent1".to_string(),
            port: 6001,
        };
        match event {
            StartupEvent::AgentStarting { name, port } => {
                assert_eq!(name, "agent1");
                assert_eq!(port, 6001);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn agent_ready_variant() {
        let event = StartupEvent::AgentReady {
            name: "agent1".to_string(),
            port: 6001,
            startup_time: Duration::from_secs(1),
        };
        match event {
            StartupEvent::AgentReady {
                name,
                port,
                startup_time,
            } => {
                assert_eq!(name, "agent1");
                assert_eq!(port, 6001);
                assert_eq!(startup_time, Duration::from_secs(1));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn server_listening_variant() {
        let event = StartupEvent::ServerListening {
            address: "0.0.0.0:8080".to_string(),
            pid: 12345,
        };
        match event {
            StartupEvent::ServerListening { address, pid } => {
                assert_eq!(address, "0.0.0.0:8080");
                assert_eq!(pid, 12345);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn warning_variant() {
        let event = StartupEvent::Warning {
            message: "Config deprecated".to_string(),
            context: Some("app.yaml:15".to_string()),
        };
        match event {
            StartupEvent::Warning { message, context } => {
                assert_eq!(message, "Config deprecated");
                assert_eq!(context, Some("app.yaml:15".to_string()));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn info_variant() {
        let event = StartupEvent::Info {
            message: "Loading modules".to_string(),
        };
        match event {
            StartupEvent::Info { message } => {
                assert_eq!(message, "Loading modules");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn startup_complete_variant() {
        let services = vec![ServiceInfo {
            name: "api".to_string(),
            service_type: ServiceType::Api,
            port: Some(8080),
            state: ServiceState::Running,
            startup_time: Some(Duration::from_secs(2)),
        }];

        let event = StartupEvent::StartupComplete {
            duration: Duration::from_secs(10),
            api_url: "http://localhost:8080".to_string(),
            services,
        };

        match event {
            StartupEvent::StartupComplete {
                duration,
                api_url,
                services,
            } => {
                assert_eq!(duration, Duration::from_secs(10));
                assert_eq!(api_url, "http://localhost:8080");
                assert_eq!(services.len(), 1);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn startup_failed_variant() {
        let event = StartupEvent::StartupFailed {
            error: "Database connection failed".to_string(),
            duration: Duration::from_secs(5),
        };
        match event {
            StartupEvent::StartupFailed { error, duration } => {
                assert_eq!(error, "Database connection failed");
                assert_eq!(duration, Duration::from_secs(5));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn startup_event_is_clone() {
        let event = StartupEvent::PhaseStarted {
            phase: Phase::PreFlight,
        };
        let cloned = event.clone();
        assert!(matches!(
            cloned,
            StartupEvent::PhaseStarted { phase: Phase::PreFlight }
        ));
    }

    #[test]
    fn startup_event_is_debug() {
        let event = StartupEvent::Info {
            message: "Test".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Info"));
        assert!(debug_str.contains("Test"));
    }

    #[test]
    fn modules_loaded_variant() {
        let modules = vec![
            ModuleInfo {
                name: "mod1".to_string(),
                category: "cat1".to_string(),
            },
        ];
        let event = StartupEvent::ModulesLoaded { count: 1, modules };
        match event {
            StartupEvent::ModulesLoaded { count, modules } => {
                assert_eq!(count, 1);
                assert_eq!(modules.len(), 1);
            }
            _ => panic!("Wrong variant"),
        }
    }
}
