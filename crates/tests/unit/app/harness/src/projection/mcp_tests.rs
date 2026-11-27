use serde_json::{Value, json};
use systemprompt_harness::projection::{BundleFiles, ManagedMcp, plan};

use crate::projection::fixtures;

fn servers(list: &Value, instance: &str) -> (Vec<ManagedMcp>, Vec<String>) {
    let manifest = fixtures::manifest(json!({ "managed_mcp_servers": list }));
    let files = BundleFiles::new();
    let key = fixtures::key();
    let mut input = fixtures::input(&manifest, &files, &key);
    input.instance_url = instance;
    let projection = plan(&input);
    (projection.mcp, projection.warnings)
}

#[test]
fn the_bearer_only_goes_to_the_instance_origin() {
    let (out, warnings) = servers(
        &json!([
            { "name": "DynBrain Docs", "url": "https://sp.example.com/api/v1/mcp/docs/mcp",
              "headers": { "Authorization": "Bearer stolen", "x-team": "a" } },
            { "id": "other", "name": "other", "url": "https://tools.example.org/mcp",
              "headers": { "x-key": "k" } },
            { "id": "plain", "name": "plain", "url": "http://tools.example.org/mcp" }
        ]),
        "https://sp.example.com",
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].name, "dynbrain-docs");
    assert!(out[0].bearer);
    assert_eq!(out[0].headers.keys().collect::<Vec<_>>(), ["x-team"]);
    assert_eq!(out[1].name, "other");
    assert!(!out[1].bearer);
    assert_eq!(warnings.len(), 1);
    assert!(!format!("{:?}", out[1]).contains("\"k\""));
}

#[test]
fn a_local_instance_reaches_its_own_servers_at_its_origin() {
    let (out, _) = servers(
        &json!([{ "name": "docs", "url": "http://localhost:8080/api/v1/mcp/docs/mcp" }]),
        "http://127.0.0.1:18080",
    );
    assert_eq!(out[0].url, "http://127.0.0.1:18080/api/v1/mcp/docs/mcp");
    assert!(out[0].bearer);
}

#[test]
fn non_plain_ids_are_dropped_and_a_shared_slug_keeps_the_last() {
    let (out, warnings) = servers(
        &json!([
            { "id": "bad id", "name": "x", "url": "https://sp.example.com/a" },
            { "id": "one", "name": "Docs", "url": "https://sp.example.com/one" },
            { "id": "two", "name": "docs", "url": "https://sp.example.com/two" }
        ]),
        "https://sp.example.com",
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].id, "two");
    assert_eq!(warnings.len(), 1);
}

#[test]
fn an_unusable_instance_url_marks_no_server_for_the_bearer() {
    let (out, _) = servers(
        &json!([{ "name": "docs", "url": "https://sp.example.com/mcp" }]),
        "http://sp.example.com",
    );
    assert_eq!(out.len(), 1);
    assert!(!out[0].bearer);
}
