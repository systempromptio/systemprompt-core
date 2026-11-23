//! Accessors for the handles the composition root injects into the context:
//! the route classifier, the identity of the binary being served, the files
//! configuration and the event broadcaster set, and the deployment token
//! authority built over the signing keys.
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

use super::AppContext;

impl AppContext {
    pub const fn route_classifier(&self) -> &Arc<systemprompt_models::RouteClassifier> {
        &self.cfg.route_classifier
    }

    pub const fn build_identity(&self) -> &systemprompt_models::BuildIdentity {
        &self.cfg.build
    }

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
