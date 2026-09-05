//! The OS-independent half of the managed-policy store: the plist renderer,
//! the `PolicyDocumentValue` JSON round-trip, and the no-op backend Linux
//! resolves to.

use std::collections::BTreeMap;

use systemprompt_bridge::config::store::document::PolicyDocumentValue;
use systemprompt_bridge::config::store::plist::render_plist;
use systemprompt_bridge::config::store::{
    PolicyDocument, PolicyHive, bridge_policy_domain, managed_policy_store,
};

fn doc(entries: Vec<(&str, PolicyDocumentValue)>) -> PolicyDocument {
    entries
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect()
}

#[test]
fn an_empty_document_renders_a_well_formed_but_empty_plist() {
    let rendered = render_plist(&PolicyDocument::new());
    assert!(rendered.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    assert!(rendered.contains("<plist version=\"1.0\">\n<dict>\n"));
    assert!(rendered.ends_with("</dict>\n</plist>\n"));
    assert!(!rendered.contains("<key>"));
}

#[test]
fn each_value_kind_renders_its_own_plist_element() {
    let rendered = render_plist(&doc(vec![
        ("aString", PolicyDocumentValue::Str("hello".to_owned())),
        ("aTrue", PolicyDocumentValue::Bool(true)),
        ("aFalse", PolicyDocumentValue::Bool(false)),
        (
            "aList",
            PolicyDocumentValue::StrList(vec!["one".to_owned(), "two".to_owned()]),
        ),
    ]));

    assert!(rendered.contains("  <key>aString</key>\n  <string>hello</string>\n"));
    assert!(rendered.contains("  <key>aTrue</key>\n  <true/>\n"));
    assert!(rendered.contains("  <key>aFalse</key>\n  <false/>\n"));
    assert!(rendered.contains(
        "  <key>aList</key>\n  <array>\n    <string>one</string>\n    <string>two</string>\n  </array>\n"
    ));
}

#[test]
fn keys_render_in_sorted_order_so_the_written_plist_is_stable() {
    let rendered = render_plist(&doc(vec![
        ("zebra", PolicyDocumentValue::Bool(true)),
        ("alpha", PolicyDocumentValue::Bool(true)),
    ]));
    let alpha = rendered.find("alpha").expect("alpha present");
    let zebra = rendered.find("zebra").expect("zebra present");
    assert!(alpha < zebra, "BTreeMap ordering should survive rendering");
}

#[test]
fn a_dict_array_nests_and_indents_its_inner_entries() {
    let mut inner = BTreeMap::new();
    inner.insert(
        "command".to_owned(),
        PolicyDocumentValue::Str("bridge".to_owned()),
    );
    let rendered = render_plist(&doc(vec![(
        "mcpServers",
        PolicyDocumentValue::Dicts(vec![inner]),
    )]));

    assert!(rendered.contains("  <key>mcpServers</key>\n  <array>\n    <dict>\n"));
    assert!(rendered.contains("      <key>command</key>\n      <string>bridge</string>\n"));
    assert!(rendered.contains("    </dict>\n  </array>\n"));
}

#[test]
fn xml_metacharacters_are_escaped_in_both_keys_and_values() {
    let rendered = render_plist(&doc(vec![(
        "a<&>\"key",
        PolicyDocumentValue::StrList(vec!["<val & \"more\">".to_owned()]),
    )]));

    assert!(rendered.contains("<key>a&lt;&amp;&gt;&quot;key</key>"));
    assert!(rendered.contains("<string>&lt;val &amp; &quot;more&quot;&gt;</string>"));
    assert!(!rendered.contains("a<&>"));
}

#[test]
fn as_str_answers_only_for_the_string_variant() {
    assert_eq!(
        PolicyDocumentValue::Str("v".to_owned()).as_str(),
        Some("v")
    );
    assert_eq!(PolicyDocumentValue::Bool(true).as_str(), None);
    assert_eq!(PolicyDocumentValue::StrList(vec![]).as_str(), None);
    assert_eq!(PolicyDocumentValue::Dicts(vec![]).as_str(), None);
}

#[test]
fn json_strings_bools_and_string_arrays_convert_to_their_matching_variant() {
    assert_eq!(
        PolicyDocumentValue::from_json(&serde_json::json!("hi")),
        Some(PolicyDocumentValue::Str("hi".to_owned()))
    );
    assert_eq!(
        PolicyDocumentValue::from_json(&serde_json::json!(false)),
        Some(PolicyDocumentValue::Bool(false))
    );
    assert_eq!(
        PolicyDocumentValue::from_json(&serde_json::json!(["a", "b"])),
        Some(PolicyDocumentValue::StrList(vec![
            "a".to_owned(),
            "b".to_owned()
        ]))
    );
}

#[test]
fn a_json_array_of_objects_becomes_a_dict_array_and_a_bare_object_becomes_a_one_entry_one() {
    let from_array = PolicyDocumentValue::from_json(&serde_json::json!([{"k": "v"}]))
        .expect("array of objects converts");
    let from_object =
        PolicyDocumentValue::from_json(&serde_json::json!({"k": "v"})).expect("object converts");
    assert_eq!(from_array, from_object);

    let PolicyDocumentValue::Dicts(dicts) = from_array else {
        panic!("expected a dict array");
    };
    assert_eq!(dicts.len(), 1);
    assert_eq!(dicts[0].get("k"), Some(&PolicyDocumentValue::Str("v".to_owned())));
}

#[test]
fn json_nulls_and_numbers_have_no_policy_representation() {
    assert_eq!(PolicyDocumentValue::from_json(&serde_json::json!(null)), None);
    assert_eq!(PolicyDocumentValue::from_json(&serde_json::json!(7)), None);
}

#[test]
fn a_number_nested_anywhere_rejects_the_whole_value() {
    assert_eq!(
        PolicyDocumentValue::from_json(&serde_json::json!({"outer": {"inner": 1}})),
        None
    );
    assert_eq!(
        PolicyDocumentValue::from_json(&serde_json::json!([{"k": 1}])),
        None
    );
}

#[test]
fn a_mixed_array_that_is_not_all_objects_rejects_rather_than_partially_converting() {
    assert_eq!(
        PolicyDocumentValue::from_json(&serde_json::json!(["a", {"k": "v"}])),
        None
    );
}

#[test]
fn a_rendered_document_survives_the_json_round_trip_plutil_performs() {
    let original = PolicyDocumentValue::from_json(&serde_json::json!({
        "servers": [{"command": "bridge", "enabled": true, "args": ["serve"]}]
    }))
    .expect("converts");
    let json = serde_json::to_value(&original).expect("serialize");
    let back: PolicyDocumentValue = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, original);
}

