//! Unit tests for `merge_json_data`, the deep-merge helper used to combine
//! template-data provider outputs.

use systemprompt_generator::merge_json_data;

#[test]
fn merges_disjoint_keys() {
    let mut base = serde_json::json!({"a": 1});
    merge_json_data(&mut base, &serde_json::json!({"b": 2}));
    assert_eq!(base, serde_json::json!({"a": 1, "b": 2}));
}

#[test]
fn nested_objects_merge_recursively() {
    let mut base = serde_json::json!({"outer": {"a": 1}});
    merge_json_data(&mut base, &serde_json::json!({"outer": {"b": 2}}));
    assert_eq!(base, serde_json::json!({"outer": {"a": 1, "b": 2}}));
}

#[test]
fn scalar_extension_overwrites_base() {
    let mut base = serde_json::json!({"a": 1});
    merge_json_data(&mut base, &serde_json::json!({"a": 99}));
    assert_eq!(base, serde_json::json!({"a": 99}));
}

#[test]
fn object_replaces_scalar_at_key() {
    let mut base = serde_json::json!({"a": 1});
    merge_json_data(&mut base, &serde_json::json!({"a": {"nested": true}}));
    assert_eq!(base, serde_json::json!({"a": {"nested": true}}));
}

#[test]
fn non_object_root_is_replaced_wholesale() {
    let mut base = serde_json::json!("original");
    merge_json_data(&mut base, &serde_json::json!({"a": 1}));
    assert_eq!(base, serde_json::json!({"a": 1}));
}

#[test]
fn deeply_nested_merge_preserves_existing() {
    let mut base = serde_json::json!({"l1": {"l2": {"keep": 1}}});
    merge_json_data(&mut base, &serde_json::json!({"l1": {"l2": {"add": 2}}}));
    assert_eq!(
        base,
        serde_json::json!({"l1": {"l2": {"keep": 1, "add": 2}}})
    );
}
