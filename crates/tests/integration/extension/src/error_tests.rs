//! Tests for extension error types.

use systemprompt_extension::error::{ExtensionConfigError, LoaderError};
use systemprompt_identifiers::ExtensionId;

#[test]
fn test_loader_error_missing_dependency_display() {
    let err = LoaderError::MissingDependency {
        extension: ExtensionId::new("blog"),
        dependency: ExtensionId::new("auth"),
    };
    let msg = err.to_string();
    assert!(msg.contains("blog"));
    assert!(msg.contains("auth"));
    assert!(msg.contains("requires dependency"));
}


#[test]
fn test_loader_error_duplicate_extension_display() {
    let err = LoaderError::DuplicateExtension(ExtensionId::new("auth"));
    let msg = err.to_string();
    assert!(msg.contains("auth"));
    assert!(msg.contains("already registered"));
}


#[test]
fn test_loader_error_initialization_failed_display() {
    let err = LoaderError::InitializationFailed {
        extension: ExtensionId::new("payment"),
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
        extension: ExtensionId::new("users"),
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
        extension: ExtensionId::new("smtp"),
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
        extension: ExtensionId::new("bad-ext"),
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
        extension: ExtensionId::new("my-ext"),
        path: "/invalid/path".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("my-ext"));
    assert!(msg.contains("/invalid/path"));
    assert!(msg.contains("must be / or start with /api/"));
}

#[test]
fn test_loader_error_circular_dependency_display() {
    let err = LoaderError::DependencyCycle {
        chain: "a -> b -> c -> a".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("a -> b -> c -> a"));
    assert!(msg.contains("Dependency cycle"));
}

#[test]
fn test_config_error_not_found_display() {
    let err = ExtensionConfigError::NotFound("database.host".to_string());
    let msg = err.to_string();
    assert!(msg.contains("database.host"));
    assert!(msg.contains("not found"));
}


#[test]
fn test_config_error_invalid_value_display() {
    let err = ExtensionConfigError::InvalidValue {
        key: "port".to_string(),
        message: "must be a positive integer".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("port"));
    assert!(msg.contains("must be a positive integer"));
}


#[test]
fn test_config_error_parse_error_display() {
    let err = ExtensionConfigError::ParseError {
        source: "invalid JSON at line 5".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("invalid JSON at line 5"));
    assert!(msg.contains("parse"));
}

#[test]
fn test_config_error_schema_validation_display() {
    let err = ExtensionConfigError::SchemaValidation("missing required property 'name'".into());
    let msg = err.to_string();
    assert!(msg.contains("missing required property"));
    assert!(msg.contains("Schema validation"));
}

