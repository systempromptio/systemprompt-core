//! The `wire::payloads` sub-payloads as JSON: the front end reads these field
//! names directly, so the shape is the contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};
use systemprompt_bridge::ids::McpSessionId;
use systemprompt_bridge::proxy::mcp_probe::{McpAuthState, McpServerAuth, McpTool};
use systemprompt_bridge::update::UpdateUiState;
use systemprompt_bridge::validate::{CheckLevel, CheckLine, ValidationReport};
use systemprompt_bridge::verdict::{Tone, Verdict};
use systemprompt_bridge::wire::codes::GatewayCode;
use systemprompt_bridge::wire::payloads::{
    CachedTokenPayload, CheckLinePayload, GatewayStatusPayload, McpServerAuthPayload,
    ProxyStatsPayload, UpdatePayload, ValidationPayload, VerifiedIdentityPayload,
};

fn server(state: McpAuthState) -> McpServerAuth {
    McpServerAuth {
        id: "srv".to_owned(),
        url: "http://127.0.0.1:9/mcp".to_owned(),
        state,
        tools: vec![
            McpTool {
                name: "search".to_owned(),
                description: Some("find things".to_owned()),
            },
            McpTool {
                name: "bare".to_owned(),
                description: None,
            },
        ],
        http_status: Some(200),
        latency_ms: Some(12),
        error: None,
        session_id: Some(McpSessionId::new("sess-1")),
        probed_at_unix: 1_700_000_000,
    }
}

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("payload serialises")
}

#[test]
fn mcp_server_payload_flattens_the_server_and_ships_its_verdict() {
    let auth = server(McpAuthState::Authenticated);
    let payload = McpServerAuthPayload::from(&auth);
    let v = json_of(&payload);

    assert_eq!(v["id"], json!("srv"));
    assert_eq!(v["url"], json!("http://127.0.0.1:9/mcp"));
    assert_eq!(v["state"], json!("authenticated"));
    assert_eq!(v["session_id"], json!("sess-1"));
    assert_eq!(v["verdict"]["tone"], json!("ok"));
    assert_eq!(v["verdict"]["code"], json!("authenticated"));
    assert_eq!(v["needs_sign_in"], json!(false));
    assert_eq!(v["conclusive"], json!(true));
    assert_eq!(v["shows_tools"], json!(true));
}

#[test]
fn mcp_server_payload_carries_the_sign_in_flags_for_a_rejected_server() {
    let auth = server(McpAuthState::GatewayUnauthorized);
    let v = json_of(&McpServerAuthPayload::from(&auth));

    assert_eq!(v["verdict"]["tone"], json!("err"));
    assert_eq!(v["verdict"]["code"], json!("gateway-unauthorized"));
    assert_eq!(v["needs_sign_in"], json!(true));
    assert_eq!(v["conclusive"], json!(true));
    assert_eq!(v["shows_tools"], json!(false));
}

#[test]
fn an_inconclusive_probe_is_unknown_toned_and_not_a_sign_in_prompt() {
    let auth = server(McpAuthState::ProxyUnreachable);
    let v = json_of(&McpServerAuthPayload::from(&auth));

    assert_eq!(v["verdict"]["tone"], json!("unknown"));
    assert_eq!(v["needs_sign_in"], json!(false));
    assert_eq!(v["conclusive"], json!(false));
}

#[test]
fn a_tool_without_a_description_omits_the_field_entirely() {
    let auth = server(McpAuthState::Authenticated);
    let v = json_of(&McpServerAuthPayload::from(&auth));
    let tools = v["tools"].as_array().expect("tools is an array");

    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["description"], json!("find things"));
    assert!(
        tools[1].get("description").is_none(),
        "a tool with no description must not ship a null: {}",
        tools[1]
    );
}

