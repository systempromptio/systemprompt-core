use serde_json::json;
use systemprompt_bridge::gateway::types::ProviderHealth;
use systemprompt_bridge::install::mdm::claude_code_settings::model_picker::{
    PickerRow, is_claude_family, merged_picker, picker_rows,
};
use systemprompt_models::services::ApiSurface;

fn rows() -> Vec<PickerRow> {
    vec![PickerRow {
        id: "gemini-2.5-flash".into(),
        label: "Gemini Flash".into(),
    }]
}

fn provider(configured: bool, models: &[&str]) -> ProviderHealth {
    ProviderHealth {
        name: "fixture".to_owned(),
        surface: ApiSurface::OpenAi,
        configured,
        models: models.iter().map(|model| (*model).to_owned()).collect(),
        config_issue: (!configured).then(|| "credential absent".to_owned()),
    }
}

#[test]
fn picker_rows_excludes_claude_models_and_unconfigured_providers() {
    let rows = picker_rows(&[
        provider(true, &["claude-sonnet-4-6", "gemini-2.5-flash", "gpt-5"]),
        provider(false, &["vertex-gemini", "custom-model"]),
        provider(true, &["ANTHROPIC-compat", "gemini-2.5-flash"]),
    ]);

    assert_eq!(
        rows,
        vec![
            PickerRow {
                id: "gemini-2.5-flash".into(),
                label: "Gemini 2.5 Flash".into(),
            },
            PickerRow {
                id: "gpt-5".into(),
                label: "Gpt 5".into(),
            },
        ]
    );
}

#[test]
fn claude_family_check_is_case_insensitive_and_requires_a_family_marker() {
    assert!(is_claude_family("Claude-4"));
    assert!(is_claude_family("vendor-anthropic-compatible"));
    assert!(!is_claude_family("gemini-2.5-pro"));
}

#[test]
fn emits_options_objects_with_model_keys_not_the_ignored_legacy_array() {
    assert_eq!(
        merged_picker(None, &[], &rows()),
        Some(json!({
            "options": [{"model": "gemini-2.5-flash", "label": "Gemini Flash"}]
        }))
    );
}

#[test]
fn preserves_user_option_metadata_and_picker_settings() {
    let old = json!({"customSetting": true, "options": [
        {"model": "personal", "label": "Mine", "description": "Keep", "behavesAs": "sonnet"}
    ]});
    let merged = merged_picker(Some(&old), &[], &rows()).unwrap();
    assert_eq!(merged["customSetting"], true);
    assert_eq!(merged["options"][0], old["options"][0]);
}

#[test]
fn repeat_sync_is_idempotent() {
    let first = merged_picker(None, &[], &rows()).unwrap();
    assert_eq!(
        merged_picker(Some(&first), &["gemini-2.5-flash".into()], &rows()),
        Some(first)
    );
}

#[test]
fn uninstall_removes_only_recorded_bridge_rows() {
    let old = json!({"options": [{"model": "personal"}, {"model": "gemini-2.5-flash"}]});
    assert_eq!(
        merged_picker(Some(&old), &["gemini-2.5-flash".into()], &[]),
        Some(json!({
            "options": [{"model": "personal"}]
        }))
    );
    assert_eq!(
        merged_picker(
            Some(&json!({"options": [{"model": "gemini-2.5-flash"}]})),
            &["gemini-2.5-flash".into()],
            &[]
        ),
        None
    );
}

#[test]
fn replacing_owned_rows_preserves_unknown_and_malformed_user_options() {
    let existing = json!({
        "title": "My models",
        "options": [
            {"model": "old-gateway-model", "label": "Old"},
            {"label": "A user row without a model key"},
            "a user extension value"
        ]
    });
    let merged = merged_picker(
        Some(&existing),
        &["old-gateway-model".to_owned()],
        &[PickerRow {
            id: "new-gateway-model".into(),
            label: "New Gateway Model".into(),
        }],
    )
    .unwrap();

    assert_eq!(merged["title"], "My models");
    assert_eq!(merged["options"][0], existing["options"][1]);
    assert_eq!(merged["options"][1], existing["options"][2]);
    assert_eq!(merged["options"][2]["model"], "new-gateway-model");
}
