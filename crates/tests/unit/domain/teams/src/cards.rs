//! Tests for Adaptive Card rendering.

use systemprompt_teams::cards::render_card;

fn card_body(value: &serde_json::Value) -> Vec<serde_json::Value> {
    value[0]["content"]["body"].as_array().cloned().unwrap()
}

#[test]
fn renders_a_single_adaptive_card_attachment() {
    let card = render_card("hello");
    let arr = card.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(
        arr[0]["contentType"],
        "application/vnd.microsoft.card.adaptive"
    );
    assert_eq!(arr[0]["content"]["type"], "AdaptiveCard");
    let body = card_body(&card);
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["type"], "TextBlock");
    assert_eq!(body[0]["text"], "hello");
    assert_eq!(body[0]["wrap"], true);
}

#[test]
fn empty_text_still_yields_one_text_block() {
    let card = render_card("");
    assert_eq!(card_body(&card).len(), 1);
}

#[test]
fn long_text_is_chunked_into_multiple_text_blocks() {
    let line = "x".repeat(500);
    let text = vec![line; 20].join("\n");
    let card = render_card(&text);
    let body = card_body(&card);
    assert!(
        body.len() > 1,
        "expected multiple TextBlocks for >3000 chars, got {}",
        body.len()
    );
    for block in body {
        assert!(block["text"].as_str().unwrap().len() <= 3000);
    }
}