#[test]
fn validation_payload_maps_each_level_to_its_tone_and_folds_the_worst() {
    let report = ValidationReport {
        lines: vec![
            CheckLine {
                level: CheckLevel::Ok,
                label: "binary".to_owned(),
                value: "v1".to_owned(),
            },
            CheckLine {
                level: CheckLevel::Info,
                label: "note".to_owned(),
                value: "fyi".to_owned(),
            },
            CheckLine {
                level: CheckLevel::Warn,
                label: "last sync".to_owned(),
                value: "never".to_owned(),
            },
        ],
        any_failed: false,
    };
    let v = json_of(&ValidationPayload::from(&report));

    assert_eq!(v["lines"][0]["tone"], json!("ok"));
    assert_eq!(v["lines"][0]["label"], json!("binary"));
    assert_eq!(v["lines"][0]["value"], json!("v1"));
    assert_eq!(v["lines"][1]["tone"], json!("unknown"));
    assert_eq!(v["lines"][2]["tone"], json!("warn"));
    assert_eq!(v["any_failed"], json!(false));
    assert_eq!(v["verdict"]["tone"], json!("warn"));
    assert_eq!(v["verdict"]["code"], json!("attention"));
}

#[test]
fn a_failed_check_makes_the_validation_verdict_failing() {
    let report = ValidationReport {
        lines: vec![CheckLine {
            level: CheckLevel::Fail,
            label: "gateway /health".to_owned(),
            value: "refused".to_owned(),
        }],
        any_failed: true,
    };
    let v = json_of(&ValidationPayload::from(&report));

    assert_eq!(v["verdict"]["tone"], json!("err"));
    assert_eq!(v["verdict"]["code"], json!("failing"));
    assert_eq!(v["any_failed"], json!(true));
}

#[test]
fn an_empty_validation_report_reads_healthy() {
    let report = ValidationReport {
        lines: Vec::new(),
        any_failed: false,
    };
    let v = json_of(&ValidationPayload::from(&report));

    assert_eq!(v["verdict"]["code"], json!("healthy"));
    assert_eq!(v["lines"], json!([]));
}

#[test]
fn check_line_payload_serialises_tone_label_and_value() {
    let line = CheckLinePayload {
        tone: Tone::Err,
        label: "pinned manifest pubkey",
        value: "absent",
    };
    let v = json_of(&line);

    assert_eq!(v["tone"], json!("err"));
    assert_eq!(v["label"], json!("pinned manifest pubkey"));
    assert_eq!(v["value"], json!("absent"));
}

#[test]
fn update_payload_flattens_the_phase_and_ships_the_button_flags() {
    let state = UpdateUiState::Available {
        version: "1.2.3".to_owned(),
        notes_url: Some("https://example.invalid/notes".to_owned()),
    };
    let v = json_of(&UpdatePayload::from(&state));

    assert_eq!(v["phase"], json!("available"));
    assert_eq!(v["version"], json!("1.2.3"));
    assert_eq!(v["notes_url"], json!("https://example.invalid/notes"));
    assert_eq!(v["tone"], json!("warn"));
    assert_eq!(v["can_install"], json!(true));
    assert_eq!(v["can_restart"], json!(false));
    assert_eq!(v["in_progress"], json!(false));
}

#[test]
fn a_download_in_flight_is_probing_and_offers_no_button() {
    let state = UpdateUiState::Downloading {
        version: "1.2.3".to_owned(),
        percent: 40,
    };
    let v = json_of(&UpdatePayload::from(&state));

    assert_eq!(v["phase"], json!("downloading"));
    assert_eq!(v["percent"], json!(40));
    assert_eq!(v["tone"], json!("probing"));
    assert_eq!(v["can_install"], json!(false));
    assert_eq!(v["can_restart"], json!(false));
    assert_eq!(v["in_progress"], json!(true));
}

#[test]
fn a_staged_update_offers_restart_rather_than_install() {
    let state = UpdateUiState::Ready {
        version: "9.9.9".to_owned(),
    };
    let v = json_of(&UpdatePayload::from(&state));

    assert_eq!(v["phase"], json!("ready"));
    assert_eq!(v["can_install"], json!(false));
    assert_eq!(v["can_restart"], json!(true));
    assert_eq!(v["in_progress"], json!(false));
}

