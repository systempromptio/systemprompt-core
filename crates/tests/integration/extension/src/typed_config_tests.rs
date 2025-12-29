//! Tests for typed config extension traits.

use serde_json::json;

use systemprompt_extension::error::ConfigError;
use systemprompt_extension::prelude::*;
use systemprompt_extension::typed::ConfigExtensionTyped;

// =============================================================================
// Test Extension Types
// =============================================================================

#[derive(Default, Debug)]
struct BasicConfigExtension;

impl ExtensionType for BasicConfigExtension {
    const ID: &'static str = "basic-config";
    const NAME: &'static str = "Basic Config Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for BasicConfigExtension {}

impl ConfigExtensionTyped for BasicConfigExtension {
    fn config_prefix(&self) -> &'static str {
        "basic"
    }
    // Uses default validate_config and config_schema
}

#[derive(Default, Debug)]
struct ValidatingConfigExtension;

impl ExtensionType for ValidatingConfigExtension {
    const ID: &'static str = "validating-config";
    const NAME: &'static str = "Validating Config Extension";
    const VERSION: &'static str = "2.0.0";
}

impl NoDependencies for ValidatingConfigExtension {}

impl ConfigExtensionTyped for ValidatingConfigExtension {
    fn config_prefix(&self) -> &'static str {
        "validating"
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<(), ConfigError> {
        if let Some(obj) = config.as_object() {
            if !obj.contains_key("required_field") {
                return Err(ConfigError::InvalidValue {
                    key: "required_field".to_string(),
                    message: "required_field is mandatory".to_string(),
                });
            }
            Ok(())
        } else {
            Err(ConfigError::ParseError("Config must be an object".to_string()))
        }
    }

    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "required_field": { "type": "string" },
                "optional_field": { "type": "number" }
            },
            "required": ["required_field"]
        }))
    }
}

#[derive(Default, Debug)]
struct NestedPrefixConfigExtension;

impl ExtensionType for NestedPrefixConfigExtension {
    const ID: &'static str = "nested-config";
    const NAME: &'static str = "Nested Config Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for NestedPrefixConfigExtension {}

impl ConfigExtensionTyped for NestedPrefixConfigExtension {
    fn config_prefix(&self) -> &'static str {
        "app.extensions.nested"
    }
}

// =============================================================================
// ConfigExtensionTyped Trait Tests
// =============================================================================

#[test]
fn test_config_extension_typed_prefix() {
    let ext = BasicConfigExtension;
    assert_eq!(ext.config_prefix(), "basic");
}

#[test]
fn test_config_extension_typed_nested_prefix() {
    let ext = NestedPrefixConfigExtension;
    assert_eq!(ext.config_prefix(), "app.extensions.nested");
}

#[test]
fn test_config_extension_typed_default_validate_config() {
    let ext = BasicConfigExtension;
    let config = json!({ "anything": "goes" });
    assert!(ext.validate_config(&config).is_ok());
}

#[test]
fn test_config_extension_typed_default_config_schema() {
    let ext = BasicConfigExtension;
    assert!(ext.config_schema().is_none());
}

#[test]
fn test_config_extension_typed_metadata() {
    let ext = BasicConfigExtension;

    assert_eq!(ext.id(), "basic-config");
    assert_eq!(ext.name(), "Basic Config Extension");
    assert_eq!(ext.version(), "1.0.0");
}

// =============================================================================
// Custom Validation Tests
// =============================================================================

#[test]
fn test_config_extension_custom_validate_success() {
    let ext = ValidatingConfigExtension;
    let config = json!({
        "required_field": "value",
        "optional_field": 42
    });

    assert!(ext.validate_config(&config).is_ok());
}

#[test]
fn test_config_extension_custom_validate_missing_required() {
    let ext = ValidatingConfigExtension;
    let config = json!({
        "optional_field": 42
    });

    let result = ext.validate_config(&config);
    assert!(result.is_err());

    match result {
        Err(ConfigError::InvalidValue { key, message }) => {
            assert_eq!(key, "required_field");
            assert!(message.contains("mandatory"));
        }
        _ => panic!("Expected InvalidValue error"),
    }
}

#[test]
fn test_config_extension_custom_validate_not_object() {
    let ext = ValidatingConfigExtension;
    let config = json!("not an object");

    let result = ext.validate_config(&config);
    assert!(result.is_err());

    match result {
        Err(ConfigError::ParseError(msg)) => {
            assert!(msg.contains("object"));
        }
        _ => panic!("Expected ParseError"),
    }
}

#[test]
fn test_config_extension_custom_validate_array() {
    let ext = ValidatingConfigExtension;
    let config = json!([1, 2, 3]);

    let result = ext.validate_config(&config);
    assert!(result.is_err());
}

