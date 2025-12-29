//! Tests for ExtensionContext trait.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_extension::context::{DynExtensionContext, ExtensionContext};
use systemprompt_extension::{Extension, ExtensionMetadata};
use systemprompt_traits::{ConfigProvider, DatabaseHandle};

// =============================================================================
// Mock Implementations
// =============================================================================

#[derive(Debug, Clone)]
struct MockConfig;

impl ConfigProvider for MockConfig {
    fn get(&self, key: &str) -> Option<String> {
        match key {
            "test_key" => Some("test_value".to_string()),
            _ => None,
        }
    }

    fn database_url(&self) -> &str {
        "postgres://test:test@localhost/test"
    }

    fn system_path(&self) -> &str {
        "/tmp/test-system"
    }

    fn api_port(&self) -> u16 {
        3000
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
struct MockDatabase {
    connected: bool,
}

impl DatabaseHandle for MockDatabase {
    fn is_connected(&self) -> bool {
        self.connected
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct TestExtension {
    id: &'static str,
}

impl Extension for TestExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "Test Extension",
            version: "1.0.0",
        }
    }
}

struct MockExtensionContext {
    config: Arc<dyn ConfigProvider>,
    database: Arc<dyn DatabaseHandle>,
    extensions: HashMap<String, Arc<dyn Extension>>,
}

impl ExtensionContext for MockExtensionContext {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        Arc::clone(&self.config)
    }

    fn database(&self) -> Arc<dyn DatabaseHandle> {
        Arc::clone(&self.database)
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn Extension>> {
        self.extensions.get(id).cloned()
    }
}

// =============================================================================
// ExtensionContext Implementation Tests
// =============================================================================

#[test]
fn test_extension_context_config() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
    };

    let config = ctx.config();
    assert_eq!(config.get("test_key"), Some("test_value".to_string()));
    assert_eq!(config.database_url(), "postgres://test:test@localhost/test");
    assert_eq!(config.api_port(), 3000);
}

#[test]
fn test_extension_context_database() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
    };

    let db = ctx.database();
    assert!(db.is_connected());
}

#[test]
fn test_extension_context_database_disconnected() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: false }),
        extensions: HashMap::new(),
    };

    let db = ctx.database();
    assert!(!db.is_connected());
}

#[test]
fn test_extension_context_get_extension_exists() {
    let ext: Arc<dyn Extension> = Arc::new(TestExtension { id: "test-ext" });
    let mut extensions = HashMap::new();
    extensions.insert("test-ext".to_string(), ext);

    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions,
    };

    let found = ctx.get_extension("test-ext");
    assert!(found.is_some());
    assert_eq!(found.expect("extension exists").id(), "test-ext");
}

#[test]
fn test_extension_context_get_extension_not_found() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
    };

    assert!(ctx.get_extension("nonexistent").is_none());
}

#[test]
fn test_extension_context_has_extension_true() {
    let ext: Arc<dyn Extension> = Arc::new(TestExtension { id: "my-ext" });
    let mut extensions = HashMap::new();
    extensions.insert("my-ext".to_string(), ext);

    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions,
    };

    assert!(ctx.has_extension("my-ext"));
}

#[test]
fn test_extension_context_has_extension_false() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
    };

    assert!(!ctx.has_extension("missing-ext"));
}

#[test]
fn test_extension_context_multiple_extensions() {
    let ext1: Arc<dyn Extension> = Arc::new(TestExtension { id: "ext-1" });
    let ext2: Arc<dyn Extension> = Arc::new(TestExtension { id: "ext-2" });
    let ext3: Arc<dyn Extension> = Arc::new(TestExtension { id: "ext-3" });

    let mut extensions = HashMap::new();
    extensions.insert("ext-1".to_string(), ext1);
    extensions.insert("ext-2".to_string(), ext2);
    extensions.insert("ext-3".to_string(), ext3);

    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions,
    };

    assert!(ctx.has_extension("ext-1"));
    assert!(ctx.has_extension("ext-2"));
    assert!(ctx.has_extension("ext-3"));
    assert!(!ctx.has_extension("ext-4"));

    assert_eq!(ctx.get_extension("ext-1").expect("ext-1 exists").id(), "ext-1");
    assert_eq!(ctx.get_extension("ext-2").expect("ext-2 exists").id(), "ext-2");
}

