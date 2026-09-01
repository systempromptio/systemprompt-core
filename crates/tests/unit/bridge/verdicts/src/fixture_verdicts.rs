//! The dev fixtures now carry a verdict that Rust computes, which makes them
//! capable of lying: a hand-edited verdict would render a state the real code
//! never produces, and on Linux `just bridge-preview` is the only way anyone
//! sees this UI at all.
//!
//! So the fixtures are generated from the same `verdict()` the app calls, and
//! this test regenerates and compares. Run with `UPDATE_FIXTURES=1` to write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use systemprompt_bridge::integration::agent_health::{
    AgentFleets, AgentSurface, AgentVerdict, HostHealthInputs, HostModelViewRef, SYNC_ONLY_AGENTS,
    verdict,
};
use systemprompt_bridge::integration::host_app::{
    AppInstallState, HostAppSnapshot, ProfileState, StaleReason,
};
use systemprompt_bridge::proxy_probe::{ProxyHealth, ProxyProbeState};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../bin/bridge/web/dev/fixtures")
}

fn profile_state_of(health: &Value) -> ProfileState {
    let code = health
        .get("profile")
        .and_then(|p| p.get("code"))
        .and_then(Value::as_str)
        .unwrap_or("absent");
    match code {
        "installed" => ProfileState::Installed,
        "partial" => ProfileState::Partial {
            missing_required: strings(health.get("missing_required")),
        },
        // Why: the stale *cause* is not on the wire; either cause yields the
        // same verdict, so the fixture cannot tell them apart and need not.
        "stale" => ProfileState::Stale {
            reason: StaleReason::LoopbackSecret,
        },
        _ => ProfileState::Absent,
    }
}

// Why: the wire carries `health` — verdicts and facts — not the raw probe
// snapshot, so the snapshot is rebuilt from what the front end actually sees.
fn snapshot_of(v: &Value) -> HostAppSnapshot {
    HostAppSnapshot {
        host_id: "fixture",
        display_name: "fixture",
        profile_state: profile_state_of(v),
        profile_source: None,
        profile_keys: BTreeMap::new(),
        host_running: v
            .get("host_running")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        host_processes: Vec::new(),
        app_installed: match v
            .get("app")
            .and_then(|a| a.get("code"))
            .and_then(Value::as_str)
        {
            Some("installed") => AppInstallState::Installed,
            Some("not-installed") => AppInstallState::NotInstalled,
            _ => AppInstallState::Unknown,
        },
        probed_at_unix: v.get("probed_at_unix").and_then(Value::as_u64).unwrap_or(0),
    }
}

