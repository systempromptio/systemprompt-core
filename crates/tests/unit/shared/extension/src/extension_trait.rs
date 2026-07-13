use std::sync::Arc;

use systemprompt_extension::error::LoaderError;
use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRole, SchemaDefinition};

struct MinimalExt;

impl Extension for MinimalExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "minimal",
            name: "Minimal",
            version: "0.1.0",
        }
    }
}

struct SchemaExt;

impl Extension for SchemaExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "schema-bearing",
            name: "Schema Bearing",
            version: "1.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            "things",
            "CREATE TABLE things (id TEXT)",
        )]
    }
}

struct RequiredExt;

impl Extension for RequiredExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "required-ext",
            name: "Required",
            version: "1.0.0",
        }
    }

    fn is_required(&self) -> bool {
        true
    }
}

struct PriorityExt;

impl Extension for PriorityExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "priority-ext",
            name: "Priority",
            version: "1.0.0",
        }
    }

    fn priority(&self) -> u32 {
        42
    }
}

struct DepExt;

impl Extension for DepExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "dep-ext",
            name: "With Deps",
            version: "1.0.0",
        }
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["other-ext"]
    }
}

struct RolesExt;

impl Extension for RolesExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "roles-ext",
            name: "Roles",
            version: "1.0.0",
        }
    }

    fn roles(&self) -> Vec<ExtensionRole> {
        vec![ExtensionRole::new("viewer", "Viewer", "Read-only")]
    }
}

struct StorageExt;

impl Extension for StorageExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "storage-ext",
            name: "Storage",
            version: "1.0.0",
        }
    }

    fn required_storage_paths(&self) -> Vec<&'static str> {
        vec!["/var/data/uploads", "/var/data/cache"]
    }
}

#[test]
fn extension_default_id_delegates_to_metadata() {
    let ext = MinimalExt;
    assert_eq!(ext.id(), "minimal");
}

#[test]
fn extension_default_name_delegates_to_metadata() {
    let ext = MinimalExt;
    assert_eq!(ext.name(), "Minimal");
}

#[test]
fn extension_default_version_delegates_to_metadata() {
    let ext = MinimalExt;
    assert_eq!(ext.version(), "0.1.0");
}

#[test]
fn extension_default_schemas_is_empty() {
    let ext = MinimalExt;
    assert!(ext.schemas().is_empty());
}

#[test]
fn extension_default_has_schemas_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_schemas());
}

#[test]
fn extension_has_schemas_true_when_schemas_nonempty() {
    let ext = SchemaExt;
    assert!(ext.has_schemas());
    assert_eq!(ext.schemas().len(), 1);
}

#[test]
fn extension_default_jobs_is_empty() {
    let ext = MinimalExt;
    assert!(ext.jobs().is_empty());
}

#[test]
fn extension_default_has_jobs_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_jobs());
}

#[test]
fn extension_default_config_prefix_is_none() {
    let ext = MinimalExt;
    assert!(ext.config_prefix().is_none());
}

#[test]
fn extension_default_has_config_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_config());
}

#[test]
fn extension_default_config_schema_is_none() {
    let ext = MinimalExt;
    assert!(ext.config_schema().is_none());
}

#[test]
fn extension_default_validate_config_returns_ok() {
    let ext = MinimalExt;
    let config = serde_json::json!({});
    assert!(ext.validate_config(&config).is_ok());
}

#[test]
fn extension_default_llm_providers_is_empty() {
    let ext = MinimalExt;
    assert!(ext.llm_providers().is_empty());
}

#[test]
fn extension_default_has_llm_providers_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_llm_providers());
}

#[test]
fn extension_default_tool_providers_is_empty() {
    let ext = MinimalExt;
    assert!(ext.tool_providers().is_empty());
}

#[test]
fn extension_default_has_tool_providers_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_tool_providers());
}

#[test]
fn extension_default_template_providers_is_empty() {
    let ext = MinimalExt;
    assert!(ext.template_providers().is_empty());
}

#[test]
fn extension_default_has_template_providers_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_template_providers());
}

