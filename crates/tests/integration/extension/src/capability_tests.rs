//! Tests for capability traits.

use std::any::Any;
use std::sync::Arc;

use systemprompt_extension::capabilities::{
    CapabilityContext, FullContext, HasConfig, HasDatabase, HasEventBus,
};
use systemprompt_traits::{ConfigProvider, DatabaseHandle, UserEvent, UserEventPublisher};

// =============================================================================
// Mock Implementations for Testing
// =============================================================================

#[derive(Debug, Clone)]
struct MockConfig {
    app_name: String,
}

impl ConfigProvider for MockConfig {
    fn get(&self, key: &str) -> Option<String> {
        match key {
            "app_name" => Some(self.app_name.clone()),
            _ => None,
        }
    }

    fn database_url(&self) -> &str {
        "postgres://localhost/test"
    }

    fn system_path(&self) -> &str {
        "/tmp/test"
    }

    fn api_port(&self) -> u16 {
        8080
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
struct MockDatabase;

impl DatabaseHandle for MockDatabase {
    fn is_connected(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
struct MockEventBus;

impl UserEventPublisher for MockEventBus {
    fn publish_user_event(&self, _event: UserEvent) {
        // No-op for testing
    }
}

// =============================================================================
// CapabilityContext Tests
// =============================================================================

#[test]
fn test_capability_context_creation() {
    let config = Arc::new(MockConfig {
        app_name: "test-app".to_string(),
    });
    let database = Arc::new(MockDatabase);
    let event_bus = Arc::new(MockEventBus);

    let ctx = CapabilityContext::new(config, database, event_bus);

    // Just verify it compiles and can be created
    let _ = ctx;
}

#[test]
fn test_has_config_trait() {
    let config = Arc::new(MockConfig {
        app_name: "test-app".to_string(),
    });
    let database = Arc::new(MockDatabase);
    let event_bus = Arc::new(MockEventBus);

    let ctx = CapabilityContext::new(config, database, event_bus);

    let cfg: &MockConfig = ctx.config();
    assert_eq!(cfg.app_name, "test-app");
}

#[test]
fn test_has_database_trait() {
    let config = Arc::new(MockConfig {
        app_name: "test-app".to_string(),
    });
    let database = Arc::new(MockDatabase);
    let event_bus = Arc::new(MockEventBus);

    let ctx = CapabilityContext::new(config, database, event_bus);

    let db: &MockDatabase = ctx.database();
    assert!(db.is_connected());
}

#[test]
fn test_has_event_bus_trait() {
    let config = Arc::new(MockConfig {
        app_name: "test-app".to_string(),
    });
    let database = Arc::new(MockDatabase);
    let event_bus = Arc::new(MockEventBus);

    let ctx = CapabilityContext::new(config, database, event_bus);

    let _bus: &MockEventBus = ctx.event_bus();
    // Just verify we can access it
}

#[test]
fn test_full_context_trait() {
    // Verify that CapabilityContext implements FullContext
    fn assert_full_context<T: FullContext>(_: &T) {}

    let config = Arc::new(MockConfig {
        app_name: "test-app".to_string(),
    });
    let database = Arc::new(MockDatabase);
    let event_bus = Arc::new(MockEventBus);

    let ctx = CapabilityContext::new(config, database, event_bus);
    assert_full_context(&ctx);
}

// =============================================================================
// Trait Bound Tests
// =============================================================================

fn requires_has_config<T: HasConfig>(ctx: &T) -> String {
    let _ = ctx;
    "has_config".to_string()
}

fn requires_has_database<T: HasDatabase>(ctx: &T) -> String {
    let _ = ctx;
    "has_database".to_string()
}

fn requires_full_context<T: FullContext>(ctx: &T) -> String {
    let _ = ctx;
    "full_context".to_string()
}

#[test]
fn test_trait_bounds() {
    let config = Arc::new(MockConfig {
        app_name: "test-app".to_string(),
    });
    let database = Arc::new(MockDatabase);
    let event_bus = Arc::new(MockEventBus);

    let ctx = CapabilityContext::new(config, database, event_bus);

    assert_eq!(requires_has_config(&ctx), "has_config");
    assert_eq!(requires_has_database(&ctx), "has_database");
    assert_eq!(requires_full_context(&ctx), "full_context");
}
