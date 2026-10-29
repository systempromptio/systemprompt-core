use serde_json::json;
use systemprompt_runtime::documents::mcp::{create_content, refusal_code};
use systemprompt_runtime::documents::refusal::RefusalCode;

#[test]
fn content_accepts_markdown_or_a_model() {
    let markdown = create_content(Some(json!({"markdown": "# Plan"}))).expect("markdown");
    assert_eq!(markdown.expect("a model").blocks[0].text, "Plan");
    let model = create_content(Some(json!({"model": {"blocks": [
        {"id": "b1", "type": "paragraph", "text": "Hello"}
    ]}})))
    .expect("model");
    assert_eq!(model.expect("a model").blocks[0].text, "Hello");
    assert!(create_content(None).expect("none").is_none());
}

#[test]
fn malformed_content_is_refused_with_the_expected_shapes() {
    for bad in [
        json!("text"),
        json!({}),
        json!({"markdown": 3}),
        json!({"markdown": "x", "model": {}}),
        json!({"html": "<p>x</p>"}),
        json!({"model": {"blocks": "nope"}}),
    ] {
        let error = create_content(Some(bad.clone())).expect_err("refused");
        assert_eq!(
            refusal_code(&error),
            Some(RefusalCode::InvalidParams),
            "{bad}"
        );
        assert!(
            error.message.contains("/api/v1/documents/schema"),
            "{bad}: {}",
            error.message
        );
    }
}
