use crate::gui::state::{AppStateSnapshot, CachedToken, GatewayStatus, VerifiedIdentity};
use crate::validate::ValidationReport;

#[derive(Debug, Default)]
pub struct AppStateSnapshotBuilder {
    snap: AppStateSnapshot,
}

impl AppStateSnapshotBuilder {
    pub fn with_gateway_url(mut self, value: impl Into<String>) -> Self {
        self.snap.gateway_url = value.into();
        self
    }

    pub fn with_config_file(mut self, value: impl Into<String>) -> Self {
        self.snap.config_file = value.into();
        self
    }

    pub fn with_pat_file(mut self, value: impl Into<String>) -> Self {
        self.snap.pat_file = value.into();
        self
    }

    pub fn with_config_present(mut self, value: bool) -> Self {
        self.snap.config_present = value;
        self
    }

    pub fn with_pat_present(mut self, value: bool) -> Self {
        self.snap.pat_present = value;
        self
    }

    pub fn with_last_sync_summary(mut self, value: Option<String>) -> Self {
        self.snap.last_sync_summary = value;
        self
    }

    pub fn with_skill_count(mut self, value: Option<usize>) -> Self {
        self.snap.skill_count = value;
        self
    }

    pub fn with_agent_count(mut self, value: Option<usize>) -> Self {
        self.snap.agent_count = value;
        self
    }

    pub fn with_plugins_dir(mut self, value: Option<String>) -> Self {
        self.snap.plugins_dir = value;
        self
    }

    pub fn with_sync_in_flight(mut self, value: bool) -> Self {
        self.snap.sync_in_flight = value;
        self
    }

    pub fn with_last_action_message(mut self, value: Option<String>) -> Self {
        self.snap.last_action_message = value;
        self
    }

    pub fn with_last_validation(mut self, value: Option<ValidationReport>) -> Self {
        self.snap.last_validation = value;
        self
    }

    pub fn with_cached_token(mut self, value: Option<CachedToken>) -> Self {
        self.snap.cached_token = value;
        self
    }

    pub fn with_plugin_count(mut self, value: Option<usize>) -> Self {
        self.snap.plugin_count = value;
        self
    }

    pub fn with_malformed_plugin_count(mut self, value: Option<usize>) -> Self {
        self.snap.malformed_plugin_count = value;
        self
    }

    pub fn with_gateway_status(mut self, value: GatewayStatus) -> Self {
        self.snap.gateway_status = value;
        self
    }

    pub fn with_verified_identity(mut self, value: Option<VerifiedIdentity>) -> Self {
        self.snap.verified_identity = value;
        self
    }

    pub fn with_last_probe_at_unix(mut self, value: Option<u64>) -> Self {
        self.snap.last_probe_at_unix = value;
        self
    }

    pub fn build(self) -> AppStateSnapshot {
        self.snap
    }
}
