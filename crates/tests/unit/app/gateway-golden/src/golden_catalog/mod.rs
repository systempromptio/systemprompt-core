//! Golden catalog recorder: reads a fixture's `services/ai` directory into a
//! merged `CatalogDocument`, resolves it into a `ResolvedCatalog` and writes
//! what every model id, alias and the default model resolve to, in exactly the
//! schema `crates/tests/common/fixtures/catalog/README.md` describes, so the
//! 0.73 resolution diffs against the committed 0.72.1 `golden/` snapshots.
//! No secret store is involved: credentials appear by id and secret name.
//!
//! Run: `cargo nextest run -p systemprompt-gateway-golden-tests
//! --run-ignored only`. `GOLDEN_INPUT=<dir>` picks the fixture subdirectory
//! holding `ai/` (default `legacy`, which a 0.73 binary refuses with the
//! `config migrate` hint); `GOLDEN_OUT=<dir>` writes `<dir>/<fixture>.json`,
//! else `<fixture>/resolved/resolution.json`. The committed `golden/` files
//! are never written.

mod walk;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use systemprompt_catalog_fixtures::read_ai_dir;
use systemprompt_manifest::services::catalog::CatalogContext;

const FIXTURES: [&str; 3] = ["core-default", "dynapps", "astound"];

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../common/fixtures/catalog")
}

fn input_dir() -> String {
    std::env::var("GOLDEN_INPUT").unwrap_or_else(|_| "legacy".to_owned())
}

fn record(fixture: &str) -> Result<Value, String> {
    let ai = fixtures_root().join(fixture).join(input_dir()).join("ai");
    let document = read_ai_dir(&ai).map_err(|e| format!("{fixture}: {e}"))?;
    let catalog = document
        .resolve(&CatalogContext::default())
        .map_err(|e| format!("{fixture}: {e}"))?;
    let mut models = Map::new();
    for id in walk::requested_ids(&catalog) {
        let resolved = walk::resolve_id(&catalog, &id);
        models.insert(id, resolved);
    }
    Ok(canonical(json!({
        "core_version": "0.73.0",
        "fixture": fixture,
        "gateway_enabled": catalog.gateway_enabled(),
        "default_model": catalog.default_model().map(|m| m.as_str().to_owned()),
        "models": models,
    })))
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
#[ignore = "recorder: writes catalog resolution snapshots"]
fn record_golden_catalog() {
    let out_dir = std::env::var_os("GOLDEN_OUT").map(PathBuf::from);
    let mut failures = Vec::new();
    for fixture in FIXTURES {
        let snapshot = match record(fixture) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                failures.push(error);
                continue;
            },
        };
        let mut text = serde_json::to_string_pretty(&snapshot).expect("serialise snapshot");
        text.push('\n');
        let path = out_dir.as_ref().map_or_else(
            || {
                fixtures_root()
                    .join(fixture)
                    .join("resolved/resolution.json")
            },
            |dir| dir.join(format!("{fixture}.json")),
        );
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create output dir");
        }
        std::fs::write(&path, text).expect("write snapshot");
    }
    assert!(
        failures.is_empty(),
        "fixtures did not resolve:\n{}",
        failures.join("\n")
    );
}
