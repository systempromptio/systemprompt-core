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
        credential_error: None,
        startup_faults: Vec::new(),
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

#[test]
fn startup_faults_and_a_credential_error_reach_the_webview_as_named_fields() {
    let proxy = ProxyHealth::default();
    let update = UpdateUiState::default();
    let mut next = payload(&proxy, &update);
    next.credential_error = Some("authentication: token rejected");
    next.startup_faults = vec![StartupFaultPayload {
        component: "proxy port file",
        error: "parse bridge-proxy.json: expected value",
    }];

    let json = serde_json::to_value(&next).expect("the state payload serialises");
    assert_eq!(
        json["startup_faults"][0]["component"],
        serde_json::json!("proxy port file")
    );
    assert_eq!(
        json["startup_faults"][0]["error"],
        serde_json::json!("parse bridge-proxy.json: expected value")
    );
    assert_eq!(
        json["credential_error"],
        serde_json::json!("authentication: token rejected")
    );
}

#[test]
fn a_healthy_state_omits_credential_error_and_carries_an_empty_fault_list() {
    let proxy = ProxyHealth::default();
    let update = UpdateUiState::default();
    let json = serde_json::to_value(payload(&proxy, &update)).expect("serialises");

    assert!(
        json.get("credential_error").is_none(),
        "no error means the key is absent, not null: {json}"
    );
    assert_eq!(
        json["startup_faults"],
        serde_json::json!([]),
        "the fault list is always present so the webview can render it unconditionally"
    );
}

#[test]
fn a_startup_fault_is_semantic_state_rather_than_telemetry() {
    let proxy = ProxyHealth::default();
    let update = UpdateUiState::default();
    let first = payload(&proxy, &update).semantic_value().unwrap();

    let mut next = payload(&proxy, &update);
    next.startup_faults = vec![StartupFaultPayload {
        component: "log file",
        error: "permission denied",
    }];
    assert_ne!(
        first,
        next.semantic_value().unwrap(),
        "a new start-up fault must wake the webview up"
    );

    let mut next = payload(&proxy, &update);
    next.credential_error = Some("authentication: token rejected");
    assert_ne!(first, next.semantic_value().unwrap());
}
