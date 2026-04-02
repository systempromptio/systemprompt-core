//! Tests for message converter implementations.

use systemprompt_ai::models::ai::{AiContentPart, AiMessage, MessageRole};
use systemprompt_ai::models::providers::anthropic::AnthropicMessage;
use systemprompt_ai::models::providers::gemini::GeminiContent;
use systemprompt_ai::models::providers::openai::{OpenAiMessage, OpenAiMessageContent};

mod openai_converter_tests {
    use super::*;

    fn assert_openai_text_content(content: &OpenAiMessageContent, expected: &str) {
        match content {
            OpenAiMessageContent::Text(text) => assert_eq!(text, expected),
            OpenAiMessageContent::Parts(_) => panic!("Expected Text, got Parts"),
        }
    }

    #[test]
    fn user_message_converts_to_openai_user_role() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "Hello, world!".to_string(),
            parts: Vec::new(),
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        assert_eq!(openai_msg.role, "user");
        assert_openai_text_content(&openai_msg.content, "Hello, world!");
    }

    #[test]
    fn assistant_message_converts_to_openai_assistant_role() {
        let ai_message = AiMessage {
            role: MessageRole::Assistant,
            content: "How can I help you?".to_string(),
            parts: Vec::new(),
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        assert_eq!(openai_msg.role, "assistant");
        assert_openai_text_content(&openai_msg.content, "How can I help you?");
    }

    #[test]
    fn system_message_converts_to_openai_system_role() {
        let ai_message = AiMessage {
            role: MessageRole::System,
            content: "You are a helpful assistant.".to_string(),
            parts: Vec::new(),
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        assert_eq!(openai_msg.role, "system");
        assert_openai_text_content(&openai_msg.content, "You are a helpful assistant.");
    }

    #[test]
    fn empty_content_preserves_empty_string() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: Vec::new(),
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        assert_openai_text_content(&openai_msg.content, "");
    }

    #[test]
    fn preserves_content_with_special_characters() {
        let content = "Line 1\nLine 2\tTabbed\rCarriage\0Null";
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: content.to_string(),
            parts: Vec::new(),
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        assert_openai_text_content(&openai_msg.content, content);
    }
}

mod openai_multipart_tests {
    use super::*;
    use systemprompt_ai::models::providers::openai::OpenAiContentPart;

