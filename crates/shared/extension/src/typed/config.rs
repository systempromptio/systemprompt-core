//! [`ConfigExtensionTyped`] — typed contract for extensions that own a
//! configuration namespace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value as JsonValue;

use crate::error::ConfigError;
use crate::types::ExtensionMeta;

pub trait ConfigExtensionTyped: ExtensionMeta {
    fn config_prefix(&self) -> &'static str;

    fn validate_config(&self, _config: &JsonValue) -> Result<(), ConfigError> {
        Ok(())
    }

    fn config_schema(&self) -> Option<JsonValue> {
        None
    }
}