#[test]
fn extension_default_component_renderers_is_empty() {
    let ext = MinimalExt;
    assert!(ext.component_renderers().is_empty());
}

#[test]
fn extension_default_has_component_renderers_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_component_renderers());
}

#[test]
fn extension_default_template_data_extenders_is_empty() {
    let ext = MinimalExt;
    assert!(ext.template_data_extenders().is_empty());
}

#[test]
fn extension_default_page_data_providers_is_empty() {
    let ext = MinimalExt;
    assert!(ext.page_data_providers().is_empty());
}

#[test]
fn extension_default_has_page_data_providers_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_page_data_providers());
}

#[test]
fn extension_default_page_prerenderers_is_empty() {
    let ext = MinimalExt;
    assert!(ext.page_prerenderers().is_empty());
}

#[test]
fn extension_default_frontmatter_processors_is_empty() {
    let ext = MinimalExt;
    assert!(ext.frontmatter_processors().is_empty());
}

#[test]
fn extension_default_content_data_providers_is_empty() {
    let ext = MinimalExt;
    assert!(ext.content_data_providers().is_empty());
}

#[test]
fn extension_default_rss_feed_providers_is_empty() {
    let ext = MinimalExt;
    assert!(ext.rss_feed_providers().is_empty());
}

#[test]
fn extension_default_has_rss_feed_providers_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_rss_feed_providers());
}

#[test]
fn extension_default_sitemap_providers_is_empty() {
    let ext = MinimalExt;
    assert!(ext.sitemap_providers().is_empty());
}

#[test]
fn extension_default_has_sitemap_providers_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_sitemap_providers());
}

#[test]
fn extension_default_router_config_is_none() {
    let ext = MinimalExt;
    assert!(ext.router_config().is_none());
}

#[test]
fn extension_default_has_template_data_extenders_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_template_data_extenders());
}

#[test]
fn extension_default_has_page_prerenderers_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_page_prerenderers());
}

#[test]
fn extension_default_has_frontmatter_processors_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_frontmatter_processors());
}

#[test]
fn extension_default_has_content_data_providers_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_content_data_providers());
}

#[test]
fn extension_default_site_auth_is_none() {
    let ext = MinimalExt;
    assert!(ext.site_auth().is_none());
}

#[test]
fn extension_default_has_site_auth_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_site_auth());
}

#[test]
fn extension_default_required_storage_paths_is_empty() {
    let ext = MinimalExt;
    assert!(ext.required_storage_paths().is_empty());
}

#[test]
fn extension_default_has_storage_paths_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_storage_paths());
}

#[test]
fn extension_has_storage_paths_true_when_nonempty() {
    let ext = StorageExt;
    assert!(ext.has_storage_paths());
    assert_eq!(ext.required_storage_paths().len(), 2);
}

#[test]
fn extension_default_dependencies_is_empty() {
    let ext = MinimalExt;
    assert!(ext.dependencies().is_empty());
}

#[test]
fn extension_default_is_required_false() {
    let ext = MinimalExt;
    assert!(!ext.is_required());
}

#[test]
fn extension_is_required_overrideable() {
    let ext = RequiredExt;
    assert!(ext.is_required());
}

#[test]
fn extension_default_priority_is_100() {
    let ext = MinimalExt;
    assert_eq!(ext.priority(), 100);
}

#[test]
fn extension_priority_overrideable() {
    let ext = PriorityExt;
    assert_eq!(ext.priority(), 42);
}

#[test]
fn extension_dependencies_overrideable() {
    let ext = DepExt;
    assert_eq!(ext.dependencies(), vec!["other-ext"]);
}

#[test]
fn extension_default_migrations_is_empty() {
    let ext = MinimalExt;
    assert!(ext.migrations().is_empty());
}

#[test]
fn extension_default_has_migrations_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_migrations());
}

#[test]
fn extension_default_seeds_is_empty() {
    let ext = MinimalExt;
    assert!(ext.seeds().is_empty());
}

#[test]
fn extension_default_cross_extension_tables_is_empty() {
    let ext = MinimalExt;
    assert!(ext.cross_extension_tables().is_empty());
}

#[test]
fn extension_default_roles_is_empty() {
    let ext = MinimalExt;
    assert!(ext.roles().is_empty());
}

