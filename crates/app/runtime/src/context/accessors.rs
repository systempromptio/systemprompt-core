//! Accessors for the handles the composition root injects into the context:
//! the files configuration, the event broadcaster set and the connector token
//! cache, and the deployment token authority built over the signing keys.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::SecretsService;
use systemprompt_events::EventBroadcasters;
use systemprompt_files::FilesConfig;
use systemprompt_loader::BootContext;
use systemprompt_manifest::{Config, Profile};
use systemprompt_models::auth::TokenAuthority;
use systemprompt_security::DeploymentTokenAuthority;
use systemprompt_security::credential::TokenCache;

use super::AppContext;

impl AppContext {
    pub const fn boot(&self) -> &BootContext {
        &self.cfg.boot
    }

    pub fn profile(&self) -> &Profile {
        self.cfg.boot.profile()
    }

    pub fn secrets(&self) -> &Arc<SecretsService> {
        self.cfg.boot.secrets()
    }

    pub const fn config_arc(&self) -> &Arc<Config> {
        &self.cfg.config
    }

    pub const fn files_config(&self) -> &Arc<FilesConfig> {
        &self.cfg.files_config
    }

    pub const fn event_broadcasters(&self) -> &Arc<EventBroadcasters> {
        self.subsystems.event_router.broadcasters()
    }

    pub const fn connector_tokens(&self) -> &Arc<TokenCache> {
        &self.subsystems.connector_tokens
    }

    pub const fn documents(&self) -> &Arc<crate::documents::DocumentsRuntime> {
        &self.subsystems.documents
    }

    #[must_use]
    pub fn token_authority(&self) -> Arc<dyn TokenAuthority> {
        Arc::new(DeploymentTokenAuthority::new(
            self.subsystems.signing_keys.clone(),
            self.cfg.config.jwt_issuer.clone(),
        ))
    }
}
