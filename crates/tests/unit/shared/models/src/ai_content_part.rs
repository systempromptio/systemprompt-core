use systemprompt_identifiers::ToolUseId;
use systemprompt_models::ai::AiContentPart;

#[test]
fn text_constructor_yields_text_variant() {
    let p = AiContentPart::text("hi");
    assert!(matches!(&p, AiContentPart::Text { text } if text == "hi"));
    assert!(!p.is_media());
}

#[test]
fn image_constructor_yields_image_variant() {
    let p = AiContentPart::image("image/png", "base64data");
    match p.clone() {
        AiContentPart::Image { mime_type, data } => {
            assert_eq!(mime_type, "image/png");
            assert_eq!(data, "base64data");
        },
        _ => panic!("expected Image"),
    }
    assert!(p.is_media());
}

#[test]
fn audio_constructor_yields_audio_variant() {
    let p = AiContentPart::audio("audio/mpeg", "raw");
    assert!(matches!(p, AiContentPart::Audio { .. }));
    assert!(p.is_media());
}

#[test]
fn video_constructor_yields_video_variant() {
    let p = AiContentPart::video("video/mp4", "raw");
    assert!(matches!(p, AiContentPart::Video { .. }));
    assert!(p.is_media());
}

#[test]
fn ai_content_part_serde_text_round_trip() {
    let p = AiContentPart::text("hello");
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["type"], "text");
    assert_eq!(json["text"], "hello");
    let parsed: AiContentPart = serde_json::from_value(json).unwrap();
    assert_eq!(parsed, p);
}

#[test]
fn ai_content_part_serde_image_round_trip() {
    let p = AiContentPart::image("image/jpeg", "ABCD");
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["type"], "image");
    assert_eq!(json["mime_type"], "image/jpeg");
    let parsed: AiContentPart = serde_json::from_value(json).unwrap();
    assert_eq!(parsed, p);
}

#[test]
fn ai_content_part_is_media_excludes_text() {
    assert!(!AiContentPart::text("x").is_media());
    assert!(AiContentPart::image("image/png", "x").is_media());
    assert!(AiContentPart::audio("audio/mpeg", "x").is_media());
    assert!(AiContentPart::video("video/mp4", "x").is_media());
}

#[test]
fn tool_use_serde_round_trip_omits_an_absent_signature() {
    let p = AiContentPart::tool_use("call-1", "search", serde_json::json!({"q": "rust"}));
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "type": "tool_use",
            "id": "call-1",
            "name": "search",
            "input": {"q": "rust"},
        })
    );
    assert_eq!(serde_json::from_value::<AiContentPart>(json).unwrap(), p);
    assert!(!p.is_media());
}

#[test]
fn tool_result_deserialises_without_is_error() {
    let p: AiContentPart = serde_json::from_value(serde_json::json!({
        "type": "tool_result",
        "tool_use_id": "call-1",
        "content": "3 hits",
    }))
    .unwrap();
    assert_eq!(
        p,
        AiContentPart::tool_result(ToolUseId::new("call-1"), "3 hits", false)
    );
}

#[test]
fn thinking_serde_round_trip_keeps_the_signature() {
    let p = AiContentPart::thinking("weighing options", Some("sig-1".to_owned()));
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "type": "thinking",
            "text": "weighing options",
            "signature": "sig-1",
        })
    );
    assert_eq!(serde_json::from_value::<AiContentPart>(json).unwrap(), p);
}
