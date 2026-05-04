//! [`ConfigExtensionTyped`] — typed contract for extensions that own a
//! configuration namespace.

use serde_json::Value as JsonValue;

use crate::error::ConfigError;
use crate::types::ExtensionMeta;

/// Typed contract for an extension that owns a configuration namespace.
pub trait ConfigExtensionTyped: ExtensionMeta {
    /// Returns the static configuration prefix this extension owns
    /// (e.g. `"agents"` selects `agents.*` keys).
    fn config_prefix(&self) -> &'static str;

    /// Validates a runtime configuration block against this extension's
    /// expectations. The default accepts anything.
    fn validate_config(&self, _config: &JsonValue) -> Result<(), ConfigError> {
        Ok(())
    }

    /// Returns a JSON schema describing this extension's configuration
    /// shape, if any.
    fn config_schema(&self) -> Option<JsonValue> {
        None
    }
}
