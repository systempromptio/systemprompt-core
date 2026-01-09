//! Tests for Anthropic converter functions.

use serde_json::json;
use systemprompt_core_ai::models::ai::{AiMessage, MessageRole};
use systemprompt_core_ai::models::providers::anthropic::AnthropicContent;
use systemprompt_core_ai::models::tools::McpTool;
use systemprompt_core_ai::services::providers::anthropic::converters::{
    convert_messages, convert_tools,
};
use systemprompt_identifiers::McpServerId;

fn create_mcp_tool(
    name: &str,
    description: Option<&str>,
    input_schema: Option<serde_json::Value>,
) -> McpTool {
    McpTool {
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        input_schema,
        output_schema: None,
        service_id: McpServerId::new("test-service"),
        terminal_on_success: false,
        model_config: None,
    }
}

mod convert_messages_tests {
    use super::*;

    #[test]
    fn extracts_system_prompt() {
        let messages = vec![AiMessage {
            role: MessageRole::System,
            content: "You are a helpful assistant.".to_string(),
            parts: Vec::new(),
        }];

        let (system_prompt, anthropic_messages) = convert_messages(&messages);

        assert_eq!(
            system_prompt,
            Some("You are a helpful assistant.".to_string())
        );
        assert!(anthropic_messages.is_empty());
    }

    #[test]
    fn converts_user_message() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "Hello!".to_string(),
            parts: Vec::new(),
        }];

        let (system_prompt, anthropic_messages) = convert_messages(&messages);

        assert!(system_prompt.is_none());
        assert_eq!(anthropic_messages.len(), 1);
        assert_eq!(anthropic_messages[0].role, "user");
        match &anthropic_messages[0].content {
            AnthropicContent::Text(text) => assert_eq!(text, "Hello!"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn converts_assistant_message() {
        let messages = vec![AiMessage {
            role: MessageRole::Assistant,
            content: "Hi there!".to_string(),
            parts: Vec::new(),
        }];

        let (system_prompt, anthropic_messages) = convert_messages(&messages);

        assert!(system_prompt.is_none());
        assert_eq!(anthropic_messages.len(), 1);
        assert_eq!(anthropic_messages[0].role, "assistant");
        match &anthropic_messages[0].content {
            AnthropicContent::Text(text) => assert_eq!(text, "Hi there!"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn handles_mixed_messages() {
        let messages = vec![
            AiMessage {
                role: MessageRole::System,
                content: "Be helpful.".to_string(),
                parts: Vec::new(),
            },
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

        let (system_prompt, anthropic_messages) = convert_messages(&messages);

        assert_eq!(system_prompt, Some("Be helpful.".to_string()));
        assert_eq!(anthropic_messages.len(), 3);
        assert_eq!(anthropic_messages[0].role, "user");
        assert_eq!(anthropic_messages[1].role, "assistant");
        assert_eq!(anthropic_messages[2].role, "user");
    }

    #[test]
    fn handles_empty_messages() {
        let messages: Vec<AiMessage> = vec![];

        let (system_prompt, anthropic_messages) = convert_messages(&messages);

        assert!(system_prompt.is_none());
        assert!(anthropic_messages.is_empty());
    }

    #[test]
    fn preserves_message_content_exactly() {
        let content_with_special_chars = "Hello! Here's some code:\n```rust\nfn main() {}\n```";
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: content_with_special_chars.to_string(),
            parts: Vec::new(),
        }];

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Text(text) => assert_eq!(text, content_with_special_chars),
            _ => panic!("Expected Text content"),
        }
    }
}

mod convert_tools_tests {
    use super::*;

    #[test]
    fn converts_tool_with_schema() {
        let tool = create_mcp_tool(
            "search",
            Some("Search for information"),
            Some(json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            })),
        );

        let result = convert_tools(vec![tool]);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "search");
        assert_eq!(
            result[0].description,
            Some("Search for information".to_string())
        );
        assert_eq!(
            result[0].input_schema,
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            })
        );
    }

    #[test]
    fn uses_default_schema_for_none() {
        let tool = create_mcp_tool("simple_tool", Some("A simple tool"), None);

        let result = convert_tools(vec![tool]);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].input_schema,
            json!({
                "type": "object",
                "properties": {}
            })
        );
    }

    #[test]
    fn converts_multiple_tools() {
        let tools = vec![
            create_mcp_tool("tool1", Some("First"), Some(json!({"type": "object"}))),
            create_mcp_tool("tool2", Some("Second"), Some(json!({"type": "object"}))),
        ];

        let result = convert_tools(tools);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "tool1");
        assert_eq!(result[1].name, "tool2");
    }

    #[test]
    fn handles_tool_without_description() {
        let tool = create_mcp_tool("no_desc", None, Some(json!({"type": "object"})));

        let result = convert_tools(vec![tool]);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, None);
    }

    #[test]
    fn handles_empty_tools() {
        let result = convert_tools(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn preserves_complex_schema() {
        let complex_schema = json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "options": {
                    "type": "object",
                    "properties": {
                        "recursive": {"type": "boolean"}
                    }
                }
            },
            "required": ["file_path"]
        });

        let tool = create_mcp_tool("file_op", Some("File operation"), Some(complex_schema.clone()));

        let result = convert_tools(vec![tool]);

        assert_eq!(result[0].input_schema, complex_schema);
    }
}
