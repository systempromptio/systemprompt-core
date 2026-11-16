//! The Teams member: the `teams` config section, and behind the `http`
//! feature the inbound route set, the HTTP clients it provides and the
//! messaging dispatcher it requires.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Deserialize;
use systemprompt_extension::prelude::*;
use systemprompt_manifest::services::TeamsAppConfig;

#[derive(Debug, Clone, Copy, Default)]
pub struct TeamsExtension;

/// The `teams` profile section: Teams apps keyed by name.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct TeamsAppsConfig(pub BTreeMap<String, TeamsAppConfig>);

impl ExtensionConfig for TeamsAppsConfig {
    const PREFIX: &'static str = "teams";

    fn validate(&self) -> Result<(), ExtensionConfigError> {
        for (name, app) in &self.0 {
            app.validate(name)
                .map_err(|e| ExtensionConfigError::SchemaValidation(Box::new(e)))?;
        }
        Ok(())
    }
}

impl Extension for TeamsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "teams",
            name: "Microsoft Teams",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.config::<TeamsAppsConfig>();
        #[cfg(feature = "http")]
        {
            c.requires::<LateBound<dyn systemprompt_traits::MessagingDispatcher>>()
                .provide(|_: &Resolved<'_>| {
                    crate::http::TeamsClients::build()
                        .map_err(|e| ProvideError::Failed(Box::new(e)))
                })
                .routes(crate::http::routes());
        }
        Ok(())
    }
}
