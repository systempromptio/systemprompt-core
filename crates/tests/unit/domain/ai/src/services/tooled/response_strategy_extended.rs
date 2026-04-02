use rmcp::model::{Annotated, Content, RawContent, RawTextContent};
use serde_json::json;
use systemprompt_ai::models::tools::{CallToolResult, ToolCall};
use systemprompt_ai::services::tooled::ResponseStrategy;
use systemprompt_identifiers::AiToolCallId;

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

fn create_success_result(text: &str) -> CallToolResult {
    CallToolResult::success(vec![create_text_content(text)])
}

fn create_result_with_structured_content() -> CallToolResult {
    let mut result = CallToolResult::success(vec![create_text_content("artifact data")]);
    result.structured_content = Some(json!({"type": "table", "data": []}));
    result
}

fn create_error_result_with_structured_content() -> CallToolResult {
    let mut result = CallToolResult::error(vec![create_text_content("error")]);
    result.structured_content = Some(json!({"error": "details"}));
    result
}

mod artifacts_provided_tests {
    use super::*;

    #[test]
    fn artifacts_provided_when_structured_content_present() {
        let content = String::new();
        let tool_calls = vec![create_tool_call("render")];
        let tool_results = vec![create_result_with_structured_content()];

        let strategy = ResponseStrategy::from_response(content, tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ArtifactsProvided {
                tool_calls: tc,
                tool_results: tr,
            } => {
                assert_eq!(tc.len(), 1);
                assert_eq!(tr.len(), 1);
            },
            other => panic!("Expected ArtifactsProvided, got: {:?}", other),
        }
    }

    #[test]
    fn tools_only_when_structured_content_is_error() {
        let content = String::new();
        let tool_calls = vec![create_tool_call("render")];
        let tool_results = vec![create_error_result_with_structured_content()];

        let strategy = ResponseStrategy::from_response(content, tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ToolsOnly { .. } => {},
            other => panic!("Expected ToolsOnly, got: {:?}", other),
        }
    }

    #[test]
    fn content_provided_takes_priority_over_artifacts() {
        let content = "Here is your response".to_string();
        let tool_calls = vec![create_tool_call("render")];
        let tool_results = vec![create_result_with_structured_content()];

        let strategy = ResponseStrategy::from_response(content.clone(), tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ContentProvided { content: c, .. } => {
                assert_eq!(c, content);
            },
            other => panic!("Expected ContentProvided, got: {:?}", other),
        }
    }

    #[test]
    fn tools_only_when_no_structured_content() {
        let content = String::new();
        let tool_calls = vec![create_tool_call("search")];
        let tool_results = vec![create_success_result("plain result")];

        let strategy = ResponseStrategy::from_response(content, tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ToolsOnly { .. } => {},
            other => panic!("Expected ToolsOnly, got: {:?}", other),
        }
    }
}

mod edge_case_tests {
    use super::*;

    #[test]
    fn empty_content_empty_tools_returns_content_provided() {
        let strategy = ResponseStrategy::from_response(
            String::new(),
            vec![],
            vec![],
        );

        match strategy {
            ResponseStrategy::ContentProvided { content, tool_calls, tool_results } => {
                assert!(content.is_empty());
                assert!(tool_calls.is_empty());
                assert!(tool_results.is_empty());
            },
            other => panic!("Expected ContentProvided, got: {:?}", other),
        }
    }

    #[test]
    fn whitespace_only_content_with_tools_is_not_content_provided() {
        let content = "   \n\t  ".to_string();
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_success_result("result")];

        let strategy = ResponseStrategy::from_response(content, tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ToolsOnly { .. } | ResponseStrategy::ArtifactsProvided { .. } => {},
            ResponseStrategy::ContentProvided { .. } => {
                panic!("Whitespace-only content with tools should not be ContentProvided");
            },
        }
    }

    #[test]
    fn multiple_tools_with_mixed_structured_content() {
        let content = String::new();
        let tool_calls = vec![
            create_tool_call("search"),
            create_tool_call("render"),
        ];
        let plain_result = create_success_result("search result");
        let artifact_result = create_result_with_structured_content();

        let strategy = ResponseStrategy::from_response(
            content,
            tool_calls,
            vec![plain_result, artifact_result],
        );

        match strategy {
            ResponseStrategy::ArtifactsProvided { .. } => {},
            other => panic!("Expected ArtifactsProvided when any result has structured content, got: {:?}", other),
        }
    }

    #[test]
    fn single_character_content_is_content_provided() {
        let content = "x".to_string();
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_success_result("result")];

        let strategy = ResponseStrategy::from_response(content, tool_calls, tool_results);

        match strategy {
            ResponseStrategy::ContentProvided { content: c, .. } => {
                assert_eq!(c, "x");
            },
            other => panic!("Expected ContentProvided, got: {:?}", other),
        }
    }

    #[test]
    fn tools_with_empty_results_vec_returns_content_provided() {
        let strategy = ResponseStrategy::from_response(
            String::new(),
            vec![create_tool_call("test")],
            vec![],
        );

        match strategy {
            ResponseStrategy::ContentProvided { .. } => {},
            other => panic!("Expected ContentProvided when results empty, got: {:?}", other),
        }
    }

    #[test]
    fn empty_tools_with_results_returns_content_provided() {
        let strategy = ResponseStrategy::from_response(
            String::new(),
            vec![],
            vec![create_success_result("orphan")],
        );

        match strategy {
            ResponseStrategy::ContentProvided { .. } => {},
            other => panic!("Expected ContentProvided when tool_calls empty, got: {:?}", other),
        }
    }
}
