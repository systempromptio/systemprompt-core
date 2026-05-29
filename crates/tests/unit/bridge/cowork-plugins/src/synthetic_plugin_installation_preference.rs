// `plugin.json::installationPreference` is the field that decides whether
// Cowork (under MDM + custom inference gateway) auto-installs the bridge's
// synthetic plugin or surfaces the "Contact an organization owner to install
// connectors" tooltip and locks the user out.
//
// Documented at https://claude.com/docs/cowork/3p/extensions :
//     "required"     — installs automatically; uninstall hidden
//     "auto_install" — installs automatically; users can uninstall
//     "available"    — manual installation via plugin browser (default)
//
// These tests pin the wire shape against drift.

use serde_json::Value;
use systemprompt_bridge::sync::{PLUGIN_INSTALLATION_PREFERENCE, render_plugin_json};

fn render_plugin_json_test(version: &str) -> Vec<u8> {
    render_plugin_json(version).expect("render plugin.json")
}

#[test]
fn constant_is_one_of_documented_auto_install_values() {
    // The bridge's emitted value must always be one of the two auto-install
    // variants. `available` is the default Cowork applies when the field is
    // absent — never a value we want to emit.
    assert!(
        matches!(PLUGIN_INSTALLATION_PREFERENCE, "required" | "auto_install"),
        "PLUGIN_INSTALLATION_PREFERENCE={PLUGIN_INSTALLATION_PREFERENCE:?} is not a \
         Cowork-auto-install value"
    );
}

#[test]
fn plugin_json_has_installation_preference_field() {
    let bytes = render_plugin_json_test("2026-01-01T00:00:00Z-deadbeef");
    let v: Value = serde_json::from_slice(&bytes).expect("plugin.json must be valid JSON");
    let pref = v
        .get("installationPreference")
        .and_then(Value::as_str)
        .expect("`installationPreference` key must be present");
    assert_eq!(pref, PLUGIN_INSTALLATION_PREFERENCE);
}

#[test]
#[allow(
    non_snake_case,
    reason = "test name mirrors the camelCase JSON field under assertion"
)]
fn plugin_json_field_name_is_camelCase() {
    // Cowork reads `installationPreference` (camelCase). If a future refactor
    // renames the serde rename and emits `installation_preference` (snake_case),
    // Cowork silently defaults to `"available"` and the install button locks.
    let bytes = render_plugin_json_test("v1");
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(
        text.contains("\"installationPreference\""),
        "expected camelCase key in JSON output, got: {text}"
    );
    assert!(
        !text.contains("\"installation_preference\""),
        "snake_case key would be invisible to Cowork"
    );
}

#[test]
fn plugin_json_preserves_other_required_fields() {
    let bytes = render_plugin_json_test("v1.2.3");
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    // Adding the new field must not have removed any of the previously
    // required keys.
    assert!(v.get("name").is_some(), "name field missing");
    assert!(v.get("description").is_some(), "description field missing");
    assert_eq!(v["version"], "v1.2.3");
}

#[test]
fn render_is_deterministic() {
    let a = render_plugin_json_test("v1");
    let b = render_plugin_json_test("v1");
    assert_eq!(a, b);
}
