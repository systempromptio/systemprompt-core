//! Tests for extension error types.

use systemprompt_extension::error::{ConfigError, LoaderError};

// =============================================================================
// LoaderError Tests
// =============================================================================

#[test]
fn test_loader_error_missing_dependency_display() {
    let err = LoaderError::MissingDependency {
        extension: "blog".to_string(),
        dependency: "auth".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("blog"));
    assert!(msg.contains("auth"));
    assert!(msg.contains("requires dependency"));
}

#[test]
fn test_loader_error_missing_dependency_debug() {
    let err = LoaderError::MissingDependency {
        extension: "blog".to_string(),
        dependency: "auth".to_string(),
    };
    let debug = format!("{:?}", err);
    assert!(debug.contains("MissingDependency"));
    assert!(debug.contains("blog"));
    assert!(debug.contains("auth"));
}

#[test]
fn test_loader_error_duplicate_extension_display() {
    let err = LoaderError::DuplicateExtension("auth".to_string());
    let msg = err.to_string();
    assert!(msg.contains("auth"));
    assert!(msg.contains("already registered"));
}

#[test]
fn test_loader_error_duplicate_extension_debug() {
    let err = LoaderError::DuplicateExtension("auth".to_string());
    let debug = format!("{:?}", err);
    assert!(debug.contains("DuplicateExtension"));
    assert!(debug.contains("auth"));
}

#[test]
fn test_loader_error_initialization_failed_display() {
    let err = LoaderError::InitializationFailed {
        extension: "payment".to_string(),
        message: "database connection failed".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("payment"));
    assert!(msg.contains("database connection failed"));
    assert!(msg.contains("initialize"));
}

#[test]
fn test_loader_error_schema_installation_failed_display() {
    let err = LoaderError::SchemaInstallationFailed {
        extension: "users".to_string(),
        message: "table already exists".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("users"));
    assert!(msg.contains("table already exists"));
    assert!(msg.contains("schema"));
}

#[test]
fn test_loader_error_config_validation_failed_display() {
    let err = LoaderError::ConfigValidationFailed {
        extension: "smtp".to_string(),
        message: "missing required field: host".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("smtp"));
    assert!(msg.contains("missing required field"));
    assert!(msg.contains("Configuration validation"));
}

#[test]
fn test_loader_error_reserved_path_collision_display() {
    let err = LoaderError::ReservedPathCollision {
        extension: "bad-ext".to_string(),
        path: "/api/v1/users".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("bad-ext"));
    assert!(msg.contains("/api/v1/users"));
    assert!(msg.contains("reserved"));
}

#[test]
fn test_loader_error_invalid_base_path_display() {
    let err = LoaderError::InvalidBasePath {
        extension: "my-ext".to_string(),
        path: "/invalid/path".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("my-ext"));
    assert!(msg.contains("/invalid/path"));
    assert!(msg.contains("must start with /api/"));
}

#[test]
fn test_loader_error_circular_dependency_display() {
    let err = LoaderError::CircularDependency {
        chain: "a -> b -> c -> a".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("a -> b -> c -> a"));
    assert!(msg.contains("Circular dependency"));
}

// =============================================================================
// ConfigError Tests
// =============================================================================

#[test]
fn test_config_error_not_found_display() {
    let err = ConfigError::NotFound("database.host".to_string());
    let msg = err.to_string();
    assert!(msg.contains("database.host"));
    assert!(msg.contains("not found"));
}

#[test]
fn test_config_error_not_found_debug() {
    let err = ConfigError::NotFound("api_key".to_string());
    let debug = format!("{:?}", err);
    assert!(debug.contains("NotFound"));
    assert!(debug.contains("api_key"));
}

#[test]
fn test_config_error_invalid_value_display() {
    let err = ConfigError::InvalidValue {
        key: "port".to_string(),
        message: "must be a positive integer".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("port"));
    assert!(msg.contains("must be a positive integer"));
}