    #[test]
    fn text_parts_converted_to_parts_content() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Text {
                text: "Part text".to_string(),
            }],
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        match &openai_msg.content {
            OpenAiMessageContent::Parts(parts) => {
                assert_eq!(parts.len(), 1);
                match &parts[0] {
                    OpenAiContentPart::Text { text } => assert_eq!(text, "Part text"),
                    _ => panic!("Expected Text part"),
                }
            },
            _ => panic!("Expected Parts content"),
        }
    }

    #[test]
    fn image_part_converted_to_data_uri() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Image {
                mime_type: "image/png".to_string(),
                data: "iVBORw0KGgo".to_string(),
            }],
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        match &openai_msg.content {
            OpenAiMessageContent::Parts(parts) => {
                assert_eq!(parts.len(), 1);
                match &parts[0] {
                    OpenAiContentPart::ImageUrl { image_url } => {
                        assert_eq!(image_url.url, "data:image/png;base64,iVBORw0KGgo");
                        assert!(image_url.detail.is_none());
                    },
                    _ => panic!("Expected ImageUrl part"),
                }
            },
            _ => panic!("Expected Parts content"),
        }
    }

    #[test]
    fn content_and_parts_both_included() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "Main content".to_string(),
            parts: vec![AiContentPart::Text {
                text: "Additional".to_string(),
            }],
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        match &openai_msg.content {
            OpenAiMessageContent::Parts(parts) => {
                assert_eq!(parts.len(), 2);
                match &parts[0] {
                    OpenAiContentPart::Text { text } => assert_eq!(text, "Main content"),
                    _ => panic!("Expected Text part first"),
                }
                match &parts[1] {
                    OpenAiContentPart::Text { text } => assert_eq!(text, "Additional"),
                    _ => panic!("Expected Text part second"),
                }
            },
            _ => panic!("Expected Parts content"),
        }
    }

    #[test]
    fn empty_content_with_parts_omits_content_block() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Text {
                text: "Only part".to_string(),
            }],
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        match &openai_msg.content {
            OpenAiMessageContent::Parts(parts) => {
                assert_eq!(parts.len(), 1);
            },
            _ => panic!("Expected Parts content"),
        }
    }

    #[test]
    fn mixed_text_and_image_parts() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![
                AiContentPart::Text {
                    text: "Describe this image".to_string(),
                },
                AiContentPart::Image {
                    mime_type: "image/jpeg".to_string(),
                    data: "jpegdata".to_string(),
                },
            ],
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        match &openai_msg.content {
            OpenAiMessageContent::Parts(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(&parts[0], OpenAiContentPart::Text { .. }));
                assert!(matches!(&parts[1], OpenAiContentPart::ImageUrl { .. }));
            },
            _ => panic!("Expected Parts content"),
        }
    }

    #[test]
    fn image_data_uri_format_correct() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Image {
                mime_type: "image/webp".to_string(),
                data: "webpdata123".to_string(),
            }],
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        match &openai_msg.content {
            OpenAiMessageContent::Parts(parts) => match &parts[0] {
                OpenAiContentPart::ImageUrl { image_url } => {
                    assert!(image_url.url.starts_with("data:image/webp;base64,"));
                    assert!(image_url.url.ends_with("webpdata123"));
                },
                _ => panic!("Expected ImageUrl part"),
            },
            _ => panic!("Expected Parts content"),
        }
    }

    #[test]
    fn multiple_images() {
        let ai_message = AiMessage {
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
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        match &openai_msg.content {
            OpenAiMessageContent::Parts(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(&parts[0], OpenAiContentPart::ImageUrl { .. }));
                assert!(matches!(&parts[1], OpenAiContentPart::ImageUrl { .. }));
            },
            _ => panic!("Expected Parts content"),
        }
    }
}

mod anthropic_converter_tests {
    use super::*;
    use systemprompt_ai::models::providers::anthropic::AnthropicContent;

