use systemprompt_ai::models::providers::anthropic::{
    AnthropicContentBlockInfo, AnthropicDelta, AnthropicStreamEvent,
};

mod stream_event_deserialization {
    use super::*;

    #[test]
    fn message_start_event() {
        let json = r#"{
            "type": "message_start",
            "message": {
                "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
                "model": "claude-sonnet-4-6-20250610",
                "role": "assistant",
                "usage": {"input_tokens": 25, "output_tokens": 1}
            }
        }"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                assert_eq!(message.model, "claude-sonnet-4-6-20250610");
                assert_eq!(message.role, "assistant");
                assert_eq!(message.usage.input, 25);
                assert_eq!(message.usage.output, 1);
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn content_block_start_text_event() {
        let json = r#"{
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "text", "text": ""}
        }"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(index, 0);
                assert!(matches!(
                    content_block,
                    AnthropicContentBlockInfo::Text { .. }
                ));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn content_block_start_tool_use_event() {
        let json = r#"{
            "type": "content_block_start",
            "index": 1,
            "content_block": {
                "type": "tool_use",
                "id": "toolu_abc123",
                "name": "get_weather",
                "input": {}
            }
        }"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(index, 1);
                match content_block {
                    AnthropicContentBlockInfo::ToolUse { id, name, .. } => {
                        assert_eq!(id, "toolu_abc123");
                        assert_eq!(name, "get_weather");
                    },
                    _ => panic!("expected tool_use"),
                }
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn content_block_delta_text_event() {
        let json = r#"{
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "Hello!"}
        }"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                match delta {
                    AnthropicDelta::TextDelta { text } => assert_eq!(text, "Hello!"),
                    _ => panic!("expected text_delta"),
                }
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn content_block_delta_json_event() {
        let json = r#"{
            "type": "content_block_delta",
            "index": 1,
            "delta": {"type": "input_json_delta", "partial_json": "{\"key\":"}
        }"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 1);
                match delta {
                    AnthropicDelta::InputJsonDelta { partial_json } => {
                        assert_eq!(partial_json, r#"{"key":"#);
                    },
                    _ => panic!("expected input_json_delta"),
                }
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn content_block_stop_event() {
        let json = r#"{"type": "content_block_stop", "index": 0}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::ContentBlockStop { index } => assert_eq!(index, 0),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn message_delta_event() {
        let json = r#"{
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn", "stop_sequence": null},
            "usage": {"output_tokens": 42}
        }"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason.as_deref(), Some("end_turn"));
                assert!(delta.stop_sequence.is_none());
                assert_eq!(usage.output_tokens, 42);
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn message_stop_event() {
        let json = r#"{"type": "message_stop"}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        assert!(matches!(event, AnthropicStreamEvent::MessageStop));
    }

    #[test]
    fn ping_event() {
        let json = r#"{"type": "ping"}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        assert!(matches!(event, AnthropicStreamEvent::Ping));
    }

    #[test]
    fn error_event() {
        let json = r#"{
            "type": "error",
            "error": {"type": "overloaded_error", "message": "Overloaded"}
        }"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::Error { error } => {
                assert_eq!(error.error_type, "overloaded_error");
                assert_eq!(error.message, "Overloaded");
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn message_start_usage_fields() {
        let json = r#"{
            "type": "message_start",
            "message": {
                "id": "msg_abc",
                "model": "claude-haiku-4-5-20251101",
                "role": "assistant",
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 5,
                    "cache_creation_input_tokens": 50,
                    "cache_read_input_tokens": 25
                }
            }
        }"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                assert_eq!(message.usage.input, 100);
                assert_eq!(message.usage.output, 5);
                assert_eq!(message.usage.cache_creation, Some(50));
                assert_eq!(message.usage.cache_read, Some(25));
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn message_delta_with_stop_sequence() {
        let json = r#"{
            "type": "message_delta",
            "delta": {"stop_reason": "stop_sequence", "stop_sequence": "</end>"},
            "usage": {"output_tokens": 15}
        }"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason.as_deref(), Some("stop_sequence"));
                assert_eq!(delta.stop_sequence.as_deref(), Some("</end>"));
                assert_eq!(usage.output_tokens, 15);
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn content_block_stop_at_index_5() {
        let json = r#"{"type": "content_block_stop", "index": 5}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json).expect("de");
        match event {
            AnthropicStreamEvent::ContentBlockStop { index } => assert_eq!(index, 5),
            _ => panic!("wrong variant"),
        }
    }
}
