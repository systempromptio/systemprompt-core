use std::fs;

use serde_json::{Value, json};
use sp_projection::{Applier, BundleFiles};

use crate::fixtures::{self, projected};

fn settings_after(applier: &Applier, defaults: &Value) -> Value {
    let manifest = fixtures::manifest(json!({
        "marketplaces": [{ "id": "m", "harness": { "defaults": defaults } }]
    }));
    let report = applier
        .apply(&projected(&manifest, &BundleFiles::new()))
        .unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let text = fs::read_to_string(applier.config_dir().join("harness.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn marketplace_defaults_fill_only_unset_keys_and_stay_after_clear() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    fs::create_dir_all(applier.config_dir()).unwrap();
    fs::write(
        applier.config_dir().join("harness.json"),
        "{\n  \"model\": \"mine/model\"\n}\n",
    )
    .unwrap();
    let written = settings_after(
        &applier,
        &json!({ "model": "systemprompt/x", "small_model": "systemprompt/s" }),
    );
    assert_eq!(
        written,
        json!({ "model": "mine/model", "small_model": "systemprompt/s" })
    );
    applier.clear().unwrap();
    let text = fs::read_to_string(applier.config_dir().join("harness.json")).unwrap();
    let after: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(after, written);
}

#[test]
fn the_default_model_is_written_as_the_bare_id() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let written = settings_after(
        &applier,
        &json!({ "model": "systemprompt/claude-sonnet-5-5" }),
    );
    assert_eq!(written, json!({ "model": "claude-sonnet-5-5" }));
}

#[test]
fn a_bare_default_model_is_written_as_given() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let written = settings_after(&applier, &json!({ "model": "claude-opus-5-5" }));
    assert_eq!(written, json!({ "model": "claude-opus-5-5" }));
}

#[test]
fn a_settings_file_that_is_not_an_object_leaves_the_defaults_unapplied() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    fs::create_dir_all(applier.config_dir()).unwrap();
    let file = applier.config_dir().join("harness.json");
    fs::write(&file, "[1, 2]").unwrap();
    let manifest = fixtures::manifest(json!({
        "marketplaces": [{ "id": "m", "harness": { "defaults": { "model": "claude-opus-5-5" } } }]
    }));
    let report = applier
        .apply(&projected(&manifest, &BundleFiles::new()))
        .unwrap();
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(fs::read_to_string(&file).unwrap(), "[1, 2]");
}
