//! Tests for Gemini converter functions.

use rmcp::model::{Annotated, Content, RawContent, RawTextContent};
use serde_json::json;
use systemprompt_ai::models::ai::{AiMessage, MessageRole};
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

        // System message is prepended as a user message
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "user");
        // First message should contain the system content
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

        // Multiple system messages should be combined
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

        // Should not be treated as error
        assert!(!json.as_object().unwrap().contains_key("error"));
    }

    #[test]
    fn handles_empty_content() {
        let result = CallToolResult::success(vec![]);

        let json = convert_tool_result_to_json(&result);

        assert_eq!(json, json!({"content": []}));
    }
}