#[test]
fn each_hive_labels_itself_with_the_windows_root_it_maps_to() {
    assert_eq!(PolicyHive::Machine.label(), "HKLM");
    assert_eq!(PolicyHive::User.label(), "HKCU");
}

#[test]
fn the_bridge_policy_domain_is_reverse_dns_under_the_brand_config_dir() {
    let domain = bridge_policy_domain();
    assert!(
        domain.starts_with("io.systemprompt."),
        "expected a reverse-dns domain, got {domain}"
    );
    assert!(domain.len() > "io.systemprompt.".len());
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[test]
fn on_a_host_with_no_managed_policy_backend_every_read_is_empty_and_every_write_is_a_no_op() {
    let store = managed_policy_store();

    assert_eq!(store.read_managed_policy("anything").expect("read"), None);

    let read = store.read_managed_policy_keys(&["a", "b"]).expect("read");
    assert!(read.source.is_none());
    assert!(read.values.is_empty());

    assert!(
        store
            .read_policy_document(PolicyHive::Machine, &["a"])
            .expect("read")
            .is_empty()
    );
    assert!(
        store
            .write_policy_values(
                PolicyHive::User,
                &[("k".to_owned(), PolicyDocumentValue::Str("v".to_owned()))]
            )
            .is_ok()
    );
    assert_eq!(
        store
            .delete_policy_values(PolicyHive::User, &["k"])
            .expect("delete"),
        0
    );
    assert!(!store.delete_policy_key(PolicyHive::Machine).expect("delete"));
}
