use crate::gui::state::{AppStateSnapshot, CachedToken, GatewayStatus, VerifiedIdentity};
#[cfg(target_os = "macos")]
use crate::integration::claude_desktop::ClaudeIntegrationSnapshot;

pub(crate) fn snapshot_to_json(snap: &AppStateSnapshot) -> String {
    serde_json::json!({
        "gateway_url": snap.gateway_url,
        "config_file": snap.config_file,
        "pat_file": snap.pat_file,
        "config_present": snap.config_present,
        "pat_present": snap.pat_present,
        "plugins_dir": snap.plugins_dir,
        "last_sync_summary": snap.last_sync_summary,
        "skill_count": snap.skill_count,
        "agent_count": snap.agent_count,
        "plugin_count": snap.plugin_count,
        "sync_in_flight": snap.sync_in_flight,
        "last_action_message": snap.last_action_message,
        "cached_token": snap.cached_token.as_ref().map(cached_token_json),
        "gateway_status": gateway_status_json(&snap.gateway_status),
        "verified_identity": snap.verified_identity.as_ref().map(verified_identity_json),
        "signed_in": snap.signed_in(),
        "last_probe_at_unix": snap.last_probe_at_unix,
        "claude_integration": claude_integration_value(snap),
        "last_generated_profile": snap.last_generated_profile.clone(),
    })
    .to_string()
}

#[cfg(target_os = "macos")]
fn claude_integration_json(snap: &ClaudeIntegrationSnapshot) -> serde_json::Value {
    serde_json::to_value(snap).unwrap_or(serde_json::Value::Null)
}

#[cfg(target_os = "macos")]
fn claude_integration_value(snap: &AppStateSnapshot) -> serde_json::Value {
    snap.claude_integration
        .as_ref()
        .map(claude_integration_json)
        .unwrap_or(serde_json::Value::Null)
}

#[cfg(not(target_os = "macos"))]
fn claude_integration_value(_snap: &AppStateSnapshot) -> serde_json::Value {
    serde_json::Value::Null
}

fn cached_token_json(t: &CachedToken) -> serde_json::Value {
    serde_json::json!({
        "ttl_seconds": t.ttl_seconds,
        "length": t.length,
    })
}

fn gateway_status_json(s: &GatewayStatus) -> serde_json::Value {
    match s {
        GatewayStatus::Unknown => serde_json::json!({"state": "unknown"}),
        GatewayStatus::Probing => serde_json::json!({"state": "probing"}),
        GatewayStatus::Reachable { latency_ms } => {
            serde_json::json!({"state": "reachable", "latency_ms": latency_ms})
        },
        GatewayStatus::Unreachable { reason } => {
            serde_json::json!({"state": "unreachable", "reason": reason})
        },
    }
}

fn verified_identity_json(v: &VerifiedIdentity) -> serde_json::Value {
    serde_json::json!({
        "email": v.email,
        "user_id": v.user_id,
        "tenant_id": v.tenant_id,
        "exp_unix": v.exp_unix,
        "verified_at_unix": v.verified_at_unix,
    })
}
