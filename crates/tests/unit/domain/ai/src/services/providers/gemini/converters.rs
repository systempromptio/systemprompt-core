//! Tests for Gemini converter functions.

use rmcp::model::{Annotated, Content, RawContent, RawTextContent};
use serde_json::json;
use systemprompt_ai::models::ai::{AiContentPart, AiMessage, MessageRole};
use systemprompt_ai::models::providers::gemini::GeminiPart;
use systemprompt_ai::models::tools::CallToolResult;
use systemprompt_ai::services::providers::gemini::converters::{
    convert_messages, convert_tool_result_to_json,
};

mod convert_messages_tests {
    use super::*;

    #[test]
    fn converts_user_message() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "Hello!".to_string(),
            parts: Vec::new(),
        }];

        let result = convert_messages(&messages);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[0].parts.len(), 1);
    }

    #[test]
    fn converts_assistant_to_model_role() {
        let messages = vec![AiMessage {
            role: MessageRole::Assistant,
            content: "Hi there!".to_string(),
            parts: Vec::new(),
        }];

        let result = convert_messages(&messages);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "model");
    }

    #[test]
    fn inserts_system_prompt_at_beginning() {
        let messages = vec![
            AiMessage {
                role: MessageRole::System,
                content: "You are helpful.".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::User,
                content: "Hello!".to_string(),
                parts: Vec::new(),
            },
        ];

        let result = convert_messages(&messages);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "user");
    }

    #[test]
    fn combines_multiple_system_messages() {
        let messages = vec![
            AiMessage {
                role: MessageRole::System,
                content: "Rule 1".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::System,
                content: "Rule 2".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::User,
                content: "Hello!".to_string(),
                parts: Vec::new(),
            },
        ];

        let result = convert_messages(&messages);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn handles_empty_messages() {
        let messages: Vec<AiMessage> = vec![];
        let result = convert_messages(&messages);
        assert!(result.is_empty());
    }

    #[test]
    fn handles_conversation_flow() {
        let messages = vec![
            AiMessage {
                role: MessageRole::User,
                content: "Hello!".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::Assistant,
                content: "Hi!".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::User,
                content: "How are you?".to_string(),
                parts: Vec::new(),
            },
        ];

        let result = convert_messages(&messages);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[1].role, "model");
        assert_eq!(result[2].role, "user");
    }

    #[test]
    fn system_content_joined_with_newline() {
        let messages = vec![
            AiMessage {
                role: MessageRole::System,
                content: "Rule A".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::System,
                content: "Rule B".to_string(),
                parts: Vec::new(),
            },
        ];

        let result = convert_messages(&messages);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "user");
        match &result[0].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "Rule A\nRule B"),
            _ => panic!("Expected Text part"),
        }
    }

    #[test]
    fn system_message_prepended_before_user_messages() {
        let messages = vec![
            AiMessage {
                role: MessageRole::User,
                content: "First user message".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::System,
                content: "System instruction".to_string(),
                parts: Vec::new(),
            },
        ];

        let result = convert_messages(&messages);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "user");
        match &result[0].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "System instruction"),
            _ => panic!("Expected Text part for system message"),
        }
    }

    #[test]
    fn user_message_text_content_as_single_part() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "Simple text".to_string(),
            parts: Vec::new(),
        }];

        let result = convert_messages(&messages);

        assert_eq!(result[0].parts.len(), 1);
        match &result[0].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "Simple text"),
            _ => panic!("Expected Text part"),
        }
    }

    #[test]
    fn converts_message_with_text_parts() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Text {
                text: "Part text".to_string(),
            }],
        }];

        let result = convert_messages(&messages);

        assert_eq!(result[0].parts.len(), 1);
        match &result[0].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "Part text"),
            _ => panic!("Expected Text part"),
        }
    }

    #[test]
    fn converts_message_with_image_part() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Image {
                mime_type: "image/png".to_string(),
                data: "base64data".to_string(),
            }],
        }];

        let result = convert_messages(&messages);

        assert_eq!(result[0].parts.len(), 1);
        match &result[0].parts[0] {
            GeminiPart::InlineData { inline_data } => {
                assert_eq!(inline_data.mime_type, "image/png");
                assert_eq!(inline_data.data, "base64data");
            },
            _ => panic!("Expected InlineData part"),
        }
    }

    #[test]
    fn converts_message_with_audio_part() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Audio {
                mime_type: "audio/mp3".to_string(),
                data: "audiodata".to_string(),
            }],
        }];

        let result = convert_messages(&messages);

        assert_eq!(result[0].parts.len(), 1);
        match &result[0].parts[0] {
            GeminiPart::InlineData { inline_data } => {
                assert_eq!(inline_data.mime_type, "audio/mp3");
                assert_eq!(inline_data.data, "audiodata");
            },
            _ => panic!("Expected InlineData part"),
        }
    }

    #[test]
    fn converts_message_with_video_part() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Video {
                mime_type: "video/mp4".to_string(),
                data: "videodata".to_string(),
            }],
        }];

        let result = convert_messages(&messages);

        assert_eq!(result[0].parts.len(), 1);
        match &result[0].parts[0] {
            GeminiPart::InlineData { inline_data } => {
                assert_eq!(inline_data.mime_type, "video/mp4");
                assert_eq!(inline_data.data, "videodata");
            },
            _ => panic!("Expected InlineData part"),
        }
    }

    #[test]
    fn converts_message_with_mixed_parts() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![
                AiContentPart::Text {
                    text: "Description".to_string(),
                },
                AiContentPart::Image {
                    mime_type: "image/jpeg".to_string(),
                    data: "jpegdata".to_string(),
                },
                AiContentPart::Audio {
                    mime_type: "audio/wav".to_string(),
                    data: "wavdata".to_string(),
                },
            ],
        }];

        let result = convert_messages(&messages);

        assert_eq!(result[0].parts.len(), 3);
        assert!(matches!(&result[0].parts[0], GeminiPart::Text { .. }));
        assert!(matches!(&result[0].parts[1], GeminiPart::InlineData { .. }));
        assert!(matches!(&result[0].parts[2], GeminiPart::InlineData { .. }));
    }

    #[test]
    fn no_system_messages_produces_no_prepended_content() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "Just a user message".to_string(),
            parts: Vec::new(),
        }];

        let result = convert_messages(&messages);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "user");
    }

    #[test]
    fn preserves_content_with_unicode() {
        let content = "Unicode content: \u{1F600} \u{4F60}\u{597D}";
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: content.to_string(),
            parts: Vec::new(),
        }];

        let result = convert_messages(&messages);

        match &result[0].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, content),
            _ => panic!("Expected Text part"),
        }
    }

    #[test]
    fn empty_parts_uses_content_field() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "Content field value".to_string(),
            parts: Vec::new(),
        }];

        let result = convert_messages(&messages);

        match &result[0].parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "Content field value"),
            _ => panic!("Expected Text part"),
        }
    }
}