    #[test]
    fn user_message_converts_to_anthropic_user_role() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "Hello!".to_string(),
            parts: Vec::new(),
        };

        let anthropic_msg: AnthropicMessage = (&ai_message).into();

        assert_eq!(anthropic_msg.role, "user");
        match anthropic_msg.content {
            AnthropicContent::Text(text) => assert_eq!(text, "Hello!"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn assistant_message_converts_to_anthropic_assistant_role() {
        let ai_message = AiMessage {
            role: MessageRole::Assistant,
            content: "I'm here to help.".to_string(),
            parts: Vec::new(),
        };

        let anthropic_msg: AnthropicMessage = (&ai_message).into();

        assert_eq!(anthropic_msg.role, "assistant");
        match anthropic_msg.content {
            AnthropicContent::Text(text) => assert_eq!(text, "I'm here to help."),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn system_message_converts_to_anthropic_assistant_role() {
        let ai_message = AiMessage {
            role: MessageRole::System,
            content: "System instruction".to_string(),
            parts: Vec::new(),
        };

        let anthropic_msg: AnthropicMessage = (&ai_message).into();

        assert_eq!(anthropic_msg.role, "assistant");
        match anthropic_msg.content {
            AnthropicContent::Text(text) => assert_eq!(text, "System instruction"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn preserves_content_exactly() {
        let content = "Complex content with unicode: \u{4F60}\u{597D} \u{1F389}";
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: content.to_string(),
            parts: Vec::new(),
        };

        let anthropic_msg: AnthropicMessage = (&ai_message).into();

        match anthropic_msg.content {
            AnthropicContent::Text(text) => assert_eq!(text, content),
            _ => panic!("Expected text content"),
        }
    }
}

mod anthropic_multipart_tests {
    use super::*;
    use systemprompt_ai::models::providers::anthropic::{
        AnthropicContent, AnthropicContentBlock, AnthropicImageSource,
    };

    #[test]
    fn text_parts_produce_blocks_content() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Text {
                text: "Part text".to_string(),
            }],
        };

        let anthropic_msg: AnthropicMessage = (&ai_message).into();

        match anthropic_msg.content {
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
    fn image_parts_produce_image_blocks() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Image {
                mime_type: "image/png".to_string(),
                data: "pngdata".to_string(),
            }],
        };

        let anthropic_msg: AnthropicMessage = (&ai_message).into();

        match anthropic_msg.content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    AnthropicContentBlock::Image { source } => match source {
                        AnthropicImageSource::Base64 { media_type, data } => {
                            assert_eq!(media_type, "image/png");
                            assert_eq!(data, "pngdata");
                        },
                    },
                    _ => panic!("Expected Image block"),
                }
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn content_and_parts_both_included_as_blocks() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "Main content".to_string(),
            parts: vec![AiContentPart::Text {
                text: "Part content".to_string(),
            }],
        };

        let anthropic_msg: AnthropicMessage = (&ai_message).into();

        match anthropic_msg.content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                match &blocks[0] {
                    AnthropicContentBlock::Text { text } => assert_eq!(text, "Main content"),
                    _ => panic!("Expected Text block first"),
                }
                match &blocks[1] {
                    AnthropicContentBlock::Text { text } => assert_eq!(text, "Part content"),
                    _ => panic!("Expected Text block second"),
                }
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn empty_content_with_parts_omits_content_block() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![AiContentPart::Text {
                text: "Only part".to_string(),
            }],
        };

        let anthropic_msg: AnthropicMessage = (&ai_message).into();

        match anthropic_msg.content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
            },
            _ => panic!("Expected Blocks content"),
        }
    }

    #[test]
    fn mixed_text_and_image_parts() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: vec![
                AiContentPart::Text {
                    text: "Describe this".to_string(),
                },
                AiContentPart::Image {
                    mime_type: "image/jpeg".to_string(),
                    data: "jpegdata".to_string(),
                },
            ],
        };

        let anthropic_msg: AnthropicMessage = (&ai_message).into();

        match anthropic_msg.content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                assert!(matches!(&blocks[0], AnthropicContentBlock::Text { .. }));
                assert!(matches!(&blocks[1], AnthropicContentBlock::Image { .. }));
            },
            _ => panic!("Expected Blocks content"),
        }
    }
}

mod gemini_converter_tests {
    use super::*;
    use systemprompt_ai::models::providers::gemini::GeminiPart;

    #[test]
    fn user_message_converts_to_gemini_user_role() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "Hello Gemini!".to_string(),
            parts: Vec::new(),
        };

        let gemini_content: GeminiContent = (&ai_message).into();

        assert_eq!(gemini_content.role, "user");
        assert_eq!(gemini_content.parts.len(), 1);
        match &gemini_content.parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "Hello Gemini!"),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn assistant_message_converts_to_gemini_model_role() {
        let ai_message = AiMessage {
            role: MessageRole::Assistant,
            content: "Hello from the model!".to_string(),
            parts: Vec::new(),
        };

        let gemini_content: GeminiContent = (&ai_message).into();

        assert_eq!(gemini_content.role, "model");
        assert_eq!(gemini_content.parts.len(), 1);
    }

    #[test]
    fn system_message_converts_to_gemini_user_role() {
        let ai_message = AiMessage {
            role: MessageRole::System,
            content: "System prompt".to_string(),
            parts: Vec::new(),
        };

        let gemini_content: GeminiContent = (&ai_message).into();

        assert_eq!(gemini_content.role, "user");
        match &gemini_content.parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, "System prompt"),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn preserves_content_in_text_part() {
        let content = "Content with\nnewlines\nand\ttabs";
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: content.to_string(),
            parts: Vec::new(),
        };

        let gemini_content: GeminiContent = (&ai_message).into();

        match &gemini_content.parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, content),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn creates_single_part_for_text() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "Single text content".to_string(),
            parts: Vec::new(),
        };

        let gemini_content: GeminiContent = (&ai_message).into();

        assert_eq!(gemini_content.parts.len(), 1);
    }
}

