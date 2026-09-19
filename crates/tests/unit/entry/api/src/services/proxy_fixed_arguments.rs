//! The external proxy writes instance-fixed arguments into every tools/call.

use std::collections::HashMap;

use serde_json::json;
use systemprompt_api::services::proxy::engine::fixed_arguments::apply;
use systemprompt_models::mcp::deployment::ToolMetadata;

fn tools() -> HashMap<String, ToolMetadata> {
    let mut arguments = serde_json::Map::new();
    arguments.insert("servingConfig".to_owned(), json!("projects/1/servingConfigs/default"));
    HashMap::from([(
        "search".to_owned(),
        ToolMetadata {
            arguments,
            ..ToolMetadata::default()
        },
    )])
}

fn call(name: &str, arguments: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    }))
    .expect("serialise")
}

fn arguments_of(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice::<serde_json::Value>(body).expect("json")["params"]["arguments"].clone()
}

#[test]
fn fixed_argument_replaces_whatever_the_client_sent() {
    let mut body = call("search", json!({"servingConfig": "projects/*/x", "query": "q"}));
    apply(&tools(), &mut body);
    assert_eq!(
        arguments_of(&body),
        json!({"servingConfig": "projects/1/servingConfigs/default", "query": "q"})
    );
}

#[test]
fn fixed_argument_is_added_when_the_client_omits_it() {
    let mut body = call("search", json!({"query": "q"}));
    apply(&tools(), &mut body);
    assert_eq!(arguments_of(&body)["servingConfig"], json!("projects/1/servingConfigs/default"));
}

#[test]
fn other_tools_and_methods_pass_through_unchanged() {
    let original = call("list_engines", json!({"parent": "projects/*"}));
    let mut body = original.clone();
    apply(&tools(), &mut body);
    assert_eq!(body, original);

    let list = br#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#.to_vec();
    let mut body = list.clone();
    apply(&tools(), &mut body);
    assert_eq!(body, list);

    let mut garbage = b"not json".to_vec();
    apply(&tools(), &mut garbage);
    assert_eq!(garbage, b"not json".to_vec());
}

#[test]
fn servers_without_fixed_arguments_leave_the_body_alone() {
    let original = call("search", json!({"servingConfig": "projects/*/x"}));
    let mut body = original.clone();
    apply(&HashMap::new(), &mut body);
    assert_eq!(body, original);
}