mod convert_tool_result_to_json_tests {
    use super::*;

    fn create_text_content(text: &str) -> Content {
        Annotated {
            raw: RawContent::Text(RawTextContent {
                text: text.to_string(),
                meta: None,
            }),
            annotations: None,
        }
    }

    #[test]
    fn converts_error_result() {
        let result = CallToolResult::error(vec![create_text_content("Something went wrong")]);

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json, json!({"error": "Something went wrong"}));
    }

    #[test]
    fn returns_structured_content_when_present() {
        let structured = json!({
            "status": "success",
            "data": [1, 2, 3]
        });

        let mut result = CallToolResult::success(vec![create_text_content("ignored")]);
        result.structured_content = Some(structured.clone());

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json, structured);
    }

    #[test]
    fn converts_text_content() {
        let result = CallToolResult::success(vec![create_text_content("Hello, world!")]);

        let json = convert_tool_result_to_json(&result);

        assert_eq!(
            json,
            json!({
                "content": [
                    {"type": "text", "text": "Hello, world!"}
                ]
            })
        );
    }

    #[test]
    fn converts_multiple_text_contents() {
        let result = CallToolResult::success(vec![
            create_text_content("Line 1"),
            create_text_content("Line 2"),
        ]);

        let json = convert_tool_result_to_json(&result);

        let content = json["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["text"], "Line 1");
        assert_eq!(content[1]["text"], "Line 2");
    }

    #[test]
    fn handles_error_with_multiple_messages() {
        let result = CallToolResult::error(vec![
            create_text_content("Error 1"),
            create_text_content("Error 2"),
        ]);

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json["error"], "Error 1\nError 2");
    }

    #[test]
    fn handles_none_is_error_as_false() {
        let result = CallToolResult::success(vec![create_text_content("Result")]);

        let json = convert_tool_result_to_json(&result);

        assert!(!json.as_object().unwrap().contains_key("error"));
    }

    #[test]
    fn handles_empty_content() {
        let result = CallToolResult::success(vec![]);

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json, json!({"content": []}));
    }

    #[test]
    fn structured_content_takes_precedence_over_text_content() {
        let structured = json!({"key": "value"});

        let mut result = CallToolResult::success(vec![
            create_text_content("text1"),
            create_text_content("text2"),
        ]);
        result.structured_content = Some(structured.clone());

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json, structured);
        assert!(!json.as_object().unwrap().contains_key("content"));
    }

    #[test]
    fn error_takes_precedence_over_structured_content() {
        let structured = json!({"key": "value"});

        let mut result = CallToolResult::error(vec![create_text_content("Error occurred")]);
        result.structured_content = Some(structured);

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json, json!({"error": "Error occurred"}));
    }

    #[test]
    fn error_with_empty_content_returns_empty_error_string() {
        let result = CallToolResult::error(vec![]);

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json, json!({"error": ""}));
    }

    #[test]
    fn text_content_includes_type_field() {
        let result = CallToolResult::success(vec![create_text_content("test")]);

        let json = convert_tool_result_to_json(&result);

        let content = &json["content"][0];
        assert_eq!(content["type"], "text");
    }

    #[test]
    fn preserves_text_content_exactly() {
        let complex_text = "Multi-line\nwith\ttabs\nand special chars: <>&\"'";
        let result = CallToolResult::success(vec![create_text_content(complex_text)]);

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json["content"][0]["text"], complex_text);
    }

    #[test]
    fn structured_content_preserves_nested_objects() {
        let structured = json!({
            "level1": {
                "level2": {
                    "level3": [1, 2, 3]
                }
            }
        });

        let mut result = CallToolResult::success(vec![]);
        result.structured_content = Some(structured.clone());

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json, structured);
    }

    #[test]
    fn structured_content_null_value_preserved() {
        let structured = json!({"key": null});

        let mut result = CallToolResult::success(vec![]);
        result.structured_content = Some(structured.clone());

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json, structured);
    }

    #[test]
    fn success_with_single_text_has_content_array() {
        let result = CallToolResult::success(vec![create_text_content("single")]);

        let json = convert_tool_result_to_json(&result);

        assert!(json["content"].is_array());
        assert_eq!(json["content"].as_array().unwrap().len(), 1);
    }
}