mod role_mapping_comparison_tests {
    use super::*;
    use systemprompt_ai::models::providers::anthropic::AnthropicContent;
    use systemprompt_ai::models::providers::gemini::GeminiPart;

    fn assert_openai_text_content(content: &OpenAiMessageContent, expected: &str) {
        match content {
            OpenAiMessageContent::Text(text) => assert_eq!(text, expected),
            OpenAiMessageContent::Parts(_) => panic!("Expected Text, got Parts"),
        }
    }

    #[test]
    fn all_providers_handle_user_role() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "Test".to_string(),
            parts: Vec::new(),
        };

        let openai: OpenAiMessage = (&ai_message).into();
        let anthropic: AnthropicMessage = (&ai_message).into();
        let gemini: GeminiContent = (&ai_message).into();

        assert_eq!(openai.role, "user");
        assert_eq!(anthropic.role, "user");
        assert_eq!(gemini.role, "user");
    }

    #[test]
    fn all_providers_handle_assistant_role() {
        let ai_message = AiMessage {
            role: MessageRole::Assistant,
            content: "Test".to_string(),
            parts: Vec::new(),
        };

        let openai: OpenAiMessage = (&ai_message).into();
        let anthropic: AnthropicMessage = (&ai_message).into();
        let gemini: GeminiContent = (&ai_message).into();

        assert_eq!(openai.role, "assistant");
        assert_eq!(anthropic.role, "assistant");
        assert_eq!(gemini.role, "model");
    }

    #[test]
    fn system_role_mapping_varies_by_provider() {
        let ai_message = AiMessage {
            role: MessageRole::System,
            content: "System instruction".to_string(),
            parts: Vec::new(),
        };

        let openai: OpenAiMessage = (&ai_message).into();
        let anthropic: AnthropicMessage = (&ai_message).into();
        let gemini: GeminiContent = (&ai_message).into();

        assert_eq!(openai.role, "system");
        assert_eq!(anthropic.role, "assistant");
        assert_eq!(gemini.role, "user");
    }

    #[test]
    fn content_preserved_across_all_providers() {
        let content = "This content should be preserved exactly as is!";
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: content.to_string(),
            parts: Vec::new(),
        };

        let openai: OpenAiMessage = (&ai_message).into();
        let anthropic: AnthropicMessage = (&ai_message).into();
        let gemini: GeminiContent = (&ai_message).into();

        assert_openai_text_content(&openai.content, content);
        match anthropic.content {
            AnthropicContent::Text(text) => assert_eq!(text, content),
            _ => panic!("Expected text content"),
        }
        match &gemini.parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, content),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn empty_content_preserved_across_all_providers() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: "".to_string(),
            parts: Vec::new(),
        };

        let openai: OpenAiMessage = (&ai_message).into();
        let anthropic: AnthropicMessage = (&ai_message).into();
        let gemini: GeminiContent = (&ai_message).into();

        assert_openai_text_content(&openai.content, "");
        match anthropic.content {
            AnthropicContent::Text(text) => assert_eq!(text, ""),
            _ => panic!("Expected text content"),
        }
        match &gemini.parts[0] {
            GeminiPart::Text { text } => assert_eq!(text, ""),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn long_content_preserved_across_all_providers() {
        let content = "A".repeat(10_000);
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: content.clone(),
            parts: Vec::new(),
        };

        let openai: OpenAiMessage = (&ai_message).into();
        let anthropic: AnthropicMessage = (&ai_message).into();
        let gemini: GeminiContent = (&ai_message).into();

        assert_openai_text_content(&openai.content, &content);
        match anthropic.content {
            AnthropicContent::Text(text) => assert_eq!(text.len(), 10_000),
            _ => panic!("Expected text content"),
        }
        match &gemini.parts[0] {
            GeminiPart::Text { text } => assert_eq!(text.len(), 10_000),
            _ => panic!("Expected text part"),
        }
    }
}
