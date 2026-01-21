use rmcp::model::{Content, RawContent};
use serde_json::json;

pub fn extract_text_from_content(content: &[Content]) -> String {
    content
        .iter()
        .filter_map(|c| match &c.raw {
            RawContent::Text(text_content) => Some(text_content.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn content_to_json(content: &[Content]) -> serde_json::Value {
    let items: Vec<serde_json::Value> = content
        .iter()
        .map(|c| match &c.raw {
            RawContent::Text(text_content) => json!({"type": "text", "text": text_content.text}),
            RawContent::Image(image_content) => {
                json!({"type": "image", "data": image_content.data, "mimeType": image_content.mime_type})
            },
            RawContent::ResourceLink(resource) => {
                json!({"type": "resource", "uri": resource.uri, "mimeType": resource.mime_type})
            },
            _ => json!({"type": "unknown"}),
        })
        .collect();
    json!(items)
}