#[test]
fn test_loader_error_variant_matching() {
    let errors = vec![
        LoaderError::MissingDependency {
            extension: ExtensionId::new("a"),
            dependency: ExtensionId::new("b"),
        },
        LoaderError::DuplicateExtension(ExtensionId::new("c")),
        LoaderError::InitializationFailed {
            extension: ExtensionId::new("d"),
            message: "failed".to_string(),
        },
        LoaderError::SchemaInstallationFailed {
            extension: ExtensionId::new("e"),
            message: "failed".to_string(),
        },
        LoaderError::ConfigValidationFailed {
            extension: ExtensionId::new("f"),
            message: "failed".to_string(),
        },
        LoaderError::ReservedPathCollision {
            extension: ExtensionId::new("g"),
            path: "/api/v1/users".to_string(),
        },
        LoaderError::InvalidBasePath {
            extension: ExtensionId::new("h"),
            path: "/bad".to_string(),
        },
        LoaderError::DependencyCycle {
            chain: "x -> y -> x".to_string(),
        },
        LoaderError::MigrationFailed {
            extension: ExtensionId::new("i"),
            message: "migration failed".to_string(),
        },
        LoaderError::MigrationStepFailed {
            extension: ExtensionId::new("i"),
            context: "Failed to record migration".to_string(),
            source: "connection reset".into(),
        },
        LoaderError::MigrationSlotReused {
            extension: ExtensionId::new("j"),
            version: 34,
            stored_name: "knowledge_bank".to_string(),
            current_name: "skill_invocation_view".to_string(),
        },
        LoaderError::MigrationReferencesDeclarativeObject {
            extension: ExtensionId::new("k"),
            migration: "012_view_dependency.sql".to_string(),
            kind: "view".to_string(),
            object: "current_items".to_string(),
            how: "FROM reference".to_string(),
        },
        LoaderError::MigrationTogglesTriggerByName {
            extension: ExtensionId::new("l"),
            migration: "020_toggle.sql".to_string(),
            table: "events".to_string(),
            trigger: "events_audit".to_string(),
        },
        LoaderError::DanglingTriggerRoutine {
            trigger: "events_audit".to_string(),
            table: "events".to_string(),
            function: "audit_event".to_string(),
            relation: "retired_table".to_string(),
        },
        LoaderError::MigrationChecksumDrift {
            extension: ExtensionId::new("m"),
            version: 7,
            name: "007_edited".to_string(),
            stored_checksum: "aaaa".to_string(),
            current_checksum: "bbbb".to_string(),
        },
    ];

    for err in errors {
        match &err {
            LoaderError::MissingDependency {
                extension,
                dependency,
            } => {
                assert!(!extension.as_str().is_empty());
                assert!(!dependency.as_str().is_empty());
            },
            LoaderError::DuplicateExtension(id) | LoaderError::RequiredExtensionDisabled(id) => {
                assert!(!id.as_str().is_empty());
            },
            LoaderError::DisabledDependency {
                extension,
                dependency,
            } => {
                assert!(!extension.as_str().is_empty());
                assert!(!dependency.as_str().is_empty());
            },
            LoaderError::InitializationFailed { extension, message } => {
                assert!(!extension.as_str().is_empty());
                assert!(!message.is_empty());
            },
            LoaderError::SchemaInstallationFailed { extension, message } => {
                assert!(!extension.as_str().is_empty());
                assert!(!message.is_empty());
            },
            LoaderError::MigrationFailed { extension, message } => {
                assert!(!extension.as_str().is_empty());
                assert!(!message.is_empty());
            },
            LoaderError::SchemaInstallationStepFailed {
                extension, context, ..
            }
            | LoaderError::MigrationStepFailed {
                extension, context, ..
            } => {
                assert!(!extension.as_str().is_empty());
                assert!(!context.is_empty());
            },
            LoaderError::MigrationReferencesDeclarativeObject {
                extension,
                migration,
                kind,
                object,
                how,
            } => {
                assert!(!extension.as_str().is_empty());
                assert!(!migration.is_empty());
                assert!(!kind.is_empty());
                assert!(!object.is_empty());
                assert!(!how.is_empty());
            },
            LoaderError::ConfigValidationFailed { extension, message } => {
                assert!(!extension.as_str().is_empty());
                assert!(!message.is_empty());
            },
            LoaderError::ReservedPathCollision { extension, path } => {
                assert!(!extension.as_str().is_empty());
                assert!(!path.is_empty());
            },
            LoaderError::InvalidBasePath { extension, path } => {
                assert!(!extension.as_str().is_empty());
                assert!(!path.is_empty());
            },
            LoaderError::DependencyCycle { chain } => {
                assert!(!chain.is_empty());
            },
            LoaderError::CrossExtensionAlterUndeclared { extension, table } => {
                assert!(!extension.as_str().is_empty());
                assert!(!table.is_empty());
            },
            LoaderError::DuplicateTableOwner {
                table,
                extension_a,
                extension_b,
            } => {
                assert!(!table.is_empty());
                assert!(!extension_a.as_str().is_empty());
                assert!(!extension_b.as_str().is_empty());
            },
            LoaderError::CrossExtensionTableNotOwned { extension, table } => {
                assert!(!extension.as_str().is_empty());
                assert!(!table.is_empty());
            },
            LoaderError::SeedInsertNotIdempotent { extension, seed } => {
                assert!(!extension.as_str().is_empty());
                assert!(!seed.is_empty());
            },
            LoaderError::InvalidSeedStatement {
                extension,
                seed,
                statement,
            } => {
                assert!(!extension.as_str().is_empty());
                assert!(!seed.is_empty());
                assert!(!statement.is_empty());
            },
            LoaderError::SeedFailed {
                extension,
                seed,
                context,
                ..
            } => {
                assert!(!extension.as_str().is_empty());
                assert!(!seed.is_empty());
                assert!(!context.is_empty());
            },
            LoaderError::MigrationNotReversible { extension, .. } => {
                assert!(!extension.as_str().is_empty());
            },
            LoaderError::MigrationSlotReused {
                extension,
                stored_name,
                current_name,
                ..
            } => {
                assert!(!extension.as_str().is_empty());
                // Both names carry the whole point of the error: a slot is
                // reused, so the message has to say which migration already
                // holds it and which one is trying to. One name alone leaves
                // the reader hunting for a file that no longer exists.
                assert!(!stored_name.is_empty());
                assert!(!current_name.is_empty());
                assert_ne!(stored_name, current_name);
            },
            LoaderError::MigrationTogglesTriggerByName {
                extension,
                migration,
                table,
                trigger,
            } => {
                assert!(!extension.as_str().is_empty());
                assert!(!migration.is_empty());
                assert!(!table.is_empty());
                assert!(!trigger.is_empty());
            },
            LoaderError::DanglingTriggerRoutine {
                trigger,
                table,
                function,
                relation,
            } => {
                assert!(!trigger.is_empty());
                assert!(!table.is_empty());
                assert!(!function.is_empty());
                assert!(!relation.is_empty());
            },
            LoaderError::MigrationChecksumDrift {
                extension,
                stored_checksum,
                current_checksum,
                ..
            } => {
                assert!(!extension.as_str().is_empty());
                assert_ne!(stored_checksum, current_checksum);
            },
        }
    }
}

#[test]
fn test_config_error_variant_matching() {
    let errors = vec![
        ExtensionConfigError::NotFound("key".to_string()),
        ExtensionConfigError::InvalidValue {
            key: "key".to_string(),
            message: "msg".to_string(),
        },
        ExtensionConfigError::ParseError {
            source: "parse error".into(),
        },
        ExtensionConfigError::SchemaValidation("schema error".into()),
    ];

    for err in errors {
        match &err {
            ExtensionConfigError::NotFound(key) => {
                assert!(!key.is_empty());
            },
            ExtensionConfigError::InvalidValue { key, message } => {
                assert!(!key.is_empty());
                assert!(!message.is_empty());
            },
            ExtensionConfigError::ParseError { source } => {
                assert!(!source.to_string().is_empty());
            },
            ExtensionConfigError::SchemaValidation(source) => {
                assert!(!source.to_string().is_empty());
            },
        }
    }
}
