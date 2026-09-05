//! The host section of the GUI wire: the local proxy, per-host health folded
//! out of the probe snapshot, and the row the Agents list renders.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use systemprompt_bridge::integration::agent_fleet::AgentFleets;
use systemprompt_bridge::integration::agent_health::{
    AgentAction, AgentReason, AgentState, AgentSurface, AgentVerdict,
};
use systemprompt_bridge::integration::host_app::{AppInstallState, ConfigFormat, HostKind};
use systemprompt_bridge::integration::profile_state::{ProfileState, StaleReason};
use systemprompt_bridge::integration::{GeneratedProfile, HostAppSnapshot};
use systemprompt_bridge::proxy_probe::{ProxyHealth, ProxyProbeState};
use systemprompt_bridge::verdict::Tone;
use systemprompt_bridge::wire::first_run::FirstRunPayload;
use systemprompt_bridge::wire::hosts::{
    HostEntryPayload, HostHealthPayload, HostsPayload, ProxyPayload,
};

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("payload serialises")
}

fn snapshot(profile_state: ProfileState, keys: BTreeMap<String, String>) -> HostAppSnapshot {
    HostAppSnapshot {
        host_id: "claude_code",
        display_name: "Claude Code",
        profile_state,
        profile_source: Some("/etc/managed.json".to_owned()),
        profile_keys: keys,
        host_running: true,
        host_processes: vec!["claude".to_owned()],
        app_installed: AppInstallState::Installed,
        probed_at_unix: 1_700_000_042,
    }
}

fn verdict() -> AgentVerdict {
    AgentVerdict {
        state: AgentState::Ready,
        tone: Tone::Ok,
        reason: AgentReason::Awaiting,
        action: Some(AgentAction::Open),
        is_set_up: true,
        is_installed: true,
        is_running: true,
    }
}

#[test]
fn proxy_payload_flattens_the_health_and_ships_the_governing_fact() {
    let health = ProxyHealth {
        url: Some("http://127.0.0.1:8899".to_owned()),
        state: ProxyProbeState::Listening,
        http_status: Some(200),
        latency_ms: Some(3),
        error: None,
        probed_at_unix: 1_700_000_000,
    };
    let v = json_of(&ProxyPayload::from(&health));

    assert_eq!(v["url"], json!("http://127.0.0.1:8899"));
    assert_eq!(v["state"], json!("listening"));
    assert_eq!(v["http_status"], json!(200));
    assert_eq!(v["verdict"]["code"], json!("listening"));
    assert_eq!(v["verdict"]["tone"], json!("ok"));
    assert_eq!(v["governing"], json!(true));
}

#[test]
fn a_refused_proxy_is_not_governing() {
    let health = ProxyHealth {
        state: ProxyProbeState::Refused,
        ..Default::default()
    };
    let v = json_of(&ProxyPayload::from(&health));

    assert_eq!(v["state"], json!("refused"));
    assert_eq!(v["governing"], json!(false));
    assert_ne!(v["verdict"]["tone"], json!("ok"));
}

#[test]
fn host_health_ships_verdicts_and_plain_facts_never_the_raw_profile_state() {
    let snap = snapshot(ProfileState::Installed, BTreeMap::new());
    let v = json_of(&HostHealthPayload::from(&snap));

    assert_eq!(v["profile"]["code"], json!("installed"));
    assert_eq!(v["app"]["code"], json!("installed"));
    assert_eq!(v["host_running"], json!(true));
    assert_eq!(v["host_processes"], json!(["claude"]));
    assert_eq!(v["missing_required"], json!([]));
    assert_eq!(v["probed_at_unix"], json!(1_700_000_042u64));
    assert!(
        v.get("profile_state").is_none(),
        "the raw probe snapshot must not cross the wire: {v}"
    );
}

#[test]
fn a_partial_profile_lists_the_keys_that_are_missing() {
    let snap = snapshot(
        ProfileState::Partial {
            missing_required: vec!["apiBaseUrl".to_owned(), "authToken".to_owned()],
        },
        BTreeMap::new(),
    );
    let v = json_of(&HostHealthPayload::from(&snap));

    assert_eq!(v["profile"]["code"], json!("partial"));
    assert_eq!(v["missing_required"], json!(["apiBaseUrl", "authToken"]));
}

#[test]
fn a_stale_profile_reports_only_its_code_not_its_cause() {
    let snap = snapshot(
        ProfileState::Stale {
            reason: StaleReason::LoopbackSecret,
        },
        BTreeMap::new(),
    );
    let v = json_of(&HostHealthPayload::from(&snap));

    assert_eq!(v["profile"]["code"], json!("stale"));
    assert!(
        v["profile"].get("reason").is_none(),
        "the stale cause is not on the wire: {}",
        v["profile"]
    );
}

#[test]
fn inference_models_are_split_and_trimmed_from_the_profile_key() {
    let mut keys = BTreeMap::new();
    keys.insert(
        "inferenceModels".to_owned(),
        " sonnet , opus ,, haiku ".to_owned(),
    );
    let snap = snapshot(ProfileState::Installed, keys);
    let v = json_of(&HostHealthPayload::from(&snap));

    assert_eq!(v["inference_models"], json!(["sonnet", "opus", "haiku"]));
}

