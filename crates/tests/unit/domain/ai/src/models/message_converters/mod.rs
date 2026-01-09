//! Tests for message converter implementations.

use systemprompt_core_ai::models::ai::{AiMessage, MessageRole};
use systemprompt_core_ai::models::providers::anthropic::AnthropicMessage;
use systemprompt_core_ai::models::providers::gemini::GeminiContent;
use systemprompt_core_ai::models::providers::openai::{OpenAiMessage, OpenAiMessageContent};

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
    fn preserves_content_exactly() {
        let content = "This is a test message with special chars: !@#$%^&*()";
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: content.to_string(),
            parts: Vec::new(),
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        assert_openai_text_content(&openai_msg.content, content);
    }

    #[test]
    fn handles_empty_content() {
        let ai_message = AiMessage {
            role: MessageRole::User,
            content: String::new(),
            parts: Vec::new(),
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        assert_openai_text_content(&openai_msg.content, "");
    }

    #[test]
    fn handles_multiline_content() {
        let content = "Line 1\nLine 2\nLine 3";
        let ai_message = AiMessage {
            role: MessageRole::Assistant,
            content: content.to_string(),
            parts: Vec::new(),
        };

        let openai_msg: OpenAiMessage = (&ai_message).into();

        assert_openai_text_content(&openai_msg.content, content);
    }
}

mod anthropic_converter_tests {
    use super::*;
    use systemprompt_core_ai::models::providers::anthropic::AnthropicContent;

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
        // Note: Anthropic maps system to assistant role
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
        let content = "Complex content with unicode: 你好 🎉";
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

mod gemini_converter_tests {
    use super::*;
    use systemprompt_core_ai::models::providers::gemini::GeminiPart;

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
        // Note: Gemini maps system to user role
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
    use systemprompt_core_ai::models::providers::anthropic::AnthropicContent;
    use systemprompt_core_ai::models::providers::gemini::GeminiPart;

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
        assert_eq!(gemini.role, "model"); // Gemini uses "model" for assistant
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

        // OpenAI preserves system role
        assert_eq!(openai.role, "system");
        // Anthropic maps to assistant (system handled separately)
        assert_eq!(anthropic.role, "assistant");
        // Gemini maps to user
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
}
