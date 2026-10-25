//! Accessors for the handles the composition root injects into the context:
//! the files configuration, the event broadcaster set and the connector token
//! cache.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::SecretsService;
use systemprompt_events::EventBroadcasters;
use systemprompt_files::FilesConfig;
use systemprompt_loader::BootContext;
use systemprompt_manifest::Profile;
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

    pub const fn files_config(&self) -> &Arc<FilesConfig> {
        &self.cfg.files_config
    }

    pub const fn event_broadcasters(&self) -> &Arc<EventBroadcasters> {
        self.subsystems.event_router.broadcasters()
    }

    pub const fn connector_tokens(&self) -> &Arc<TokenCache> {
        &self.subsystems.connector_tokens
    }
}
