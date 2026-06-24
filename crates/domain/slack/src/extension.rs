use std::collections::BTreeMap;

use serde_json::Value as JsonValue;
use systemprompt_extension::prelude::*;
use systemprompt_models::services::SlackAppConfig;

#[derive(Debug, Clone, Copy, Default)]
pub struct SlackExtension;

impl Extension for SlackExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "slack",
            name: "Slack",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn config_prefix(&self) -> Option<&str> {
        Some("slack")
    }

    fn config_schema(&self) -> Option<JsonValue> {
        serde_json::to_value(schemars::schema_for!(BTreeMap<String, SlackAppConfig>)).ok()
    }

    fn validate_config(&self, config: &JsonValue) -> Result<(), ConfigError> {
        let apps: BTreeMap<String, SlackAppConfig> = serde_json::from_value(config.clone())
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

register_extension!(SlackExtension);