#[test]
fn test_config_error_invalid_value_debug() {
    let err = ConfigError::InvalidValue {
        key: "timeout".to_string(),
        message: "cannot be negative".to_string(),
    };
    let debug = format!("{:?}", err);
    assert!(debug.contains("InvalidValue"));
    assert!(debug.contains("timeout"));
}

#[test]
fn test_config_error_parse_error_display() {
    let err = ConfigError::ParseError("invalid JSON at line 5".to_string());
    let msg = err.to_string();
    assert!(msg.contains("invalid JSON at line 5"));
    assert!(msg.contains("parse"));
}

#[test]
fn test_config_error_schema_validation_display() {
    let err = ConfigError::SchemaValidation("missing required property 'name'".to_string());
    let msg = err.to_string();
    assert!(msg.contains("missing required property"));
    assert!(msg.contains("Schema validation"));
}

// =============================================================================
// Error Variant Matching Tests
// =============================================================================

#[test]
fn test_loader_error_variant_matching() {
    let errors = vec![
        LoaderError::MissingDependency {
            extension: "a".to_string(),
            dependency: "b".to_string(),
        },
        LoaderError::DuplicateExtension("c".to_string()),
        LoaderError::InitializationFailed {
            extension: "d".to_string(),
            message: "failed".to_string(),
        },
        LoaderError::SchemaInstallationFailed {
            extension: "e".to_string(),
            message: "failed".to_string(),
        },
        LoaderError::ConfigValidationFailed {
            extension: "f".to_string(),
            message: "failed".to_string(),
        },
        LoaderError::ReservedPathCollision {
            extension: "g".to_string(),
            path: "/api/v1/users".to_string(),
        },
        LoaderError::InvalidBasePath {
            extension: "h".to_string(),
            path: "/bad".to_string(),
        },
        LoaderError::CircularDependency {
            chain: "x -> y -> x".to_string(),
        },
    ];

    for err in errors {
        match &err {
            LoaderError::MissingDependency { extension, dependency } => {
                assert!(!extension.is_empty());
                assert!(!dependency.is_empty());
            }
            LoaderError::DuplicateExtension(id) => {
                assert!(!id.is_empty());
            }
            LoaderError::InitializationFailed { extension, message } => {
                assert!(!extension.is_empty());
                assert!(!message.is_empty());
            }
            LoaderError::SchemaInstallationFailed { extension, message } => {
                assert!(!extension.is_empty());
                assert!(!message.is_empty());
            }
            LoaderError::ConfigValidationFailed { extension, message } => {
                assert!(!extension.is_empty());
                assert!(!message.is_empty());
            }
            LoaderError::ReservedPathCollision { extension, path } => {
                assert!(!extension.is_empty());
                assert!(!path.is_empty());
            }
            LoaderError::InvalidBasePath { extension, path } => {
                assert!(!extension.is_empty());
                assert!(!path.is_empty());
            }
            LoaderError::CircularDependency { chain } => {
                assert!(!chain.is_empty());
            }
        }
    }
}

#[test]
fn test_config_error_variant_matching() {
    let errors = vec![
        ConfigError::NotFound("key".to_string()),
        ConfigError::InvalidValue {
            key: "key".to_string(),
            message: "msg".to_string(),
        },
        ConfigError::ParseError("parse error".to_string()),
        ConfigError::SchemaValidation("schema error".to_string()),
    ];

    for err in errors {
        match &err {
            ConfigError::NotFound(key) => {
                assert!(!key.is_empty());
            }
            ConfigError::InvalidValue { key, message } => {
                assert!(!key.is_empty());
                assert!(!message.is_empty());
            }
            ConfigError::ParseError(msg) => {
                assert!(!msg.is_empty());
            }
            ConfigError::SchemaValidation(msg) => {
                assert!(!msg.is_empty());
            }
        }
    }
}
