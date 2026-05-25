use std::sync::Arc;

use systemprompt_extension::capabilities::{
    CapabilityContext, FullContext, HasConfig, HasDatabase, HasEventBus,
};
use systemprompt_traits::{ConfigProvider, DatabaseHandle, UserEvent, UserEventPublisher};

#[derive(Debug)]
struct TestConfig {
    db_url: String,
    system_path: String,
}

impl ConfigProvider for TestConfig {
    fn get(&self, _key: &str) -> Option<String> {
        None
    }

    fn database_url(&self) -> &str {
        &self.db_url
    }

    fn system_path(&self) -> &str {
        &self.system_path
    }

    fn api_port(&self) -> u16 {
        8080
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
struct TestDatabase {
    connected: bool,
}

impl DatabaseHandle for TestDatabase {
    fn is_connected(&self) -> bool {
        self.connected
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
struct TestEventBus;

impl UserEventPublisher for TestEventBus {
    fn publish_user_event(&self, _event: UserEvent) {}
}

fn make_context() -> CapabilityContext<TestConfig, TestDatabase, TestEventBus> {
    CapabilityContext::new(
        Arc::new(TestConfig {
            db_url: "postgres://localhost/test".to_string(),
            system_path: "/tmp/test".to_string(),
        }),
        Arc::new(TestDatabase { connected: true }),
        Arc::new(TestEventBus),
    )
}

#[test]
fn capability_context_has_config() {
    let ctx = make_context();
    let config = HasConfig::config(&ctx);
    assert_eq!(config.database_url(), "postgres://localhost/test");
}

#[test]
fn capability_context_has_database() {
    let ctx = make_context();
    let db = HasDatabase::database(&ctx);
    assert!(db.is_connected());
}

#[test]
fn capability_context_has_event_bus() {
    let ctx = make_context();
    let _bus = HasEventBus::event_bus(&ctx);
    assert!(true);
}

#[test]
fn capability_context_config_system_path() {
    let ctx = make_context();
    assert_eq!(HasConfig::config(&ctx).system_path(), "/tmp/test");
}

#[test]
fn capability_context_config_api_port() {
    let ctx = make_context();
    assert_eq!(HasConfig::config(&ctx).api_port(), 8080);
}

#[test]
fn capability_context_implements_full_context() {
    fn assert_full_context<T: FullContext>(_ctx: &T) {}
    let ctx = make_context();
    assert_full_context(&ctx);
    assert!(true);
}

#[test]
fn capability_context_debug_format() {
    let ctx = make_context();
    let debug = format!("{ctx:?}");
    assert!(debug.contains("CapabilityContext"));
}
