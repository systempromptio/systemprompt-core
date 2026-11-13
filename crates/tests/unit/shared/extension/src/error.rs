use systemprompt_extension::error::{
    ExtensionConfigError, LoaderError, MigrationFault, SchemaFault,
};
use systemprompt_identifiers::ExtensionId;

#[test]
fn loader_error_missing_dependency_display() {
    let err = LoaderError::MissingDependency {
        extension: ExtensionId::new("my-ext"),
        dependency: ExtensionId::new("base-ext"),
    };
    let msg = err.to_string();
    assert!(msg.contains("my-ext"));
    assert!(msg.contains("base-ext"));
    assert!(msg.contains("requires dependency"));
}

#[test]
fn loader_error_duplicate_extension_display() {
    let err = LoaderError::DuplicateExtension(ExtensionId::new("dup-ext"));
    let msg = err.to_string();
    assert!(msg.contains("dup-ext"));
    assert!(msg.contains("already registered"));
}

#[test]
fn loader_error_initialization_failed_display() {
    let err = LoaderError::InitializationFailed {
        extension: ExtensionId::new("fail-ext"),
        message: "startup crashed".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("fail-ext"));
    assert!(msg.contains("startup crashed"));
}

#[test]
fn loader_error_schema_installation_failed_keeps_its_fault() {
    let err = LoaderError::SchemaInstallationStepFailed {
        extension: ExtensionId::new("schema-ext"),
        context: "Lint declarative schema".to_string(),
        source: Box::new(SchemaFault::ImperativeSql {
            offences: vec!["DO $$ BEGIN END $$".to_string()],
        }),
    };
    let msg = err.to_string();
    assert!(msg.contains("schema-ext"));
    assert!(msg.contains("imperative SQL"));
    let fault = std::error::Error::source(&err)
        .and_then(|source| source.downcast_ref::<SchemaFault>());
    assert!(matches!(fault, Some(SchemaFault::ImperativeSql { offences }) if offences.len() == 1));
}

#[test]
fn loader_error_migration_failed_keeps_its_fault() {
    let err = LoaderError::MigrationStepFailed {
        extension: ExtensionId::new("mig-ext"),
        context: "Mark migration applied".to_string(),
        source: Box::new(MigrationFault::AlreadyApplied {
            version: 3,
            name: "add_column".to_string(),
        }),
    };
    let msg = err.to_string();
    assert!(msg.contains("mig-ext"));
    assert!(msg.contains("migration 3 ('add_column') is already tracked as applied"));
    let fault = std::error::Error::source(&err)
        .and_then(|source| source.downcast_ref::<MigrationFault>());
    assert!(matches!(fault, Some(MigrationFault::AlreadyApplied { version: 3, .. })));
}

#[test]
fn loader_error_config_validation_failed_keeps_its_source() {
    let err = LoaderError::ConfigValidationFailed {
        extension: ExtensionId::new("config-ext"),
        source: ExtensionConfigError::InvalidValue {
            key: "port".to_string(),
            message: "not a number".to_string(),
        },
    };
    let msg = err.to_string();
    assert!(msg.contains("config-ext"));
    assert!(msg.contains("'port'"));
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn loader_error_reserved_path_collision_display() {
    let err = LoaderError::ReservedPathCollision {
        extension: ExtensionId::new("path-ext"),
        path: "/api/v1/oauth".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("path-ext"));
    assert!(msg.contains("/api/v1/oauth"));
    assert!(msg.contains("reserved"));
}

#[test]
fn loader_error_invalid_base_path_display() {
    let err = LoaderError::InvalidBasePath {
        extension: ExtensionId::new("bad-path-ext"),
        path: "/not-api/v1".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("bad-path-ext"));
    assert!(msg.contains("/not-api/v1"));
    assert!(msg.contains("must be / or start with /api/"));
}

#[test]
fn loader_error_circular_dependency_display() {
    let err = LoaderError::DependencyCycle {
        chain: "a -> b -> a".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("a -> b -> a"));
    assert!(msg.contains("Dependency cycle"));
}

#[test]
fn loader_error_is_std_error() {
    let err = LoaderError::DuplicateExtension(ExtensionId::new("test"));
    let _: &dyn std::error::Error = &err;
    assert!(!err.to_string().is_empty());
}

#[test]
fn config_error_not_found_display() {
    let err = ExtensionConfigError::NotFound("database.url".to_string());
    let msg = err.to_string();
    assert!(msg.contains("database.url"));
    assert!(msg.contains("not found"));
}

#[test]
fn config_error_invalid_value_display() {
    let err = ExtensionConfigError::InvalidValue {
        key: "port".to_string(),
        message: "must be between 1 and 65535".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("port"));
    assert!(msg.contains("must be between"));
}

#[test]
fn config_error_parse_error_display() {
    let err = ExtensionConfigError::ParseError {
        source: "unexpected token at line 5".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("unexpected token"));
}

#[test]
fn config_error_schema_validation_display() {
    let err = ExtensionConfigError::SchemaValidation("missing required field 'name'".into());
    let msg = err.to_string();
    assert!(msg.contains("missing required field"));
}

#[test]
fn config_error_is_std_error() {
    let err = ExtensionConfigError::NotFound("key".to_string());
    let _: &dyn std::error::Error = &err;
    assert!(!err.to_string().is_empty());
}
