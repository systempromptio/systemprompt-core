//! The Teams member: the `teams` config section and its inbound surface.
//!
//! Behind the `http` feature it declares the inbound route set, provides the
//! HTTP clients, and provides **sealed** the dispatch handle (`TeamsDispatch`)
//! over the sealed messaging dispatcher it requires: the handle dispatches
//! only an activity the member's token verifier has verified.
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
            c.requires::<LateRead<dyn systemprompt_traits::MessagingDispatcher>>()
                .provide(|_: &Resolved<'_>| {
                    crate::http::TeamsClients::build()
                        .map_err(|e| ProvideError::Failed(Box::new(e)))
                })
                .provide_sealed(
                    |r: &Resolved<'_>| -> Result<crate::http::TeamsDispatch, ProvideError> {
                        Ok(crate::http::TeamsDispatch::over(r.get()?))
                    },
                )
                .routes(crate::http::routes());
        }
        Ok(())
    }
}
