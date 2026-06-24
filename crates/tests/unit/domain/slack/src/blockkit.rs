use systemprompt_slack::blockkit::render_blocks;

#[test]
fn short_text_renders_single_section() {
    let blocks = render_blocks("hello world");
    let arr = blocks.as_array().expect("blocks is an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["type"], "section");
    assert_eq!(arr[0]["text"]["type"], "mrkdwn");
    assert_eq!(arr[0]["text"]["text"], "hello world");
}

#[test]
fn empty_text_renders_single_empty_section() {
    let blocks = render_blocks("");
    let arr = blocks.as_array().expect("blocks is an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["text"]["text"], "");
}

#[test]
fn long_text_splits_into_multiple_capped_sections() {
    let line = "x".repeat(500);
    let text = std::iter::repeat(line)
        .take(20)
        .collect::<Vec<_>>()
        .join("\n");
    let blocks = render_blocks(&text);
    let arr = blocks.as_array().expect("blocks is an array");
    assert!(arr.len() > 1, "long text should chunk into multiple blocks");
    for block in arr {
        let len = block["text"]["text"].as_str().unwrap().len();
        assert!(
            len <= 3000,
            "each section must respect Slack's 3000-char cap"
        );
    }
}