#[test]
fn the_default_update_state_is_unknown_with_no_affordances() {
    let state = UpdateUiState::default();
    let v = json_of(&UpdatePayload::from(&state));

    assert_eq!(v["phase"], json!("unknown"));
    assert_eq!(v["tone"], json!("unknown"));
    assert_eq!(v["can_install"], json!(false));
    assert_eq!(v["can_restart"], json!(false));
    assert_eq!(v["in_progress"], json!(false));
}

#[test]
fn a_failed_update_is_err_toned_and_keeps_its_message() {
    let state = UpdateUiState::Failed {
        message: "signature mismatch".to_owned(),
    };
    let v = json_of(&UpdatePayload::from(&state));

    assert_eq!(v["phase"], json!("failed"));
    assert_eq!(v["message"], json!("signature mismatch"));
    assert_eq!(v["tone"], json!("err"));
}

#[test]
fn proxy_stats_default_to_zero_counters_the_gui_can_render() {
    let v = json_of(&ProxyStatsPayload::default());

    for field in [
        "forwarded_total",
        "messages_total",
        "tokens_in_total",
        "tokens_out_total",
        "last_status",
        "last_latency_ms",
        "last_forwarded_at_unix",
    ] {
        assert_eq!(v[field], json!(0), "{field} must default to zero");
    }
}

#[test]
fn cached_token_payload_ships_ttl_and_length_never_the_token() {
    let v = json_of(&CachedTokenPayload {
        ttl_seconds: 900,
        length: 412,
    });

    assert_eq!(v["ttl_seconds"], json!(900));
    assert_eq!(v["length"], json!(412));
    assert_eq!(
        v.as_object().map(serde_json::Map::len),
        Some(2),
        "the cached-token payload must carry nothing else: {v}"
    );
}

#[test]
fn gateway_status_flattens_its_verdict_and_omits_absent_optionals() {
    let payload = GatewayStatusPayload {
        verdict: Verdict::new(Tone::Probing, GatewayCode::Probing),
        settled: false,
        latency_ms: None,
        reason: None,
    };
    let v = json_of(&payload);

    assert_eq!(v["tone"], json!("probing"));
    assert_eq!(v["code"], json!("probing"));
    assert_eq!(v["settled"], json!(false));
    assert!(v.get("latency_ms").is_none(), "absent latency must vanish");
    assert!(v.get("reason").is_none(), "absent reason must vanish");
}

#[test]
fn gateway_status_carries_latency_and_reason_when_they_exist() {
    let payload = GatewayStatusPayload {
        verdict: Verdict::new(Tone::Err, GatewayCode::Unreachable),
        settled: true,
        latency_ms: Some(2_400),
        reason: Some("connection refused"),
    };
    let v = json_of(&payload);

    assert_eq!(v["tone"], json!("err"));
    assert_eq!(v["code"], json!("unreachable"));
    assert_eq!(v["settled"], json!(true));
    assert_eq!(v["latency_ms"], json!(2_400));
    assert_eq!(v["reason"], json!("connection refused"));
}

#[test]
fn verified_identity_keeps_null_claims_so_the_gui_can_tell_them_apart() {
    let payload = VerifiedIdentityPayload {
        email: Some("a@example.invalid"),
        user_id: Some("u-1"),
        tenant_id: None,
        exp_unix: Some(1_800_000_000),
        verified_at_unix: 1_700_000_000,
    };
    let v = json_of(&payload);

    assert_eq!(v["email"], json!("a@example.invalid"));
    assert_eq!(v["user_id"], json!("u-1"));
    assert_eq!(v["tenant_id"], Value::Null);
    assert_eq!(v["exp_unix"], json!(1_800_000_000));
    assert_eq!(v["verified_at_unix"], json!(1_700_000_000));
}
