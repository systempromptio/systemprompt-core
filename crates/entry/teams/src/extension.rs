//! `Extension` registration for the Teams capability: config prefix and
//! schema, and behind the `http` feature the inbound route declaration and
//! the messaging dispatcher it requires.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde_json::Value as JsonValue;
use systemprompt_extension::prelude::*;
use systemprompt_manifest::services::TeamsAppConfig;

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

    #[cfg(feature = "http")]
    fn router(&self, ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        crate::http::declaration(ctx)
    }

    #[cfg(feature = "http")]
    fn requires(&self) -> Vec<StateRequirement> {
        vec![
            StateRequirement::of::<LateBound<dyn systemprompt_traits::MessagingDispatcher>>(),
            StateRequirement::of::<std::sync::Arc<dyn systemprompt_traits::SecretLookup>>(),
        ]
    }

    fn config_prefix(&self) -> Option<&str> {
        Some("teams")
    }

    // JSON: JSON Schema document for the extension's config block.
    fn config_schema(&self) -> Option<JsonValue> {
        serde_json::to_value(schemars::schema_for!(BTreeMap<String, TeamsAppConfig>))
            .inspect_err(|e| tracing::warn!(error = %e, "Failed to serialize Teams config schema"))
            .ok()
    }

    // JSON: Extension config block from the profile YAML; the extension owns it.
    fn validate_config(&self, config: &JsonValue) -> Result<(), ExtensionConfigError> {
        let apps: BTreeMap<String, TeamsAppConfig> = serde_json::from_value(config.clone())
            .map_err(|e| ExtensionConfigError::ParseError {
                source: Box::new(e),
            })?;
        for (name, app) in &apps {
            app.validate(name)
                .map_err(|e| ExtensionConfigError::SchemaValidation(Box::new(e)))?;
        }
        Ok(())
    }
}

register_extension!(TeamsExtension);
