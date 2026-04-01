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
    ext.validate_config(&config)
        .expect("default validate_config should accept any value");
}

#[test]
fn test_config_extension_typed_default_config_schema() {
    let ext = BasicConfigExtension;
    assert_eq!(ext.config_schema(), None, "default config_schema should return None");
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

    ext.validate_config(&config)
        .expect("config with required_field and optional_field should validate");
}

#[test]
fn test_config_extension_custom_validate_missing_required() {
    let ext = ValidatingConfigExtension;
    let config = json!({
        "optional_field": 42
    });

    let err = ext
        .validate_config(&config)
        .expect_err("config missing required_field should fail validation");

    match err {
        ConfigError::InvalidValue { key, message } => {
            assert_eq!(key, "required_field");
            assert!(message.contains("mandatory"));
        }
        other => panic!("Expected InvalidValue error, got {:?}", other),
    }
}

#[test]
fn test_config_extension_custom_validate_not_object() {
    let ext = ValidatingConfigExtension;
    let config = json!("not an object");

    let err = ext
        .validate_config(&config)
        .expect_err("non-object config should fail validation");

    match err {
        ConfigError::ParseError(msg) => {
            assert!(msg.contains("object"), "error message should mention 'object', got: {}", msg);
        }
        other => panic!("Expected ParseError, got {:?}", other),
    }
}

#[test]
fn test_config_extension_custom_validate_array() {
    let ext = ValidatingConfigExtension;
    let config = json!([1, 2, 3]);

    let err = ext
        .validate_config(&config)
        .expect_err("array config should fail validation");
    assert!(
        matches!(err, ConfigError::ParseError(_)),
        "expected ParseError for array input, got {:?}",
        err
    );
}

#[test]
fn test_config_extension_custom_validate_null() {
    let ext = ValidatingConfigExtension;
    let config = json!(null);

    let err = ext
        .validate_config(&config)
        .expect_err("null config should fail validation");
    assert!(
        matches!(err, ConfigError::ParseError(_)),
        "expected ParseError for null input, got {:?}",
        err
    );
}

// =============================================================================
// Config Schema Tests
// =============================================================================

#[test]
fn test_config_extension_custom_schema() {
    let ext = ValidatingConfigExtension;
    let schema = ext.config_schema();

    let schema = schema.expect("ValidatingConfigExtension should provide a schema");
    assert_eq!(schema["type"], "object");
    assert!(
        schema["properties"]["required_field"].is_object(),
        "schema should define required_field property"
    );
    let required = schema["required"]
        .as_array()
        .expect("schema should have a 'required' array");
    assert!(
        required.contains(&json!("required_field")),
        "required array should contain 'required_field'"
    );
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
    let schema = ext
        .config_schema()
        .expect("ValidatingConfigExtension should provide a schema");
    assert_eq!(schema["type"], "object");
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

    let err = ext
        .validate_config(&config)
        .expect_err("empty object should fail because required_field is missing");
    assert!(
        matches!(err, ConfigError::InvalidValue { ref key, .. } if key == "required_field"),
        "expected InvalidValue for required_field, got {:?}",
        err
    );
}

#[test]
fn test_validate_with_extra_fields() {
    let ext = ValidatingConfigExtension;
    let config = json!({
        "required_field": "value",
        "extra_field": "ignored",
        "another_extra": 123
    });

    ext.validate_config(&config)
        .expect("extra fields should be allowed when required_field is present");
}

#[test]
fn test_validate_with_wrong_type_for_optional() {
    let ext = ValidatingConfigExtension;
    let config = json!({
        "required_field": "value",
        "optional_field": "not a number"
    });

    ext.validate_config(&config)
        .expect("simple validator does not check types, so wrong type for optional_field should pass");
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
    let required = schema["required"]
        .as_array()
        .expect("schema should have a 'required' array");
    assert!(
        required.contains(&json!("required_field")),
        "required array should list 'required_field'"
    );

    // 3. Validate good config
    let good_config = json!({ "required_field": "test" });
    ext.validate_config(&good_config)
        .expect("config with required_field should validate");

    // 4. Validate bad config
    let bad_config = json!({});
    let err = ext
        .validate_config(&bad_config)
        .expect_err("empty config should fail validation");
    assert!(
        matches!(err, ConfigError::InvalidValue { ref key, .. } if key == "required_field"),
        "expected InvalidValue for required_field, got {:?}",
        err
    );
}
