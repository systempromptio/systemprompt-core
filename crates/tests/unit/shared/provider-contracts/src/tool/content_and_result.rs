//! Tests for ToolContent and ToolCallResult.

use systemprompt_provider_contracts::{ToolCallResult, ToolContent};

mod tool_content_tests {
    use super::*;

    #[test]
    fn text_constructor() {
        let content = ToolContent::text("Hello");
        match content {
            ToolContent::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn text_variant() {
        let content = ToolContent::Text {
            text: "test".to_string(),
        };
        if let ToolContent::Text { text } = content {
            assert_eq!(text, "test");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn image_variant() {
        let content = ToolContent::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        };
        if let ToolContent::Image { data, mime_type } = content {
            assert_eq!(data, "base64data");
            assert_eq!(mime_type, "image/png");
        } else {
            panic!("Expected Image variant");
        }
    }

    #[test]
    fn resource_variant() {
        let content = ToolContent::Resource {
            uri: "file://test.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
        };
        if let ToolContent::Resource { uri, mime_type } = content {
            assert_eq!(uri, "file://test.txt");
            assert_eq!(mime_type, Some("text/plain".to_string()));
        } else {
            panic!("Expected Resource variant");
        }
    }

    #[test]
    fn resource_variant_without_mime() {
        let content = ToolContent::Resource {
            uri: "file://test.txt".to_string(),
            mime_type: None,
        };
        if let ToolContent::Resource { mime_type, .. } = content {
            assert!(mime_type.is_none());
        } else {
            panic!("Expected Resource variant");
        }
    }

    #[test]
    fn is_serializable() {
        let content = ToolContent::text("test");
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("text"));
    }

    #[test]
    fn is_debug() {
        let content = ToolContent::text("test");
        let debug = format!("{:?}", content);
        assert!(debug.contains("Text"));
    }
}

mod tool_call_result_tests {
    use super::*;

    #[test]
    fn success_creates_non_error() {
        let result = ToolCallResult::success("Done");
        assert_eq!(result.is_error, Some(false));
    }

    #[test]
    fn success_has_text_content() {
        let result = ToolCallResult::success("Done");
        assert_eq!(result.content.len(), 1);
        if let ToolContent::Text { text } = &result.content[0] {
            assert_eq!(text, "Done");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn success_has_no_structured_content() {
        let result = ToolCallResult::success("Done");
        assert!(result.structured_content.is_none());
    }

    #[test]
    fn success_has_no_meta() {
        let result = ToolCallResult::success("Done");
        assert!(result.meta.is_none());
    }

    #[test]
    fn error_creates_error_flag() {
        let result = ToolCallResult::error("Failed");
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn error_has_text_content() {
        let result = ToolCallResult::error("Failed");
        assert_eq!(result.content.len(), 1);
        if let ToolContent::Text { text } = &result.content[0] {
            assert_eq!(text, "Failed");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn with_structured_content() {
        let result =
            ToolCallResult::success("ok").with_structured_content(serde_json::json!({"key": 1}));
        assert_eq!(
            result
                .structured_content
                .expect("structured_content should be set")["key"],
            1
        );
    }

    #[test]
    fn is_serializable() {
        let result = ToolCallResult::success("test");
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("test"));
    }

    #[test]
    fn is_debug() {
        let result = ToolCallResult::success("test");
        let debug = format!("{:?}", result);
        assert!(debug.contains("ToolCallResult"));
    }
}
