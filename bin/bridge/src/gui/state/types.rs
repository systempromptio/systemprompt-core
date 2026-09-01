//! Snapshot value types describing GUI application state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{TenantId, UserId};

use super::AppStateSnapshotBuilder;
use crate::gui::hosts::state::HostsState;
use crate::proxy::mcp_probe::McpServerAuth;
use crate::validate::ValidationReport;

#[derive(Debug, Clone, Default)]
pub enum GatewayStatus {
    #[default]
    Unknown,
    Probing,
    Reachable {
        latency_ms: u64,
    },
    Unreachable {
        reason: String,
    },
}

impl GatewayStatus {
    pub const fn is_reachable(&self) -> bool {
        matches!(self, Self::Reachable { .. })
    }
}

#[derive(Debug, Clone)]
pub struct VerifiedIdentity {
    pub email: Option<String>,
    pub user_id: Option<UserId>,
    pub tenant_id: Option<TenantId>,
    pub exp_unix: Option<u64>,
    pub verified_at_unix: u64,
}

#[derive(Debug, Clone)]
pub struct GatewayProbeOutcome {
    pub status: GatewayStatus,
    pub identity: Option<VerifiedIdentity>,
    pub at_unix: u64,
    pub provider_health: Vec<crate::gateway::types::ProviderHealth>,
}

#[derive(Debug, Clone, Default)]
pub struct AppStateSnapshot {
    pub gateway_url: String,
    pub config_file: String,
    pub pat_file: String,
    pub config_present: bool,
    pub pat_present: bool,
    pub last_sync_summary: Option<String>,
    pub skill_count: Option<usize>,
    pub agent_count: Option<usize>,
    pub plugins_dir: Option<String>,
    pub sync_in_flight: bool,
    pub last_sync_report: Option<crate::sync::SyncSummary>,
    pub last_validation: Option<ValidationReport>,
    pub last_validation_at_unix: Option<u64>,
    pub cached_token: Option<CachedToken>,
    pub plugin_count: Option<usize>,
    pub malformed_plugin_count: Option<usize>,
    pub gateway_status: GatewayStatus,
    pub verified_identity: Option<VerifiedIdentity>,
    pub last_probe_at_unix: Option<u64>,
    pub agents_onboarded: bool,
    pub first_run: crate::gui::first_run::state::FirstRunState,
    pub enabled_hosts: Vec<String>,
    pub host_model_protocols: std::collections::BTreeMap<String, Vec<String>>,
    pub provider_health: Vec<crate::gateway::types::ProviderHealth>,

    pub hosts: HostsState,

    pub mcp_auth: Vec<McpServerAuth>,
    pub mcp_auth_probe_in_flight: bool,

    pub update: crate::update::UpdateUiState,
}

impl AppStateSnapshot {
    // Why: deliberately not `!enabled_hosts.is_empty()` -- an instance may
    // disable every host, and that empty list is a real answer from a good
    // manifest, not a missing one. Anything gating on the instance's host
    // policy must ask this instead.
    pub const fn manifest_synced(&self) -> bool {
        self.last_sync_summary.is_some()
    }

    pub const fn signed_in(&self) -> bool {
        self.gateway_status.is_reachable() && self.verified_identity.is_some()
    }

    pub fn builder() -> AppStateSnapshotBuilder {
        AppStateSnapshotBuilder::default()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CachedToken {
    pub ttl_seconds: u64,
    pub length: usize,
}
