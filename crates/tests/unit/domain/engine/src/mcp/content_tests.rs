use std::collections::BTreeMap;

use serde_json::{Value, json};
use systemprompt_engine::mcp::{Attachment, RequestOptions, ToolInfo};

use crate::mcp::support::stream::{Handler, connected, reply, standard};

fn answering(method: &'static str, result: Value) -> Handler {
    Box::new(move |message: &Value| {
        if message["method"] == method {
            return vec![reply(message, &result.clone())];
        }
        standard(message)
    })
}

#[tokio::test]
async fn a_call_result_keeps_text_and_images() {
    let result = json!({
        "content": [
            {"type": "text", "text": "first"},
            {"type": "image", "data": "aGk=", "mimeType": "image/png"},
            {"type": "resource", "resource": {"uri": "file:///a.txt", "text": "body"}},
            {"type": "resource_link", "uri": "file:///b", "name": "b"},
            {"type": "audio", "data": "AA==", "mimeType": "audio/wav"},
        ],
        "isError": true,
    });
    let (client, _seen) = connected(answering("tools/call", result)).await;
    let result = client
        .call_tool("any", json!({}), RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(
        result.text,
        "first\nbody\n[resource b: file:///b]\n[audio audio/wav]"
    );
    assert!(result.is_error);
    let image = Attachment {
        mime_type: "image/png".into(),
        data: "aGk=".into(),
    };
    assert_eq!(result.attachments, vec![image]);
}

#[tokio::test]
async fn structured_content_fills_an_empty_result() {
    let result = json!({"content": [], "structuredContent": {"n": 1}});
    let (client, _seen) = connected(answering("tools/call", result)).await;
    let result = client
        .call_tool("any", json!({}), RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(result.text, r#"{"n":1}"#);
    assert_eq!(result.structured, Some(json!({"n": 1})));
    assert!(!result.is_error);
}

#[test]
fn tool_info_defaults_its_schema() {
    let tool: ToolInfo = serde_json::from_value(json!({"name": "echo"})).unwrap();
    assert_eq!(
        tool.input_schema,
        json!({"type": "object", "properties": {}})
    );
    assert_eq!(tool.description, None);
}

#[tokio::test]
async fn resources_are_listed_and_read() {
    let handler: Handler = Box::new(|message: &Value| match message["method"].as_str() {
        Some("resources/list") => vec![reply(
            message,
            &json!({"resources": [{"uri": "file:///a", "name": "a", "mimeType": "text/plain"}, 7]}),
        )],
        Some("resources/read") => vec![reply(
            message,
            &json!({"contents": [{"uri": message["params"]["uri"].clone(), "text": "hello"}]}),
        )],
        _ => standard(message),
    });
    let (client, _seen) = connected(handler).await;
    let listed = client
        .list_resources(RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].mime_type.as_deref(), Some("text/plain"));
    let read = client
        .read_resource("file:///a", RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(read[0].uri, "file:///a");
    assert_eq!(read[0].text.as_deref(), Some("hello"));
}

#[tokio::test]
async fn a_prompt_is_rendered_with_its_arguments() {
    let handler: Handler = Box::new(|message: &Value| {
        if message["method"] == "prompts/get" {
            let topic = message["params"]["arguments"]["topic"]
                .as_str()
                .unwrap_or_default();
            let text = format!("write about {topic}");
            return vec![reply(
                message,
                &json!({
                    "description": "a writing prompt",
                    "messages": [
                        {"role": "assistant", "content": {"type": "text", "text": text}},
                        {"content": {"type": "image", "data": "aGk=", "mimeType": "image/png"}},
                    ],
                }),
            )];
        }
        standard(message)
    });
    let (client, _seen) = connected(handler).await;
    let arguments = BTreeMap::from([("topic".to_owned(), "tides".to_owned())]);
    let prompt = client
        .get_prompt("essay", &arguments, RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(prompt.description.as_deref(), Some("a writing prompt"));
    assert_eq!(prompt.messages[0].role, "assistant");
    assert_eq!(prompt.messages[0].text, "write about tides");
    assert_eq!(prompt.messages[1].role, "user");
    assert_eq!(prompt.messages[1].attachments.len(), 1);
}
