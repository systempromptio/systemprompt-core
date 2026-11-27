use serde_json::{Value, json};
use systemprompt_engine::llm::{Message, Protocol, Request, prepare};

use crate::llm::support::{endpoint, model};

#[test]
fn messages_request_gets_its_path_headers_and_merged_body() {
    let mut request = Request::new(model(Protocol::Messages));
    request.messages = vec![Message::user_text("hi")];
    request.metadata.insert("trace".into(), json!("t"));
    let prepared = prepare(&endpoint("https://gw.example/v1/"), &request).unwrap();
    assert_eq!(prepared.url.as_str(), "https://gw.example/v1/messages");
    let has = |name: &str, value: &str| {
        prepared
            .headers
            .iter()
            .any(|(k, v)| k == name && v == value)
    };
    assert!(has("anthropic-version", "2023-06-01"));
    assert!(has("accept", "text/event-stream"));
    assert!(has("content-type", "application/json"));
    assert!(prepared.headers.iter().any(|(k, _)| k == "authorization"));
    let body: Value = serde_json::from_slice(&prepared.body).unwrap();
    assert_eq!(
        body["metadata"],
        json!({"trace": "t", "user_id": "{\"session_id\":\"s\"}"})
    );
    assert!(!format!("{prepared:?}").contains("secret"));
}

#[test]
fn chat_request_gets_its_path_and_no_messages_version() {
    let request = Request::new(model(Protocol::ChatCompletions));
    let prepared = prepare(&endpoint("https://gw.example/v1"), &request).unwrap();
    assert_eq!(
        prepared.url.as_str(),
        "https://gw.example/v1/chat/completions"
    );
    assert!(
        !prepared
            .headers
            .iter()
            .any(|(k, _)| k == "anthropic-version")
    );
}

#[test]
fn endpoint_headers_replace_protocol_defaults() {
    let mut endpoint = endpoint("https://gw.example/v1");
    endpoint
        .headers
        .push(("Accept".into(), "application/json".into()));
    let request = Request::new(model(Protocol::Messages));
    let prepared = prepare(&endpoint, &request).unwrap();
    let accepts: Vec<&str> = prepared
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("accept"))
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(accepts, vec!["application/json"]);
}
