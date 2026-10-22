//! Golden catalog recorder (C-WP1): loads each fixture's legacy `services/ai`
//! tree through the production `ConfigLoader` and writes what every model id
//! and alias resolves to as `golden/resolution.json`. No secret store is
//! bootstrapped, so no real secret is read and no provider is demoted.
//!
//! Run: `cargo test -p systemprompt-gateway-golden-tests -- --ignored
//! record_golden_catalog` (`GOLDEN_OUT=<dir>` writes `<dir>/<fixture>.json`
//! instead of the fixture).

mod walk;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use systemprompt_loader::ConfigLoader;

const FIXTURES: [&str; 3] = ["core-default", "dynapps", "astound"];

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../common/fixtures/catalog")
}

fn root_config(ai: &Path) -> String {
    let mut includes = String::new();
    for file in ["config.yaml", "providers.yaml", "gateway.yaml"] {
        if ai.join(file).exists() {
            includes.push_str(&format!("  - ../ai/{file}\n"));
        }
    }
    format!(
        "includes:\n{includes}agents: {{}}\nmcp_servers: {{}}\nsettings:\n  agent_port_range: [4000, 4999]\n  mcp_port_range: [5000, 5999]\n"
    )
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create dir");
    for entry in std::fs::read_dir(from).expect("read fixture dir") {
        let entry = entry.expect("dir entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy fixture file");
        }
    }
}

fn record(fixture: &str) -> Value {
    let legacy_ai = fixtures_root().join(fixture).join("legacy/ai");
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let services = tmp.path().join("services");
    copy_dir(&legacy_ai, &services.join("ai"));
    std::fs::create_dir_all(services.join("config")).expect("config dir");
    let config_path = services.join("config/config.yaml");
    std::fs::write(&config_path, root_config(&legacy_ai)).expect("write root config");

    let config = ConfigLoader::load_from_path(&config_path)
        .unwrap_or_else(|e| panic!("{fixture}: loader refused the tree: {e}"));
    let gateway = config.gateway_config().filter(|g| g.enabled);
    let mut models = Map::new();
    for id in walk::requested_ids(&config.providers, gateway) {
        let resolved = walk::resolve_id(&config.providers, gateway, &id);
        models.insert(id, resolved);
    }
    canonical(json!({
        "core_version": "0.72.1",
        "fixture": fixture,
        "gateway_enabled": gateway.is_some(),
        "default_model": gateway.and_then(|g| g.default_model.clone()),
        "models": models,
    }))
}

fn canonical(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k, canonical(v)))
                    .collect(),
            )
        },
        Value::Array(items) => Value::Array(items.into_iter().map(canonical).collect()),
        other => other,
    }
}

#[test]
#[ignore = "recorder: writes the golden catalog snapshots"]
fn record_golden_catalog() {
    let out_dir = std::env::var_os("GOLDEN_OUT").map(PathBuf::from);
    for fixture in FIXTURES {
        let snapshot = record(fixture);
        let mut text = serde_json::to_string_pretty(&snapshot).expect("serialise snapshot");
        text.push('\n');
        let path = out_dir.as_ref().map_or_else(
            || fixtures_root().join(fixture).join("golden/resolution.json"),
            |dir| dir.join(format!("{fixture}.json")),
        );
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create output dir");
        }
        std::fs::write(&path, text).expect("write snapshot");
    }
}
