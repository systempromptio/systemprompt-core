//! Tests for startup_events types.

use std::time::Duration;
use systemprompt_traits::{ModuleInfo, Phase, ServiceInfo, ServiceState, ServiceType};

mod phase_tests {
    use super::*;

    #[test]
    fn name_returns_human_readable_names() {
        assert_eq!(Phase::PreFlight.name(), "Pre-flight");
        assert_eq!(Phase::Database.name(), "Database");
        assert_eq!(Phase::McpServers.name(), "MCP Servers");
        assert_eq!(Phase::ApiServer.name(), "API Server");
        assert_eq!(Phase::Agents.name(), "Agents");
        assert_eq!(Phase::Scheduler.name(), "Scheduler");
    }

    #[test]
    fn is_blocking_returns_true_for_blocking_phases() {
        assert!(Phase::PreFlight.is_blocking());
        assert!(Phase::Database.is_blocking());
        assert!(Phase::McpServers.is_blocking());
        assert!(Phase::ApiServer.is_blocking());
    }

    #[test]
    fn is_blocking_returns_false_for_non_blocking_phases() {
        assert!(!Phase::Agents.is_blocking());
        assert!(!Phase::Scheduler.is_blocking());
    }

    #[test]
    fn phase_is_copy() {
        let phase = Phase::Database;
        let copied = phase;
        assert_eq!(phase, copied);
    }

    #[test]
    fn phase_is_clone() {
        let phase = Phase::McpServers;
        let cloned = phase.clone();
        assert_eq!(phase, cloned);
    }

    #[test]
    fn phase_equality() {
        assert_eq!(Phase::PreFlight, Phase::PreFlight);
        assert_ne!(Phase::PreFlight, Phase::Database);
    }

    #[test]
    fn phase_is_debug() {
        let phase = Phase::ApiServer;
        let debug_str = format!("{:?}", phase);
        assert!(debug_str.contains("ApiServer"));
    }

    #[test]
    fn phase_is_hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Phase::PreFlight);
        set.insert(Phase::Database);
        set.insert(Phase::PreFlight); // duplicate

        assert_eq!(set.len(), 2);
    }
}

mod service_type_tests {
    use super::*;

    #[test]
    fn label_returns_short_labels() {
        assert_eq!(ServiceType::Mcp.label(), "MCP");
        assert_eq!(ServiceType::Agent.label(), "Agent");
        assert_eq!(ServiceType::Api.label(), "API");
        assert_eq!(ServiceType::Scheduler.label(), "Sched");
    }

    #[test]
    fn service_type_is_copy() {
        let st = ServiceType::Mcp;
        let copied = st;
        assert_eq!(st, copied);
    }

    #[test]
    fn service_type_equality() {
        assert_eq!(ServiceType::Agent, ServiceType::Agent);
        assert_ne!(ServiceType::Agent, ServiceType::Api);
    }

    #[test]
    fn service_type_is_debug() {
        let st = ServiceType::Scheduler;
        let debug_str = format!("{:?}", st);
        assert!(debug_str.contains("Scheduler"));
    }
}

mod service_state_tests {
    use super::*;

    #[test]
    fn service_state_variants() {
        let _ = ServiceState::Starting;
        let _ = ServiceState::Running;
        let _ = ServiceState::Stopped;
        let _ = ServiceState::Failed;
    }

    #[test]
    fn service_state_is_copy() {
        let state = ServiceState::Running;
        let copied = state;
        assert_eq!(state, copied);
    }

    #[test]
    fn service_state_equality() {
        assert_eq!(ServiceState::Running, ServiceState::Running);
        assert_ne!(ServiceState::Running, ServiceState::Stopped);
    }

    #[test]
    fn service_state_is_debug() {
        let state = ServiceState::Failed;
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("Failed"));
    }
}

mod service_info_tests {
    use super::*;

    #[test]
    fn can_create_service_info() {
        let info = ServiceInfo {
            name: "test-service".to_string(),
            service_type: ServiceType::Mcp,
            port: Some(5000),
            state: ServiceState::Running,
            startup_time: Some(Duration::from_millis(500)),
        };

        assert_eq!(info.name, "test-service");
        assert_eq!(info.service_type, ServiceType::Mcp);
        assert_eq!(info.port, Some(5000));
        assert_eq!(info.state, ServiceState::Running);
        assert!(info.startup_time.is_some());
    }

    #[test]
    fn service_info_without_optional_fields() {
        let info = ServiceInfo {
            name: "scheduler".to_string(),
            service_type: ServiceType::Scheduler,
            port: None,
            state: ServiceState::Starting,
            startup_time: None,
        };

        assert!(info.port.is_none());
        assert!(info.startup_time.is_none());
    }

    #[test]
    fn service_info_is_clone() {
        let info = ServiceInfo {
            name: "cloneable".to_string(),
            service_type: ServiceType::Api,
            port: Some(8080),
            state: ServiceState::Running,
            startup_time: Some(Duration::from_secs(1)),
        };
        let cloned = info.clone();

        assert_eq!(info.name, cloned.name);
        assert_eq!(info.service_type, cloned.service_type);
        assert_eq!(info.port, cloned.port);
        assert_eq!(info.state, cloned.state);
        assert_eq!(info.startup_time, cloned.startup_time);
    }

    #[test]
    fn service_info_is_debug() {
        let info = ServiceInfo {
            name: "debug-test".to_string(),
            service_type: ServiceType::Agent,
            port: Some(3000),
            state: ServiceState::Stopped,
            startup_time: None,
        };
        let debug_str = format!("{:?}", info);

        assert!(debug_str.contains("debug-test"));
        assert!(debug_str.contains("Agent"));
    }
}

mod module_info_tests {
    use super::*;

    #[test]
    fn can_create_module_info() {
        let info = ModuleInfo {
            name: "auth_module".to_string(),
            category: "security".to_string(),
        };

        assert_eq!(info.name, "auth_module");
        assert_eq!(info.category, "security");
    }

    #[test]
    fn module_info_is_clone() {
        let info = ModuleInfo {
            name: "original".to_string(),
            category: "test".to_string(),
        };
        let cloned = info.clone();

        assert_eq!(info.name, cloned.name);
        assert_eq!(info.category, cloned.category);
    }

    #[test]
    fn module_info_is_debug() {
        let info = ModuleInfo {
            name: "test_module".to_string(),
            category: "testing".to_string(),
        };
        let debug_str = format!("{:?}", info);

        assert!(debug_str.contains("test_module"));
        assert!(debug_str.contains("testing"));
    }
}