fn proxy_of(v: Option<&Value>) -> ProxyHealth {
    let state = match v.and_then(|p| p.get("state")).and_then(Value::as_str) {
        Some("unconfigured") => ProxyProbeState::Unconfigured,
        Some("listening") => ProxyProbeState::Listening,
        Some("refused") => ProxyProbeState::Refused,
        Some("timeout") => ProxyProbeState::Timeout,
        Some("http-error") => ProxyProbeState::HttpError,
        _ => ProxyProbeState::Unknown,
    };
    ProxyHealth {
        state,
        ..Default::default()
    }
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

// Recompute one fixture's verdicts and fleet fold. Returns the updated doc.
fn recompute(doc: &Value) -> Option<Value> {
    let mut doc = doc.clone();
    let proxy = proxy_of(doc.get("local_proxy"));
    let manifest_synced = doc
        .get("hosts_gated")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let hosts: Vec<Value> = doc
        .get("host_apps")?
        .as_array()?
        .iter()
        .filter(|h| h.get("surface").and_then(Value::as_str) != Some("sync-only"))
        .cloned()
        .collect();
    let mut updated = Vec::with_capacity(hosts.len());
    let mut verdicts: Vec<AgentVerdict> = Vec::with_capacity(hosts.len());

    for host in hosts {
        let snap = host.get("health").filter(|s| !s.is_null()).map(snapshot_of);
        let unconfigured = strings(host.get("unconfigured_providers"));
        let v = verdict(&HostHealthInputs {
            snapshot: snap.as_ref(),
            proxy: &proxy,
            models: HostModelViewRef {
                checked: host
                    .get("models_checked")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                available: host
                    .get("compatible_models_available")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                unconfigured_providers: &unconfigured,
            },
            has_download_url: host
                .get("download_url")
                .and_then(Value::as_str)
                .is_some_and(|u| !u.is_empty()),
            surface: AgentSurface::LocalProfile,
            manifest_synced,
            can_open: host
                .get("can_open")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        });

        let mut host = host.clone();
        if let Some(obj) = host.as_object_mut() {
            obj.insert("surface".to_owned(), json!("local-profile"));
            obj.insert(
                "verdict".to_owned(),
                serde_json::to_value(&v).unwrap_or_default(),
            );
        }
        // Only enabled hosts are folded, matching `serde::payload`.
        if host
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            verdicts.push(v);
        }
        updated.push(host);
    }

    // Mirror `serde::payload`: sync-only agents are appended after the hosts
    // that have a local profile. Without them the preview would not show
    // `claude-code` at all, which is the whole point of surfacing them.
    for agent in SYNC_ONLY_AGENTS {
        let v = verdict(&HostHealthInputs {
            snapshot: None,
            proxy: &proxy,
            models: HostModelViewRef {
                checked: false,
                available: false,
                unconfigured_providers: &[],
            },
            has_download_url: false,
            surface: AgentSurface::SyncOnly,
            manifest_synced,
            can_open: false,
        });
        updated.push(json!({
            "id": agent.id,
            "display_name": agent.display_name,
            "kind": "cli-tool",
            "description": agent.description,
            "icon": agent.icon,
            "config_format": "json",
            "download_url": "",
            "install_action_label": "",
            "probe_in_flight": false,
            "enabled": true,
            "last_generated_profile": Value::Null,
            "health": Value::Null,
            "compatible_models": [],
            "models_checked": false,
            "compatible_models_available": false,
            "unconfigured_providers": [],
            "model_protocols": [],
            "model_protocols_overridden": false,
            "surface": "sync-only",
            "verdict": serde_json::to_value(&v).unwrap_or_default(),
        }));
        verdicts.push(v);
    }

    let obj = doc.as_object_mut()?;
    obj.insert("host_apps".to_owned(), Value::Array(updated));
    obj.insert(
        "agent_fleet".to_owned(),
        serde_json::to_value(AgentFleets::fold(&verdicts)).unwrap_or_default(),
    );
    Some(doc)
}

#[test]
fn fixtures_carry_the_verdict_rust_computes() {
    let dir = fixtures_dir();
    let entries = std::fs::read_dir(&dir)
        .map(Iterator::collect::<Vec<_>>)
        .unwrap_or_default();
    assert!(
        !entries.is_empty(),
        "no fixtures found under {} — this test would pass vacuously",
        dir.display()
    );
    let update = std::env::var("UPDATE_FIXTURES").is_ok();
    let mut stale = Vec::new();
    let mut broken = Vec::new();

    for entry in entries.into_iter().flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // Why: an unreadable, unparseable or shapeless fixture is a broken
        // fixture, not a fixture to skip — skipping is how a truncated file
        // stayed green. It is recorded and the test fails on it below.
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                broken.push(format!("{}: unreadable: {e}", path.display()));
                continue;
            },
        };
        let doc = match serde_json::from_str::<Value>(&text) {
            Ok(d) => d,
            Err(e) => {
                broken.push(format!("{}: not JSON: {e}", path.display()));
                continue;
            },
        };
        let Some(expected) = recompute(&doc) else {
            broken.push(format!("{}: has no host_apps array", path.display()));
            continue;
        };

        if update {
            let mut out = serde_json::to_string_pretty(&expected).unwrap_or_default();
            out.push('\n');
            std::fs::write(&path, out).unwrap_or_default();
        } else if doc != expected {
            stale.push(path.display().to_string());
        }
    }

    assert!(
        broken.is_empty(),
        "these fixtures could not be checked at all:\n  {}",
        broken.join("\n  ")
    );
    assert!(
        stale.is_empty(),
        "these fixtures no longer match the verdict Rust computes; re-run with \
         UPDATE_FIXTURES=1:\n  {}",
        stale.join("\n  ")
    );
}
