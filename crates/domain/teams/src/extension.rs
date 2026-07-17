//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde_json::Value as JsonValue;
use systemprompt_extension::prelude::*;
use systemprompt_models::services::TeamsAppConfig;

/// `Extension` registration entry-point for the Microsoft Teams integration.
#[derive(Debug, Clone, Copy, Default)]
pub struct TeamsExtension;

impl Extension for TeamsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "teams",
            name: "Microsoft Teams",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn config_prefix(&self) -> Option<&str> {
        Some("teams")
    }

    fn config_schema(&self) -> Option<JsonValue> {
        serde_json::to_value(schemars::schema_for!(BTreeMap<String, TeamsAppConfig>)).ok()
    }

    fn validate_config(&self, config: &JsonValue) -> Result<(), ConfigError> {
        let apps: BTreeMap<String, TeamsAppConfig> = serde_json::from_value(config.clone())
            .map_err(|e| ConfigError::ParseError {
                message: e.to_string(),
            })?;
        for (name, app) in &apps {
            app.validate(name)
                .map_err(|e| ConfigError::SchemaValidation(e.to_string()))?;
        }
        Ok(())
    }
}

register_extension!(TeamsExtension);