#[test]
fn extension_default_has_roles_is_false() {
    let ext = MinimalExt;
    assert!(!ext.has_roles());
}

#[test]
fn extension_has_roles_true_when_nonempty() {
    let ext = RolesExt;
    assert!(ext.has_roles());
    assert_eq!(ext.roles().len(), 1);
}

#[test]
fn extension_default_declares_assets_is_false() {
    let ext = MinimalExt;
    assert!(!ext.declares_assets());
}

#[test]
fn extension_is_arc_dyn_compatible() {
    let ext: Arc<dyn Extension> = Arc::new(MinimalExt);
    assert_eq!(ext.id(), "minimal");
    assert_eq!(ext.priority(), 100);
    assert!(!ext.is_required());
}

#[test]
fn loader_error_migration_not_reversible_display() {
    let err = LoaderError::MigrationNotReversible {
        extension: "my-ext".to_string(),
        version: 5,
    };
    let msg = err.to_string();
    assert!(msg.contains("my-ext"));
    assert!(msg.contains("5"));
    assert!(msg.contains("not reversible"));
}

#[test]
fn loader_error_dependency_cycle_display() {
    let err = LoaderError::DependencyCycle {
        chain: "ext-a -> ext-b -> ext-a".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("ext-a -> ext-b -> ext-a"));
    assert!(msg.contains("cycle"));
}

#[test]
fn loader_error_cross_extension_alter_undeclared_display() {
    let err = LoaderError::CrossExtensionAlterUndeclared {
        extension: "plugin-x".to_string(),
        table: "users".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("plugin-x"));
    assert!(msg.contains("users"));
    assert!(msg.contains("cross-extension"));
}

#[test]
fn loader_error_duplicate_table_owner_display() {
    let err = LoaderError::DuplicateTableOwner {
        table: "orders".to_string(),
        extension_a: "billing".to_string(),
        extension_b: "shop".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("orders"));
    assert!(msg.contains("billing"));
    assert!(msg.contains("shop"));
}

#[test]
fn loader_error_cross_extension_table_not_owned_display() {
    let err = LoaderError::CrossExtensionTableNotOwned {
        extension: "addon".to_string(),
        table: "nonexistent_table".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("addon"));
    assert!(msg.contains("nonexistent_table"));
}

#[test]
fn loader_error_invalid_seed_statement_display() {
    let err = LoaderError::InvalidSeedStatement {
        extension: "seeder".to_string(),
        seed: "initial_data".to_string(),
        statement: "CREATE TABLE".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("seeder"));
    assert!(msg.contains("initial_data"));
    assert!(msg.contains("CREATE TABLE"));
}

#[test]
fn loader_error_seed_insert_not_idempotent_display() {
    let err = LoaderError::SeedInsertNotIdempotent {
        extension: "ext".to_string(),
        seed: "seed_roles".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("ext"));
    assert!(msg.contains("seed_roles"));
    assert!(msg.contains("idempotent"));
}

#[test]
fn loader_error_seed_failed_display() {
    let err = LoaderError::SeedFailed {
        extension: "data-ext".to_string(),
        seed: "load_defaults".to_string(),
        message: "connection timeout".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("data-ext"));
    assert!(msg.contains("load_defaults"));
    assert!(msg.contains("connection timeout"));
}

#[test]
fn schema_definition_with_schema_sets_schema_name() {
    let schema =
        SchemaDefinition::new("events", "CREATE TABLE events (id TEXT)").with_schema("audit");
    assert_eq!(schema.schema_name(), "audit");
}

#[test]
fn schema_definition_schema_name_defaults_to_public() {
    let schema = SchemaDefinition::new("events", "CREATE TABLE events (id TEXT)");
    assert_eq!(schema.schema_name(), "public");
}

#[test]
fn schema_definition_with_schema_roundtrip_serde() {
    let schema = SchemaDefinition::new("logs", "CREATE TABLE logs (id TEXT)").with_schema("audit");
    let json = serde_json::to_string(&schema).expect("serialize");
    let back: SchemaDefinition = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.schema_name(), "audit");
}
