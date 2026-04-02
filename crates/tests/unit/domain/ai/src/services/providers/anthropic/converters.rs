//! Tests for Anthropic converter functions.

use serde_json::json;
use systemprompt_ai::models::ai::{AiContentPart, AiMessage, MessageRole};
use systemprompt_ai::models::providers::anthropic::{AnthropicContent, AnthropicContentBlock, AnthropicImageSource};
use systemprompt_ai::models::tools::McpTool;
use systemprompt_ai::services::providers::anthropic::converters::{
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

    #[test]
    fn last_system_message_wins() {
        let messages = vec![
            AiMessage {
                role: MessageRole::System,
                content: "First system prompt.".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::System,
                content: "Second system prompt.".to_string(),
                parts: Vec::new(),
            },
        ];

        let (system_prompt, anthropic_messages) = convert_messages(&messages);

        assert_eq!(
            system_prompt,
            Some("Second system prompt.".to_string())
        );
        assert!(anthropic_messages.is_empty());
    }

    #[test]
    fn system_between_user_messages_overwrites() {
        let messages = vec![
            AiMessage {
                role: MessageRole::System,
                content: "Initial system.".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::User,
                content: "Hello!".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::System,
                content: "Updated system.".to_string(),
                parts: Vec::new(),
            },
        ];

        let (system_prompt, anthropic_messages) = convert_messages(&messages);

        assert_eq!(system_prompt, Some("Updated system.".to_string()));
        assert_eq!(anthropic_messages.len(), 1);
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

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    AnthropicContentBlock::Text { text } => assert_eq!(text, "Part text"),
                    _ => panic!("Expected Text block"),
                }
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn converts_message_with_content_and_text_parts() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "Main content".to_string(),
            parts: vec![AiContentPart::Text {
                text: "Additional text".to_string(),
            }],
        }];

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                match &blocks[0] {
                    AnthropicContentBlock::Text { text } => assert_eq!(text, "Main content"),
                    _ => panic!("Expected Text block for main content"),
                }
                match &blocks[1] {
                    AnthropicContentBlock::Text { text } => assert_eq!(text, "Additional text"),
                    _ => panic!("Expected Text block for part"),
                }
            },
            _ => panic!("Expected Blocks content"),
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

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    AnthropicContentBlock::Image { source } => {
                        match source {
                            AnthropicImageSource::Base64 { media_type, data } => {
                                assert_eq!(media_type, "image/png");
                                assert_eq!(data, "base64data");
                            },
                        }
                    },
                    _ => panic!("Expected Image block"),
                }
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn converts_message_with_mixed_parts() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "Describe this:".to_string(),
            parts: vec![
                AiContentPart::Text {
                    text: "Context text".to_string(),
                },
                AiContentPart::Image {
                    mime_type: "image/jpeg".to_string(),
                    data: "jpegdata".to_string(),
                },
            ],
        }];

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 3);
                assert!(matches!(&blocks[0], AnthropicContentBlock::Text { .. }));
                assert!(matches!(&blocks[1], AnthropicContentBlock::Text { .. }));
                assert!(matches!(&blocks[2], AnthropicContentBlock::Image { .. }));
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn skips_audio_parts() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![
                AiContentPart::Text {
                    text: "Some text".to_string(),
                },
                AiContentPart::Audio {
                    mime_type: "audio/mp3".to_string(),
                    data: "audiodata".to_string(),
                },
            ],
        }];

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                assert!(matches!(&blocks[0], AnthropicContentBlock::Text { .. }));
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn skips_video_parts() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![
                AiContentPart::Text {
                    text: "Some text".to_string(),
                },
                AiContentPart::Video {
                    mime_type: "video/mp4".to_string(),
                    data: "videodata".to_string(),
                },
            ],
        }];

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                assert!(matches!(&blocks[0], AnthropicContentBlock::Text { .. }));
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn empty_content_with_parts_omits_content_block() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Text {
                text: "Only part".to_string(),
            }],
        }];

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    AnthropicContentBlock::Text { text } => assert_eq!(text, "Only part"),
                    _ => panic!("Expected Text block"),
                }
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn preserves_empty_content_when_no_parts() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: Vec::new(),
        }];

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Text(text) => assert_eq!(text, ""),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn multiple_image_parts() {
        let messages = vec![AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![
                AiContentPart::Image {
                    mime_type: "image/png".to_string(),
                    data: "img1".to_string(),
                },
                AiContentPart::Image {
                    mime_type: "image/jpeg".to_string(),
                    data: "img2".to_string(),
                },
            ],
        }];

        let (_, anthropic_messages) = convert_messages(&messages);

        match &anthropic_messages[0].content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                assert!(matches!(&blocks[0], AnthropicContentBlock::Image { .. }));
                assert!(matches!(&blocks[1], AnthropicContentBlock::Image { .. }));
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn only_user_messages_no_system() {
        let messages = vec![
            AiMessage {
                role: MessageRole::User,
                content: "First".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::Assistant,
                content: "Response".to_string(),
                parts: Vec::new(),
            },
            AiMessage {
                role: MessageRole::User,
                content: "Second".to_string(),
                parts: Vec::new(),
            },
        ];

        let (system_prompt, anthropic_messages) = convert_messages(&messages);

        assert!(system_prompt.is_none());
        assert_eq!(anthropic_messages.len(), 3);
    }

    #[test]
    fn single_system_message_only() {
        let messages = vec![AiMessage {
            role: MessageRole::System,
            content: "System only".to_string(),
            parts: Vec::new(),
        }];

        let (system_prompt, anthropic_messages) = convert_messages(&messages);

        assert_eq!(system_prompt, Some("System only".to_string()));
        assert!(anthropic_messages.is_empty());
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

    #[test]
    fn preserves_tool_name_with_special_characters() {
        let tool = create_mcp_tool(
            "my-tool_v2.0",
            Some("Tool with special chars in name"),
            Some(json!({"type": "object"})),
        );

        let result = convert_tools(vec![tool]);

        assert_eq!(result[0].name, "my-tool_v2.0");
    }

    #[test]
    fn preserves_description_with_newlines() {
        let tool = create_mcp_tool(
            "documented_tool",
            Some("Line 1\nLine 2\nLine 3"),
            Some(json!({"type": "object"})),
        );

        let result = convert_tools(vec![tool]);

        assert_eq!(
            result[0].description,
            Some("Line 1\nLine 2\nLine 3".to_string())
        );
    }

    #[test]
    fn default_schema_has_object_type() {
        let tool = create_mcp_tool("no_schema", None, None);

        let result = convert_tools(vec![tool]);

        assert_eq!(result[0].input_schema["type"], "object");
    }

    #[test]
    fn default_schema_has_empty_properties() {
        let tool = create_mcp_tool("no_schema", None, None);

        let result = convert_tools(vec![tool]);

        let properties = result[0].input_schema["properties"].as_object().unwrap();
        assert!(properties.is_empty());
    }

    #[test]
    fn preserves_schema_with_nested_arrays() {
        let schema = json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "nested_array": {
                                "type": "array",
                                "items": {"type": "string"}
                            }
                        }
                    }
                }
            }
        });

        let tool = create_mcp_tool("nested_tool", None, Some(schema.clone()));

        let result = convert_tools(vec![tool]);

        assert_eq!(result[0].input_schema, schema);
    }

    #[test]
    fn converts_many_tools_preserves_order() {
        let tools: Vec<McpTool> = (0..10)
            .map(|i| {
                create_mcp_tool(
                    &format!("tool_{i}"),
                    Some(&format!("Description {i}")),
                    Some(json!({"type": "object"})),
                )
            })
            .collect();

        let result = convert_tools(tools);

        assert_eq!(result.len(), 10);
        for (i, tool) in result.iter().enumerate() {
            assert_eq!(tool.name, format!("tool_{i}"));
        }
    }

    #[test]
    fn single_tool_produces_single_element_vec() {
        let tool = create_mcp_tool("only_one", Some("Single"), Some(json!({"type": "object"})));

        let result = convert_tools(vec![tool]);

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn tool_with_empty_name() {
        let tool = create_mcp_tool("", Some("Empty name"), Some(json!({"type": "object"})));

        let result = convert_tools(vec![tool]);

        assert_eq!(result[0].name, "");
    }

    #[test]
    fn tool_with_empty_description() {
        let tool = create_mcp_tool("tool", Some(""), Some(json!({"type": "object"})));

        let result = convert_tools(vec![tool]);

        assert_eq!(result[0].description, Some("".to_string()));
    }
}