#[test]
fn test_config_extension_custom_validate_null() {
    let ext = ValidatingConfigExtension;
    let config = json!(null);

    let result = ext.validate_config(&config);
    assert!(result.is_err());
}

// =============================================================================
// Config Schema Tests
// =============================================================================

#[test]
fn test_config_extension_custom_schema() {
    let ext = ValidatingConfigExtension;
    let schema = ext.config_schema();

    assert!(schema.is_some());

    let schema = schema.expect("schema exists");
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["required_field"].is_object());
    assert!(schema["required"].is_array());
}

#[test]
fn test_config_extension_schema_has_properties() {
    let ext = ValidatingConfigExtension;
    let schema = ext.config_schema().expect("schema exists");

    let properties = schema["properties"].as_object().expect("properties is object");
    assert!(properties.contains_key("required_field"));
    assert!(properties.contains_key("optional_field"));
}

#[test]
fn test_config_extension_schema_required_fields() {
    let ext = ValidatingConfigExtension;
    let schema = ext.config_schema().expect("schema exists");

    let required = schema["required"].as_array().expect("required is array");
    assert!(required.contains(&json!("required_field")));
}

// =============================================================================
// Trait Object Tests
// =============================================================================

#[test]
fn test_config_extension_as_trait_object() {
    let ext: &dyn ConfigExtensionTyped = &BasicConfigExtension;
    assert_eq!(ext.config_prefix(), "basic");
}

#[test]
fn test_config_extension_boxed_trait_object() {
    let ext: Box<dyn ConfigExtensionTyped> = Box::new(ValidatingConfigExtension);
    assert_eq!(ext.config_prefix(), "validating");
    assert!(ext.config_schema().is_some());
}

// =============================================================================
// Multiple Config Extensions Tests
// =============================================================================

#[test]
fn test_multiple_config_extensions() {
    let extensions: Vec<&dyn ConfigExtensionTyped> = vec![
        &BasicConfigExtension,
        &ValidatingConfigExtension,
        &NestedPrefixConfigExtension,
    ];

    assert_eq!(extensions.len(), 3);

    let prefixes: Vec<_> = extensions.iter().map(|e| e.config_prefix()).collect();
    assert!(prefixes.contains(&"basic"));
    assert!(prefixes.contains(&"validating"));
    assert!(prefixes.contains(&"app.extensions.nested"));
}

#[test]
fn test_filter_extensions_with_schema() {
    let extensions: Vec<&dyn ConfigExtensionTyped> = vec![
        &BasicConfigExtension,
        &ValidatingConfigExtension,
        &NestedPrefixConfigExtension,
    ];

    let with_schema: Vec<_> = extensions
        .iter()
        .filter(|e| e.config_schema().is_some())
        .collect();

    assert_eq!(with_schema.len(), 1);
    assert_eq!(with_schema[0].config_prefix(), "validating");
}

// =============================================================================
// Config Prefix Format Tests
// =============================================================================

#[test]
fn test_config_prefix_simple() {
    let ext = BasicConfigExtension;
    assert!(!ext.config_prefix().contains('.'));
}

#[test]
fn test_config_prefix_dotted() {
    let ext = NestedPrefixConfigExtension;
    assert!(ext.config_prefix().contains('.'));
    assert_eq!(ext.config_prefix().split('.').count(), 3);
}

// =============================================================================
// Validation Edge Cases
// =============================================================================

#[test]
fn test_validate_empty_object() {
    let ext = ValidatingConfigExtension;
    let config = json!({});

    // Empty object should fail because required_field is missing
    assert!(ext.validate_config(&config).is_err());
}

#[test]
fn test_validate_with_extra_fields() {
    let ext = ValidatingConfigExtension;
    let config = json!({
        "required_field": "value",
        "extra_field": "ignored",
        "another_extra": 123
    });

    // Extra fields should be allowed
    assert!(ext.validate_config(&config).is_ok());
}

#[test]
fn test_validate_with_wrong_type_for_optional() {
    let ext = ValidatingConfigExtension;
    let config = json!({
        "required_field": "value",
        "optional_field": "not a number"
    });

    // The simple validator doesn't check types, so this passes
    // A real implementation would use JSON schema validation
    assert!(ext.validate_config(&config).is_ok());
}

// =============================================================================
// Integration Tests
// =============================================================================

#[test]
fn test_config_extension_full_workflow() {
    let ext = ValidatingConfigExtension;

    // 1. Check prefix
    assert_eq!(ext.config_prefix(), "validating");

    // 2. Get schema
    let schema = ext.config_schema().expect("should have schema");
    assert!(schema["required"].as_array().is_some());

    // 3. Validate good config
    let good_config = json!({ "required_field": "test" });
    assert!(ext.validate_config(&good_config).is_ok());

    // 4. Validate bad config
    let bad_config = json!({});
    assert!(ext.validate_config(&bad_config).is_err());
}
