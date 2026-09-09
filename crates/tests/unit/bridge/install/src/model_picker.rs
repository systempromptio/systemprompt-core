use serde_json::json;
use systemprompt_bridge::install::mdm::claude_code_settings::model_picker::{
    PickerRow, merged_picker,
};

fn rows() -> Vec<PickerRow> {
    vec![PickerRow {
        id: "gemini-2.5-flash".into(),
        label: "Gemini Flash".into(),
    }]
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
fn migrates_legacy_arrays_and_preserves_unrelated_rows() {
    let old = json!([
        {"id": "old-gemini", "label": "Old"},
        {"id": "personal-model", "label": "Mine", "description": "Keep"}
    ]);
    let merged = merged_picker(Some(&old), &["old-gemini".into()], &rows()).unwrap();
    assert_eq!(
        merged["options"][0],
        json!({
            "model": "personal-model", "label": "Mine", "description": "Keep"
        })
    );
    assert_eq!(merged["options"][1]["model"], "gemini-2.5-flash");
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
