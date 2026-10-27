use std::collections::BTreeMap;

use systemprompt_bridge::drift::diff;
use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::integration::claude_desktop::plist_value_texts;
use systemprompt_bridge::integration::claude_desktop::reg_profile::{desired, profile_entries};
use systemprompt_bridge::integration::host_app::ProfileGenInputs;
use systemprompt_models::bridge::profile::AdvertisedLimits;

const API_KEY: &str = "inferenceGatewayApiKey";

fn inputs() -> ProfileGenInputs {
    ProfileGenInputs {
        model_limits: [(
            "claude-opus-4-7".to_owned(),
            AdvertisedLimits {
                context_window: 1_000_000,
                max_output_tokens: 64_000,
            },
        )]
        .into_iter()
        .collect(),
        model_names: Default::default(),
        gateway_base_url: "https://gateway.example.com".to_string(),
        host_token: HostToken::new("sp-secret-key"),
        models: vec![
            "claude-opus-4-7".to_string(),
            "claude-sonnet-4-6".to_string(),
        ],
        default_model: None,
        organization_uuid: None,
        headers: Default::default(),
        mcp_servers: Some(Vec::new()),
    }
}

fn as_plist_value(text: &str) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v @ (serde_json::Value::Array(_) | serde_json::Value::Object(_))) => v,
        _ => serde_json::Value::String(text.to_owned()),
    }
}

fn observed_from_plist(plist: &BTreeMap<String, serde_json::Value>) -> BTreeMap<String, String> {
    plist
        .iter()
        .map(|(k, v)| (k.clone(), plist_value_texts(v).compare))
        .collect()
}

fn written_plist() -> BTreeMap<String, serde_json::Value> {
    profile_entries(&inputs())
        .expect("profile renders")
        .into_iter()
        .filter(|(k, _)| *k != API_KEY)
        .map(|(k, v)| (k.to_owned(), as_plist_value(&v)))
        .collect()
}

#[test]
fn a_plist_holding_what_the_writer_wrote_reads_back_without_drift() {
    let desired = desired(&inputs()).expect("desired renders");
    let drift = diff(&desired, &observed_from_plist(&written_plist()));
    assert!(
        drift.differing.is_empty(),
        "string lists read from a plist must compare as the JSON the writer rendered: {drift:?}"
    );
    assert_eq!(drift.missing, vec![API_KEY.to_owned()]);
}

#[test]
fn a_plist_with_a_different_model_list_still_drifts() {
    let desired = desired(&inputs()).expect("desired renders");
    let mut plist = written_plist();
    plist.insert(
        "inferenceModels".to_owned(),
        serde_json::json!(["claude-haiku-4-5"]),
    );
    let drift = diff(&desired, &observed_from_plist(&plist));
    assert_eq!(drift.differing, vec!["inferenceModels".to_owned()]);
}

#[test]
fn a_string_list_displays_joined_but_compares_as_json() {
    let texts = plist_value_texts(&serde_json::json!(["a[1m]", "b"]));
    assert_eq!(texts.display, "a[1m], b");
    assert_eq!(texts.compare, r#"["a[1m]","b"]"#);
    let empty = plist_value_texts(&serde_json::json!([]));
    assert_eq!(empty.display, "");
    assert_eq!(empty.compare, "[]");
}
