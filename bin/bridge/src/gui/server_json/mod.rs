//! JSON payload shapes served to the GUI webview (state and proxy stats).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

mod payloads;

use payloads::{
    McpServerAuthPayload, ProxyStatsPayload, UpdatePayload, ValidationPayload,
    VerifiedIdentityPayload, cached_token_payload, gateway_status_payload, mcp_servers_payload,
    verified_identity_payload,
};

use crate::gui::hosts::serde::ProxyPayload;
use crate::gui::state::AppStateSnapshot;
use crate::verdict::Tone;
use crate::wire::StatePayload;

#[derive(Debug, Serialize)]
pub struct McpAuthPayload<'a> {
    pub servers: Vec<McpServerAuthPayload<'a>>,
    pub probing: bool,
    pub tone: Tone,
}

pub fn identity_payload(snap: &AppStateSnapshot) -> Option<VerifiedIdentityPayload<'_>> {
    snap.verified_identity
        .as_ref()
        .map(verified_identity_payload)
}

pub fn local_proxy_payload(snap: &AppStateSnapshot) -> ProxyPayload<'_> {
    ProxyPayload::from(&snap.hosts.local_proxy)
}

pub fn mcp_auth_payload(snap: &AppStateSnapshot) -> McpAuthPayload<'_> {
    McpAuthPayload {
        servers: mcp_servers_payload(snap),
        probing: snap.mcp_auth_probe_in_flight,
        tone: snap.mcp_auth_tone(),
    }
}

pub fn state_payload<'a>(
    snap: &'a AppStateSnapshot,
    proxy: &crate::proxy::ProxyHandle,
) -> StatePayload<'a> {
    {
        StatePayload {
            gateway_url: snap.gateway_url.as_str(),
            gateway_configured: snap.gateway_configured,
            config_file: snap.config_file.as_str(),
            pat_file: snap.pat_file.as_str(),
            config_present: snap.config_present,
            pat_present: snap.pat_present,
            plugins_dir: snap.plugins_dir.as_deref(),
            last_sync_summary: snap.last_sync_summary.as_deref(),
            last_sync_report: snap.last_sync_report.as_ref(),
            skill_count: snap.skill_count,
            agent_count: snap.agent_count,
            plugin_count: snap.plugin_count,
            malformed_plugin_count: snap.malformed_plugin_count,
            last_validation: snap.last_validation.as_ref().map(ValidationPayload::from),
            last_validation_at_unix: snap.last_validation_at_unix,
            health: snap.health_verdict(),
            provider_health: &snap.provider_health,
            credential_error: snap.credential_error.as_deref(),
            elevated: snap.elevated,
            startup_faults: snap
                .startup_faults
                .iter()
                .map(crate::wire::payloads::StartupFaultPayload::from)
                .collect(),
            sync_in_flight: snap.sync_in_flight,
            cached_token: snap.cached_token.as_ref().map(cached_token_payload),
            token: snap.token_verdict(),
            gateway_status: gateway_status_payload(&snap.gateway_status),
            verified_identity: snap
                .verified_identity
                .as_ref()
                .map(verified_identity_payload),
            identity: snap.identity_verdict(),
            cloud_tone: snap.cloud_tone(),
            overall: snap.overall_verdict(),
            signed_in: snap.signed_in(),
            last_probe_at_unix: snap.last_probe_at_unix,
            proxy_stats: ProxyStatsPayload::current(proxy),
            mcp_auth: mcp_servers_payload(snap),
            mcp_auth_probe_in_flight: snap.mcp_auth_probe_in_flight,
            mcp_auth_tone: snap.mcp_auth_tone(),
            update: UpdatePayload::from(&snap.update),
            pending_device_action: snap.pending_device_action,

            app_name: crate::brand::brand().app_name,
            sign_in_label: crate::brand::brand().sign_in_label,
            sign_in_hint: crate::brand::brand().sign_in_hint,
            docs_url: crate::brand::brand().docs_url,
            contact_email: crate::brand::brand().contact_email,
            pitch_head: crate::brand::brand().pitch_head,
            pitch_body: crate::brand::brand().pitch_body,

            hosts: crate::gui::hosts::serde::payload(snap),
        }
    }
}
