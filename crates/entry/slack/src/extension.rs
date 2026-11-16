//! The Slack member: the `slack` config section, and behind the `http`
//! feature the inbound route set, the HTTP clients it provides and the
//! messaging dispatcher it requires.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Deserialize;
use systemprompt_extension::prelude::*;
use systemprompt_manifest::services::SlackAppConfig;

#[derive(Debug, Clone, Copy, Default)]
pub struct SlackExtension;

/// The `slack` profile section: Slack apps keyed by name.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct SlackAppsConfig(pub BTreeMap<String, SlackAppConfig>);

impl ExtensionConfig for SlackAppsConfig {
    const PREFIX: &'static str = "slack";

    fn validate(&self) -> Result<(), ExtensionConfigError> {
        for (name, app) in &self.0 {
            app.validate(name)
                .map_err(|e| ExtensionConfigError::SchemaValidation(Box::new(e)))?;
        }
        Ok(())
    }
}

impl Extension for SlackExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "slack",
            name: "Slack",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.config::<SlackAppsConfig>();
        #[cfg(feature = "http")]
        {
            c.requires::<LateBound<dyn systemprompt_traits::MessagingDispatcher>>()
                .provide(|_: &Resolved<'_>| {
                    crate::http::SlackClients::build()
                        .map_err(|e| ProvideError::Failed(Box::new(e)))
                })
                .routes(crate::http::routes());
        }
        Ok(())
    }
}
