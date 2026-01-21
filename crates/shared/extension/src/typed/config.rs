use serde_json::Value as JsonValue;

use crate::error::ConfigError;
use crate::types::ExtensionMeta;

pub trait ConfigExtensionTyped: ExtensionMeta {
    fn config_prefix(&self) -> &'static str;

    fn validate_config(&self, config: &JsonValue) -> Result<(), ConfigError> {
        let _ = config;
        Ok(())
    }

    fn config_schema(&self) -> Option<JsonValue> {
        None
    }
}