#[test]
fn a_host_with_no_inference_models_key_ships_an_empty_list() {
    let snap = snapshot(ProfileState::Absent, BTreeMap::new());
    let v = json_of(&HostHealthPayload::from(&snap));

    assert_eq!(v["inference_models"], json!([]));
    assert_eq!(v["profile"]["code"], json!("absent"));
}

#[test]
fn host_entry_carries_the_row_the_agents_list_renders() {
    let snap = snapshot(ProfileState::Installed, BTreeMap::new());
    let generated = GeneratedProfile {
        path: "/tmp/managed.json".to_owned(),
        bytes: 128,
        payload_uuid: "payload-uuid".to_owned(),
        profile_uuid: "profile-uuid".to_owned(),
    };
    let entry = HostEntryPayload {
        id: "claude_code",
        display_name: "Claude Code",
        kind: HostKind::CliTool,
        description: "the CLI",
        icon: "claude",
        config_format: ConfigFormat::Json,
        download_url: "https://example.invalid/dl",
        install_action_label: "Install",
        can_open: true,
        can_verify: true,
        can_repair: true,
        can_open_config: true,
        can_remove: true,
        probe_in_flight: false,
        enabled: true,
        last_generated_profile: Some(&generated),
        health: Some(HostHealthPayload::from(&snap)),
        compatible_models: vec!["sonnet".to_owned()],
        models_checked: true,
        compatible_models_available: true,
        unconfigured_providers: Vec::new(),
        model_protocols: vec!["anthropic".to_owned()],
        model_protocols_overridden: false,
        surface: AgentSurface::LocalProfile,
        verdict: verdict(),
    };
    let v = json_of(&entry);

    assert_eq!(v["id"], json!("claude_code"));
    assert_eq!(v["kind"], json!("cli-tool"));
    assert_eq!(v["config_format"], json!("json"));
    assert_eq!(v["surface"], json!("local-profile"));
    assert_eq!(v["can_open"], json!(true));
    assert_eq!(v["enabled"], json!(true));
    assert_eq!(v["compatible_models"], json!(["sonnet"]));
    assert_eq!(v["model_protocols"], json!(["anthropic"]));
    assert_eq!(
        v["last_generated_profile"]["path"],
        json!("/tmp/managed.json")
    );
    assert_eq!(v["health"]["profile"]["code"], json!("installed"));
    assert_eq!(v["verdict"]["state"], json!("ready"));
    assert_eq!(v["verdict"]["tone"], json!("ok"));
    assert_eq!(v["verdict"]["is_set_up"], json!(true));
    assert_eq!(v["verdict"]["action"]["code"], json!("open"));
}

#[test]
fn an_unprobed_host_ships_null_health_rather_than_omitting_it() {
    let entry = HostEntryPayload {
        id: "codex",
        display_name: "Codex",
        kind: HostKind::DesktopApp,
        description: "",
        icon: "",
        config_format: ConfigFormat::Toml,
        download_url: "",
        install_action_label: "Add",
        can_open: false,
        can_verify: false,
        can_repair: false,
        can_open_config: false,
        can_remove: false,
        probe_in_flight: true,
        enabled: false,
        last_generated_profile: None,
        health: None,
        compatible_models: Vec::new(),
        models_checked: false,
        compatible_models_available: false,
        unconfigured_providers: vec!["openai".to_owned()],
        model_protocols: Vec::new(),
        model_protocols_overridden: true,
        surface: AgentSurface::SyncOnly,
        verdict: verdict(),
    };
    let v = json_of(&entry);

    assert_eq!(v["health"], Value::Null);
    assert_eq!(v["last_generated_profile"], Value::Null);
    assert_eq!(v["kind"], json!("desktop-app"));
    assert_eq!(v["config_format"], json!("toml"));
    assert_eq!(v["surface"], json!("sync-only"));
    assert_eq!(v["probe_in_flight"], json!(true));
    assert_eq!(v["unconfigured_providers"], json!(["openai"]));
    assert_eq!(v["model_protocols_overridden"], json!(true));
}

#[test]
fn hosts_payload_fails_closed_before_the_first_manifest_sync() {
    let health = ProxyHealth::default();
    let payload = HostsPayload {
        host_apps: Vec::new(),
        local_proxy: ProxyPayload::from(&health),
        hosts_gated: false,
        agent_fleet: AgentFleets::fold(&[]),
        agents_onboarded: false,
        first_run: FirstRunPayload {
            active: true,
            done: false,
            phase: "hosts",
            sync: "pending",
            error: None,
            hosts: Vec::new(),
        },
    };
    let v = json_of(&payload);

    assert_eq!(v["hosts_gated"], json!(false));
    assert_eq!(v["host_apps"], json!([]));
    assert_eq!(v["agents_onboarded"], json!(false));
    assert_eq!(v["local_proxy"]["state"], json!("unknown"));
    assert_eq!(v["first_run"]["active"], json!(true));
    assert_eq!(v["first_run"]["phase"], json!("hosts"));
    assert_eq!(v["first_run"]["sync"], json!("pending"));
    assert_eq!(v["agent_fleet"]["all"]["total"], json!(0));
}

