use serde_json::json;
use systemprompt_gateway::audit::canonical_tool_result;
use systemprompt_models::artifacts::EXECUTION_META_KEY;
use systemprompt_wire::canonical::{CanonicalContent, ImageSource};

#[test]
fn canonical_tool_result_preserves_wire_semantics() {
    let content = vec![
        CanonicalContent::text("plain"),
        CanonicalContent::image(ImageSource::Base64 {
            media_type: "image/png".to_owned(),
            data: "aGVsbG8=".to_owned(),
            detail: None,
        }),
        CanonicalContent::image(ImageSource::Url {
            url: "https://example.invalid/image.png".to_owned(),
            detail: None,
        }),
        CanonicalContent::ToolUse {
            id: "call-1".to_owned(),
            name: "ignored".to_owned(),
            input: json!({}),
            signature: None,
            cache_control: None,
        },
        CanonicalContent::Thinking {
            text: "ignored".to_owned(),
            signature: None,
            id: None,
            encrypted_content: None,
        },
    ];
    let mut meta = json!({});
    meta[EXECUTION_META_KEY] = json!({"artifact_id": "artifact-wire"});
    let result = canonical_tool_result(&content, Some(&json!({"answer": 42})), Some(&meta), true);
    assert_eq!(result.content.len(), 3);
    assert_eq!(result.is_error, Some(true));
    assert_eq!(result.structured_content, Some(json!({"answer": 42})));
    assert_eq!(
        result.meta.unwrap().0[EXECUTION_META_KEY]["artifact_id"],
        "artifact-wire"
    );
}
