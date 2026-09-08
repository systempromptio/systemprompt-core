use systemprompt_bridge::proxy_probe::ProxyHealth;
use systemprompt_bridge::update::UpdateUiState;
use systemprompt_bridge::verdict::{Tone, Verdict};
use systemprompt_bridge::wire::StatePayload;
use systemprompt_bridge::wire::codes::*;
use systemprompt_bridge::wire::first_run::FirstRunPayload;
use systemprompt_bridge::wire::hosts::HostsPayload;
use systemprompt_bridge::wire::payloads::*;

fn payload<'a>(proxy: &'a ProxyHealth, update: &'a UpdateUiState) -> StatePayload<'a> {
    StatePayload {
        gateway_url: "https://gateway.example",
        gateway_configured: true,
        config_file: "config.toml",
        pat_file: "credential",
        config_present: true,
        pat_present: true,
        plugins_dir: None,
        last_sync_summary: None,
        last_sync_report: None,
        skill_count: None,
        agent_count: None,
        plugin_count: None,
        malformed_plugin_count: None,
        last_validation: None,
        last_validation_at_unix: None,
        health: Verdict::new(Tone::Unknown, HealthCode::NotChecked),
        provider_health: &[],
        sync_in_flight: false,
        cached_token: Some(CachedTokenPayload {
            ttl_seconds: 300,
            length: 123,
        }),
        token: Verdict::new(Tone::Ok, TokenCode::Valid),
        gateway_status: GatewayStatusPayload {
            verdict: Verdict::new(Tone::Ok, GatewayCode::Reachable),
            settled: true,
            latency_ms: Some(4),
            reason: None,
        },
        verified_identity: Some(VerifiedIdentityPayload {
            email: Some("user@example.com"),
            user_id: Some("user"),
            tenant_id: None,
            exp_unix: Some(900),
            verified_at_unix: 600,
        }),
        identity: Verdict::new(Tone::Ok, IdentityCode::SignedIn),
        cloud_tone: Tone::Ok,
        overall: Verdict::new(Tone::Ok, OverallCode::Ready),
        signed_in: true,
        last_probe_at_unix: Some(600),
        proxy_stats: ProxyStatsPayload::default(),
        mcp_auth: Vec::new(),
        mcp_auth_probe_in_flight: false,
        mcp_auth_tone: Tone::Unknown,
        update: UpdatePayload::from(update),
        app_name: "Bridge",
        sign_in_label: "Sign in",
        sign_in_hint: "",
        docs_url: "",
        contact_email: "",
        pitch_head: "",
        pitch_body: "",
        hosts: HostsPayload {
            host_apps: Vec::new(),
            local_proxy: proxy.into(),
            hosts_gated: false,
            agent_fleet: Default::default(),
            agents_onboarded: true,
            first_run: FirstRunPayload {
                active: false,
                done: true,
                phase: "done",
                sync: "done",
                error: None,
                hosts: Vec::new(),
            },
        },
    }
}

#[test]
fn probe_telemetry_does_not_change_semantic_state() {
    let proxy = ProxyHealth::default();
    let update = UpdateUiState::default();
    let first = payload(&proxy, &update).semantic_value().unwrap();
    let mut next = payload(&proxy, &update);
    next.last_probe_at_unix = Some(630);
    next.last_validation_at_unix = Some(630);
    next.gateway_status.latency_ms = Some(700);
    next.cached_token.as_mut().unwrap().ttl_seconds = 270;
    next.verified_identity.as_mut().unwrap().verified_at_unix = 630;
    next.proxy_stats.forwarded_total = 100;
    assert_eq!(first, next.semantic_value().unwrap());
}

#[test]
fn gateway_identity_and_verdict_changes_remain_visible() {
    let proxy = ProxyHealth::default();
    let update = UpdateUiState::default();
    let first = payload(&proxy, &update).semantic_value().unwrap();
    let mut next = payload(&proxy, &update);
    next.gateway_url = "https://other.example";
    assert_ne!(first, next.semantic_value().unwrap());
    let mut next = payload(&proxy, &update);
    next.verified_identity.as_mut().unwrap().user_id = Some("another-user");
    assert_ne!(first, next.semantic_value().unwrap());
    let mut next = payload(&proxy, &update);
    next.token = Verdict::new(Tone::Warn, TokenCode::Expiring);
    assert_ne!(first, next.semantic_value().unwrap());
}
