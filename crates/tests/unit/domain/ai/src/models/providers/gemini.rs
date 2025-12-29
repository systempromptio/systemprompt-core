//! Tests for Gemini model types.

use systemprompt_core_ai::models::providers::gemini::{
    GeminiContent, GeminiModels, GeminiPart,
};

mod gemini_models_tests {
    use super::*;

    #[test]
    fn default_models_have_correct_ids() {
        let models = GeminiModels::default();

        assert!(models.gemini_flash_lite.id.contains("flash-lite"));
        assert!(models.gemini_flash.id.contains("flash"));
    }

    #[test]
    fn default_models_have_large_context() {
        let models = GeminiModels::default();

        // Gemini models support 1M tokens
        assert_eq!(models.gemini_flash_lite.max_tokens, 1_000_000);
        assert_eq!(models.gemini_flash.max_tokens, 1_000_000);
    }

    #[test]
    fn default_models_support_tools() {
        let models = GeminiModels::default();

        assert!(models.gemini_flash_lite.supports_tools);
        assert!(models.gemini_flash.supports_tools);
    }

    #[test]
    fn flash_lite_is_cheaper() {
        let models = GeminiModels::default();

        assert!(models.gemini_flash_lite.cost_per_1k_tokens < models.gemini_flash.cost_per_1k_tokens);
    }
}

mod gemini_content_tests {
    use super::*;

    #[test]
    fn create_user_content() {
        let content = GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart::Text {
                text: "Hello Gemini!".to_string(),
            }],
        };

        assert_eq!(content.role, "user");
        assert_eq!(content.parts.len(), 1);
    }

    #[test]
    fn create_model_content() {
        let content = GeminiContent {
            role: "model".to_string(),
            parts: vec![GeminiPart::Text {
                text: "Hello! How can I help?".to_string(),
            }],
        };

        assert_eq!(content.role, "model");
    }

    #[test]
    fn content_with_multiple_parts() {
        let content = GeminiContent {
            role: "user".to_string(),
            parts: vec![
                GeminiPart::Text {
                    text: "First part".to_string(),
                },
                GeminiPart::Text {
                    text: "Second part".to_string(),
                },
            ],
        };

        assert_eq!(content.parts.len(), 2);
    }

    #[test]
    fn content_serialization() {
        let content = GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart::Text {
                text: "Test message".to_string(),
            }],
        };

        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Test message"));
    }
}

mod gemini_part_tests {
    use super::*;

    #[test]
    fn text_part_creation() {
        let part = GeminiPart::Text {
            text: "Hello world".to_string(),
        };

        match part {
            GeminiPart::Text { text } => assert_eq!(text, "Hello world"),
            _ => panic!("Expected Text part"),
        }
    }

    #[test]
    fn text_part_serialization() {
        let part = GeminiPart::Text {
            text: "Test text".to_string(),
        };

        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("text"));
        assert!(json.contains("Test text"));
    }

    #[test]
    fn text_part_deserialization() {
        let json = r#"{"text": "Deserialized text"}"#;
        let part: GeminiPart = serde_json::from_str(json).unwrap();

        match part {
            GeminiPart::Text { text } => assert_eq!(text, "Deserialized text"),
            _ => panic!("Expected Text part"),
        }
    }

    #[test]
    fn text_part_clone() {
        let part = GeminiPart::Text {
            text: "Original".to_string(),
        };
        let cloned = part.clone();

        match cloned {
            GeminiPart::Text { text } => assert_eq!(text, "Original"),
            _ => panic!("Expected Text part"),
        }
    }
}

mod gemini_content_clone_tests {
    use super::*;

    #[test]
    fn content_is_cloneable() {
        let content = GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart::Text {
                text: "Clone me".to_string(),
            }],
        };

        let cloned = content.clone();
        assert_eq!(content.role, cloned.role);
        assert_eq!(content.parts.len(), cloned.parts.len());
    }
}
