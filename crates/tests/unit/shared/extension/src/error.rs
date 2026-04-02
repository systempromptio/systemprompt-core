use systemprompt_extension::error::{ConfigError, LoaderError};

#[test]
fn loader_error_missing_dependency_display() {
    let err = LoaderError::MissingDependency {
        extension: "my-ext".to_string(),
        dependency: "base-ext".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("my-ext"));
    assert!(msg.contains("base-ext"));
    assert!(msg.contains("requires dependency"));
}

#[test]
fn loader_error_duplicate_extension_display() {
    let err = LoaderError::DuplicateExtension("dup-ext".to_string());
    let msg = err.to_string();
    assert!(msg.contains("dup-ext"));
    assert!(msg.contains("already registered"));
}

#[test]
fn loader_error_initialization_failed_display() {
    let err = LoaderError::InitializationFailed {
        extension: "fail-ext".to_string(),
        message: "startup crashed".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("fail-ext"));
    assert!(msg.contains("startup crashed"));
}

#[test]
fn loader_error_schema_installation_failed_display() {
    let err = LoaderError::SchemaInstallationFailed {
        extension: "schema-ext".to_string(),
        message: "invalid SQL".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("schema-ext"));
    assert!(msg.contains("invalid SQL"));
}

#[test]
fn loader_error_migration_failed_display() {
    let err = LoaderError::MigrationFailed {
        extension: "mig-ext".to_string(),
        message: "column missing".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("mig-ext"));
    assert!(msg.contains("column missing"));
}

#[test]
fn loader_error_config_validation_failed_display() {
    let err = LoaderError::ConfigValidationFailed {
        extension: "config-ext".to_string(),
        message: "invalid port".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("config-ext"));
    assert!(msg.contains("invalid port"));
}

#[test]
fn loader_error_reserved_path_collision_display() {
    let err = LoaderError::ReservedPathCollision {
        extension: "path-ext".to_string(),
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
        extension: "bad-path-ext".to_string(),
        path: "/not-api/v1".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("bad-path-ext"));
    assert!(msg.contains("/not-api/v1"));
    assert!(msg.contains("must start with /api/"));
}

#[test]
fn loader_error_circular_dependency_display() {
    let err = LoaderError::CircularDependency {
        chain: "a -> b -> a".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("a -> b -> a"));
    assert!(msg.contains("Circular dependency"));
}

#[test]
fn loader_error_is_std_error() {
    let err = LoaderError::DuplicateExtension("test".to_string());
    let _: &dyn std::error::Error = &err;
    assert!(!err.to_string().is_empty());
}

#[test]
fn config_error_not_found_display() {
    let err = ConfigError::NotFound("database.url".to_string());
    let msg = err.to_string();
    assert!(msg.contains("database.url"));
    assert!(msg.contains("not found"));
}

#[test]
fn config_error_invalid_value_display() {
    let err = ConfigError::InvalidValue {
        key: "port".to_string(),
        message: "must be between 1 and 65535".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("port"));
    assert!(msg.contains("must be between"));
}

#[test]
fn config_error_parse_error_display() {
    let err = ConfigError::ParseError("unexpected token at line 5".to_string());
    let msg = err.to_string();
    assert!(msg.contains("unexpected token"));
}

#[test]
fn config_error_schema_validation_display() {
    let err = ConfigError::SchemaValidation("missing required field 'name'".to_string());
    let msg = err.to_string();
    assert!(msg.contains("missing required field"));
}

#[test]
fn config_error_is_std_error() {
    let err = ConfigError::NotFound("key".to_string());
    let _: &dyn std::error::Error = &err;
    assert!(!err.to_string().is_empty());
}
