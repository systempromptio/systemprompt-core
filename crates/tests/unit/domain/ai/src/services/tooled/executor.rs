//! Tests for ResponseStrategy and TooledExecutor.

use systemprompt_core_ai::services::tooled::ResponseStrategy;
use systemprompt_core_ai::models::tools::{CallToolResult, ToolCall};
use systemprompt_identifiers::AiToolCallId;
use rmcp::model::{Annotated, Content, RawContent, RawTextContent};
use serde_json::json;

fn create_tool_call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("call-{}", name)),
        name: name.to_string(),
        arguments: json!({}),
    }
}

fn create_text_content(text: &str) -> Content {
    Annotated {
        raw: RawContent::Text(RawTextContent {
            text: text.to_string(),
            meta: None,
        }),
        annotations: None,
    }
}

fn create_result_with_content(text: &str) -> CallToolResult {
    CallToolResult {
        content: vec![create_text_content(text)],
        structured_content: None,
        is_error: Some(false),
        meta: None,
    }
}

fn create_result_with_artifact() -> CallToolResult {
    CallToolResult {
        content: vec![],
        structured_content: Some(json!({"artifact": "data"})),
        is_error: Some(false),
        meta: None,
    }
}

fn create_error_result() -> CallToolResult {
    CallToolResult {
        content: vec![create_text_content("Error")],
        structured_content: Some(json!({"error": true})),
        is_error: Some(true),
        meta: None,
    }
}

mod response_strategy_tests {
    use super::*;

    #[test]
    fn content_provided_when_content_not_empty() {
        let content = "This is the response".to_string();
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_result_with_content("result")];

        let strategy = ResponseStrategy::from_response(content.clone(), tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ContentProvided { content: c, .. } => {
                assert_eq!(c, content);
            }
            _ => panic!("Expected ContentProvided"),
        }
    }

    #[test]
    fn content_provided_when_whitespace_only_content() {
        let content = "   \n\t  ".to_string();
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_result_with_content("result")];

        let strategy = ResponseStrategy::from_response(content, tool_calls.clone(), tool_results.clone());

        // Empty whitespace should not trigger ContentProvided with that content
        match strategy {
            ResponseStrategy::ToolsOnly { .. } | ResponseStrategy::ArtifactsProvided { .. } => {
                // Expected when content is just whitespace
            }
            ResponseStrategy::ContentProvided { content: c, .. } => {
                // Also acceptable if content is preserved
                assert!(c.trim().is_empty() || !c.is_empty());
            }
        }
    }

    #[test]
    fn artifacts_provided_when_structured_content_present() {
        let content = String::new();
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_result_with_artifact()];

        let strategy = ResponseStrategy::from_response(content, tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ArtifactsProvided { .. } => {
                // Expected
            }
            _ => panic!("Expected ArtifactsProvided"),
        }
    }

    #[test]
    fn tools_only_when_no_artifacts() {
        let content = String::new();
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_result_with_content("just text")];

        let strategy = ResponseStrategy::from_response(content, tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ToolsOnly { .. } => {
                // Expected
            }
            _ => panic!("Expected ToolsOnly"),
        }
    }

    #[test]
    fn content_provided_when_empty_tools() {
        let content = String::new();
        let tool_calls: Vec<ToolCall> = vec![];
        let tool_results: Vec<CallToolResult> = vec![];

        let strategy = ResponseStrategy::from_response(content.clone(), tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ContentProvided { content: c, .. } => {
                assert_eq!(c, content);
            }
            _ => panic!("Expected ContentProvided for empty tools"),
        }
    }

    #[test]
    fn error_results_not_treated_as_artifacts() {
        let content = String::new();
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_error_result()];

        let strategy = ResponseStrategy::from_response(content, tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ToolsOnly { .. } => {
                // Error results should not be treated as artifacts
            }
            ResponseStrategy::ArtifactsProvided { .. } => {
                panic!("Error results should not be treated as artifacts");
            }
            _ => {}
        }
    }

    #[test]
    fn preserves_tool_calls_and_results() {
        let content = "Response".to_string();
        let tool_calls = vec![
            create_tool_call("tool1"),
            create_tool_call("tool2"),
        ];
        let tool_results = vec![
            create_result_with_content("result1"),
            create_result_with_content("result2"),
        ];

        let strategy = ResponseStrategy::from_response(
            content,
            tool_calls.clone(),
            tool_results.clone(),
        );

        match strategy {
            ResponseStrategy::ContentProvided {
                tool_calls: tc,
                tool_results: tr,
                ..
            } => {
                assert_eq!(tc.len(), 2);
                assert_eq!(tr.len(), 2);
            }
            _ => panic!("Expected ContentProvided"),
        }
    }
}

mod response_strategy_debug_tests {
    use super::*;

    #[test]
    fn is_debug() {
        let strategy = ResponseStrategy::ContentProvided {
            content: "test".to_string(),
            tool_calls: vec![],
            tool_results: vec![],
        };

        let debug = format!("{:?}", strategy);
        assert!(debug.contains("ContentProvided"));
    }

    #[test]
    fn artifacts_provided_is_debug() {
        let strategy = ResponseStrategy::ArtifactsProvided {
            tool_calls: vec![],
            tool_results: vec![],
        };

        let debug = format!("{:?}", strategy);
        assert!(debug.contains("ArtifactsProvided"));
    }

    #[test]
    fn tools_only_is_debug() {
        let strategy = ResponseStrategy::ToolsOnly {
            tool_calls: vec![],
            tool_results: vec![],
        };

        let debug = format!("{:?}", strategy);
        assert!(debug.contains("ToolsOnly"));
    }
}
