//! Tests for Gemini model types.

use systemprompt_ai::models::providers::gemini::{GeminiContent, GeminiPart};

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
}