// =============================================================================
// DynExtensionContext Type Alias Tests
// =============================================================================

#[test]
fn test_dyn_extension_context_type_alias() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
    };

    // Can be used as DynExtensionContext
    let dyn_ctx: DynExtensionContext = Arc::new(ctx);
    assert!(dyn_ctx.database().is_connected());
}

#[test]
fn test_dyn_extension_context_clone() {
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
    };

    let dyn_ctx: DynExtensionContext = Arc::new(ctx);
    let cloned = Arc::clone(&dyn_ctx);

    assert!(dyn_ctx.database().is_connected());
    assert!(cloned.database().is_connected());
}

#[test]
fn test_dyn_extension_context_thread_safe() {
    use std::thread;

    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
    };

    let dyn_ctx: DynExtensionContext = Arc::new(ctx);
    let ctx_clone = Arc::clone(&dyn_ctx);

    let handle = thread::spawn(move || {
        ctx_clone.database().is_connected()
    });

    assert!(dyn_ctx.database().is_connected());
    assert!(handle.join().expect("thread should complete"));
}

// =============================================================================
// Extension Trait Default Method Tests
// =============================================================================

#[test]
fn test_extension_default_schemas() {
    let ext = TestExtension { id: "test" };
    assert!(ext.schemas().is_empty());
}

#[test]
fn test_extension_default_migration_weight() {
    let ext = TestExtension { id: "test" };
    assert_eq!(ext.migration_weight(), 100);
}

#[test]
fn test_extension_default_router() {
    let ext = TestExtension { id: "test" };
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
    };
    assert!(ext.router(&ctx).is_none());
}

#[test]
fn test_extension_default_jobs() {
    let ext = TestExtension { id: "test" };
    assert!(ext.jobs().is_empty());
}

#[test]
fn test_extension_default_config_prefix() {
    let ext = TestExtension { id: "test" };
    assert!(ext.config_prefix().is_none());
}

#[test]
fn test_extension_default_config_schema() {
    let ext = TestExtension { id: "test" };
    assert!(ext.config_schema().is_none());
}

#[test]
fn test_extension_default_validate_config() {
    let ext = TestExtension { id: "test" };
    let config = serde_json::json!({});
    assert!(ext.validate_config(&config).is_ok());
}

#[test]
fn test_extension_default_llm_providers() {
    let ext = TestExtension { id: "test" };
    assert!(ext.llm_providers().is_empty());
}

#[test]
fn test_extension_default_tool_providers() {
    let ext = TestExtension { id: "test" };
    assert!(ext.tool_providers().is_empty());
}

#[test]
fn test_extension_default_dependencies() {
    let ext = TestExtension { id: "test" };
    assert!(ext.dependencies().is_empty());
}

#[test]
fn test_extension_default_priority() {
    let ext = TestExtension { id: "test" };
    assert_eq!(ext.priority(), 100);
}

#[test]
fn test_extension_derived_methods() {
    let ext = TestExtension { id: "my-ext" };

    // These methods derive from metadata()
    assert_eq!(ext.id(), "my-ext");
    assert_eq!(ext.name(), "Test Extension");
    assert_eq!(ext.version(), "1.0.0");
}

#[test]
fn test_extension_has_methods() {
    let ext = TestExtension { id: "test" };
    let ctx = MockExtensionContext {
        config: Arc::new(MockConfig),
        database: Arc::new(MockDatabase { connected: true }),
        extensions: HashMap::new(),
    };

    assert!(!ext.has_schemas());
    assert!(!ext.has_router(&ctx));
    assert!(!ext.has_jobs());
    assert!(!ext.has_config());
    assert!(!ext.has_llm_providers());
    assert!(!ext.has_tool_providers());
}