#[test]
fn the_fleet_summary_is_folded_from_the_very_verdicts_the_rows_carry() {
    let fleets = AgentFleets::fold(&[verdict(), verdict()]);
    let v = json_of(&fleets);

    assert_eq!(v["all"]["total"], json!(2));
    assert_eq!(v["all"]["ready"], json!(2));
    assert_eq!(v["set_up"]["total"], json!(2));
}

// The dev fixtures in `bin/bridge/web/dev/fixtures/` are the only way anyone
// sees this UI on Linux (`just bridge-preview`), and `fixture_verdicts.rs`
// regenerates their `verdict` subtree so it cannot lie. Nothing checked the
// rest of the host entry, and it drifted: four of the five entries in
// `healthy.json` were missing `can_open` entirely, so the fixtures were
// rendering a payload the app never emits.
//
// That is the same failure that took the sign-in screen down — JS reading a
// field the wire does not carry — so the guard is the key set itself, in both
// directions. A field added to `HostEntryPayload` without reaching the fixtures
// fails here, and so does a fixture key no payload produces.
fn fixture_paths() -> Vec<std::path::PathBuf> {
    let dir = systemprompt_test_fixtures::repo_path("bin/bridge/web/dev/fixtures");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read fixtures dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures found in {}", dir.display());
    paths
}

fn host_entry_keys() -> Vec<String> {
    let entry = HostEntryPayload {
        id: "claude-code",
        display_name: "Claude Code",
        kind: HostKind::CliTool,
        description: "",
        icon: "claude-code",
        config_format: ConfigFormat::Json,
        download_url: "",
        install_action_label: "",
        can_open: false,
        can_verify: false,
        can_repair: false,
        can_open_config: false,
        can_remove: false,
        probe_in_flight: false,
        enabled: true,
        last_generated_profile: None,
        health: None,
        compatible_models: Vec::new(),
        models_checked: false,
        compatible_models_available: false,
        unconfigured_providers: Vec::new(),
        model_protocols: Vec::new(),
        model_protocols_overridden: false,
        surface: AgentSurface::SyncOnly,
        verdict: verdict(),
    };
    let mut keys: Vec<String> = json_of(&entry)
        .as_object()
        .expect("host entry is an object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

#[test]
fn every_fixture_host_entry_carries_exactly_the_wire_key_set() {
    let expected = host_entry_keys();
    let mut checked = 0_usize;

    for path in fixture_paths() {
        let raw = std::fs::read_to_string(&path).expect("read fixture");
        let value: Value = serde_json::from_str(&raw).expect("fixture is JSON");
        let Some(hosts) = value.get("host_apps").and_then(Value::as_array) else {
            continue;
        };
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        for entry in hosts {
            let obj = entry
                .as_object()
                .unwrap_or_else(|| panic!("{name}: host_apps entry is not an object"));
            let id = obj
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<no id>")
                .to_owned();
            let mut actual: Vec<String> = obj.keys().cloned().collect();
            actual.sort();

            let missing: Vec<&String> = expected.iter().filter(|k| !actual.contains(k)).collect();
            let extra: Vec<&String> = actual.iter().filter(|k| !expected.contains(k)).collect();

            assert!(
                missing.is_empty(),
                "{name}: host '{id}' is missing wire field(s) {missing:?} — the fixture \
                 renders a payload the app never emits. Regenerate the fixture."
            );
            assert!(
                extra.is_empty(),
                "{name}: host '{id}' carries key(s) {extra:?} that HostEntryPayload does not \
                 produce. A consumer reading them reads something that is never there."
            );
            checked += 1;
        }
    }

    assert!(checked > 0, "no fixture host entries were checked");
}

// Why: a fixture id that is not a real host is a preview of a screen the app
// cannot produce, and a missing one is a screen no reviewer ever sees —
// `no-models.json` and `proxy-down.json` had quietly dropped `codex-cli`.
#[test]
fn every_fixture_lists_exactly_the_known_hosts() {
    use systemprompt_models::bridge::profile::KNOWN_HOSTS;

    let mut expected: Vec<&str> = KNOWN_HOSTS.to_vec();
    expected.sort_unstable();
    let mut checked = 0_usize;

    for path in fixture_paths() {
        let raw = std::fs::read_to_string(&path).expect("read fixture");
        let value: Value = serde_json::from_str(&raw).expect("fixture is JSON");
        let Some(hosts) = value.get("host_apps").and_then(Value::as_array) else {
            continue;
        };
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let mut ids: Vec<&str> = hosts
            .iter()
            .filter_map(|h| h.get("id").and_then(Value::as_str))
            .collect();
        ids.sort_unstable();
        assert_eq!(
            ids, expected,
            "{name}: fixture host ids have drifted from KNOWN_HOSTS"
        );
        checked += 1;
    }

    assert!(checked > 0, "no fixtures carried host_apps");
}
