use rmcp::model::{Content, RawContent, RawImageContent, RawResource, RawTextContent};
use systemprompt_agent::services::a2a_server::processing::task_builder::helpers::{
    content_to_json, extract_text_from_content,
};

fn text_content(text: &str) -> Content {
    Content {
        raw: RawContent::Text(RawTextContent {
            text: text.to_string(),
            meta: None,
        }),
        annotations: None,
    }
}

fn image_content(data: &str, mime: &str) -> Content {
    Content {
        raw: RawContent::Image(RawImageContent {
            data: data.to_string(),
            mime_type: mime.to_string(),
            meta: None,
        }),
        annotations: None,
    }
}

fn resource_content(uri: &str) -> Content {
    Content {
        raw: RawContent::ResourceLink(RawResource {
            uri: uri.to_string(),
            name: "test-resource".to_string(),
            title: None,
            description: None,
            mime_type: Some("application/json".to_string()),
            size: None,
            icons: None,
            meta: None,
        }),
        annotations: None,
    }
}

#[test]
fn extract_text_single_text_part() {
    let content = vec![text_content("Hello world")];
    let result = extract_text_from_content(&content);
    assert_eq!(result, "Hello world");
}

#[test]
fn extract_text_multiple_text_parts() {
    let content = vec![text_content("First"), text_content("Second")];
    let result = extract_text_from_content(&content);
    assert_eq!(result, "First\nSecond");
}

#[test]
fn extract_text_empty_content() {
    let content: Vec<Content> = vec![];
    let result = extract_text_from_content(&content);
    assert_eq!(result, "");
}

#[test]
fn extract_text_skips_non_text_parts() {
    let content = vec![resource_content("file:///test.txt")];
    let result = extract_text_from_content(&content);
    assert_eq!(result, "");
}

#[test]
fn extract_text_mixed_content_extracts_only_text() {
    let content = vec![
        text_content("text part"),
        resource_content("file:///a.txt"),
        text_content("another text"),
    ];
    let result = extract_text_from_content(&content);
    assert_eq!(result, "text part\nanother text");
}

#[test]
fn extract_text_single_empty_text() {
    let content = vec![text_content("")];
    let result = extract_text_from_content(&content);
    assert_eq!(result, "");
}

#[test]
fn extract_text_three_text_parts_joined_by_newlines() {
    let content = vec![
        text_content("alpha"),
        text_content("beta"),
        text_content("gamma"),
    ];
    let result = extract_text_from_content(&content);
    assert_eq!(result, "alpha\nbeta\ngamma");
}

#[test]
fn content_to_json_single_text() {
    let content = vec![text_content("hello")];
    let result = content_to_json(&content);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["type"], "text");
    assert_eq!(arr[0]["text"], "hello");
}

#[test]
fn content_to_json_empty() {
    let content: Vec<Content> = vec![];
    let result = content_to_json(&content);
    let arr = result.as_array().unwrap();
    assert!(arr.is_empty());
}

#[test]
fn content_to_json_resource_link() {
    let content = vec![resource_content("file:///data.json")];
    let result = content_to_json(&content);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["type"], "resource");
    assert_eq!(arr[0]["uri"], "file:///data.json");
    assert_eq!(arr[0]["mimeType"], "application/json");
}

#[test]
fn content_to_json_multiple_text() {
    let content = vec![text_content("a"), text_content("b"), text_content("c")];
    let result = content_to_json(&content);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0]["text"], "a");
    assert_eq!(arr[1]["text"], "b");
    assert_eq!(arr[2]["text"], "c");
}

#[test]
fn content_to_json_mixed_types() {
    let content = vec![text_content("hello"), resource_content("file:///x")];
    let result = content_to_json(&content);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["type"], "text");
    assert_eq!(arr[1]["type"], "resource");
}

#[test]
fn content_to_json_returns_array_type() {
    let content = vec![text_content("test")];
    let result = content_to_json(&content);
    assert!(result.is_array());
}

#[test]
fn content_to_json_text_has_correct_keys() {
    let content = vec![text_content("structured")];
    let result = content_to_json(&content);
    let item = &result[0];
    assert!(item.get("type").is_some());
    assert!(item.get("text").is_some());
    assert!(item.get("data").is_none());
}

#[test]
fn extract_text_skips_image_parts() {
    let content = vec![image_content("base64data", "image/png")];
    let result = extract_text_from_content(&content);
    assert_eq!(result, "");
}

#[test]
fn content_to_json_image_content() {
    let content = vec![image_content("aW1hZ2U=", "image/jpeg")];
    let result = content_to_json(&content);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["type"], "image");
    assert_eq!(arr[0]["data"], "aW1hZ2U=");
    assert_eq!(arr[0]["mimeType"], "image/jpeg");
}

#[test]
fn content_to_json_resource_has_correct_keys() {
    let content = vec![resource_content("file:///r")];
    let result = content_to_json(&content);
    let item = &result[0];
    assert!(item.get("type").is_some());
    assert!(item.get("uri").is_some());
    assert!(item.get("mimeType").is_some());
}
