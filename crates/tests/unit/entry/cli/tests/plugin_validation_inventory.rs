//! Compile-time extension validation reports configuration and asset warnings.

use systemprompt_cli::CliConfig;
use systemprompt_cli::plugins::validate::{ValidateArgs, execute};
use systemprompt_extension::{Extension, ExtensionMetadata, register_extension};

#[derive(Default)]
struct NullConfigSchema;
impl Extension for NullConfigSchema {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "coverage-null-config",
            name: "Coverage Null Config",
            version: "0.0.1",
        }
    }
    fn config_prefix(&self) -> Option<&str> {
        Some("coverage_null")
    }
    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::Value::Null)
    }
}
register_extension!(NullConfigSchema);

#[derive(Default)]
struct AssetBearing;
impl Extension for AssetBearing {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "coverage-assets",
            name: "Coverage Assets",
            version: "0.0.1",
        }
    }
    fn declares_assets(&self) -> bool {
        true
    }
}
register_extension!(AssetBearing);

fn field<'a>(artifact: &'a serde_json::Value, heading: &str) -> &'a serde_json::Value {
    &artifact["sections"]
        .as_array()
        .expect("validation sections")
        .iter()
        .find(|section| section["heading"] == heading)
        .unwrap_or_else(|| panic!("missing {heading}: {artifact}"))["content"]
}

#[test]
fn validation_projects_config_and_asset_warnings_from_compiled_extensions() {
    let (output, valid) = execute(&ValidateArgs { verbose: false }, &CliConfig::new());
    assert!(valid);
    let artifact = serde_json::to_value(output.artifact()).expect("validation artifact");
    assert_eq!(field(&artifact, "valid"), true);
    assert!(
        field(&artifact, "extension_count")
            .as_u64()
            .expect("extension count")
            >= 2
    );
    let errors = field(&artifact, "errors")
        .as_array()
        .expect("validation errors");
    assert!(errors.is_empty(), "{artifact}");
    let warnings = field(&artifact, "warnings")
        .as_array()
        .expect("validation warnings");
    assert!(
        warnings
            .iter()
            .any(|warning| warning["extension_id"] == "coverage-null-config"
                && warning["warning_type"] == "config"
                && warning["message"] == "Config prefix defined but schema is null"),
        "{artifact}"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning["extension_id"] == "coverage-assets"
                && warning["warning_type"] == "asset_validation_skipped"
                && warning["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("infra db doctor"))),
        "{artifact}"
    );
}
