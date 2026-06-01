use rmcp::model::{Annotated, RawContent, RawTextContent};
use systemprompt_ai::models::ai::{AiMessage, MessageRole, SamplingParams};
use systemprompt_ai::models::tools::{CallToolResult, McpTool, ToolCall};
use systemprompt_ai::services::providers::gemini::{ToolRequestParams, ToolResultParams};
use systemprompt_identifiers::{AiToolCallId, McpServerId};

fn user_msg(text: &str) -> AiMessage {
    AiMessage {
        role: MessageRole::User,
        content: text.to_owned(),
        parts: Vec::new(),
    }
}

fn make_tool(name: &str) -> McpTool {
    McpTool::new(name, McpServerId::new("svc"))
}

fn make_tool_call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("call-{name}")),
        name: name.to_owned(),
        arguments: serde_json::json!({"input": "value"}),
    }
}

fn make_tool_result(content: &str) -> CallToolResult {
    CallToolResult::success(vec![Annotated {
        raw: RawContent::Text(RawTextContent {
            text: content.to_owned(),
            meta: None,
        }),
        annotations: None,
    }])
}

mod tool_request_params_tests {
    use super::*;

    #[test]
    fn builder_minimal() {
        let messages = vec![user_msg("hi")];
        let tools = vec![make_tool("my_tool")];
        let params = ToolRequestParams::builder(&messages, &tools, 512, "gemini-2.5-flash").build();
        assert_eq!(params.messages.len(), 1);
        assert_eq!(params.tools.len(), 1);
        assert_eq!(params.max_output_tokens, 512);
        assert_eq!(params.model, "gemini-2.5-flash");
        assert!(params.sampling.is_none());
    }

    #[test]
    fn builder_with_sampling() {
        let messages = vec![user_msg("hello")];
        let tools = vec![];
        let sampling = SamplingParams {
            temperature: Some(0.8),
            top_p: Some(0.9),
            top_k: Some(40),
            stop_sequences: None,
            presence_penalty: None,
            frequency_penalty: None,
        };
        let params = ToolRequestParams::builder(&messages, &tools, 1024, "gemini-2.5-flash")
            .with_sampling(&sampling)
            .build();
        assert!(params.sampling.is_some());
        let s = params.sampling.unwrap();
        assert!((s.temperature.unwrap() - 0.8).abs() < 1e-6);
    }

    #[test]
    fn direct_constructor_matches_builder() {
        let messages = vec![user_msg("test")];
        let tools = vec![make_tool("t1"), make_tool("t2")];
        let params_direct = ToolRequestParams::builder(&messages, &tools, 256, "model-x").build();
        assert_eq!(params_direct.messages.len(), 1);
        assert_eq!(params_direct.tools.len(), 2);
        assert_eq!(params_direct.max_output_tokens, 256);
        assert_eq!(params_direct.model, "model-x");
    }

    #[test]
    fn multiple_messages() {
        let messages = vec![
            AiMessage {
                role: MessageRole::System,
                content: "You are an assistant.".to_owned(),
                parts: Vec::new(),
            },
            user_msg("What's 2+2?"),
        ];
        let tools = vec![];
        let params = ToolRequestParams::builder(&messages, &tools, 128, "gemini-flash").build();
        assert_eq!(params.messages.len(), 2);
    }

    #[test]
    fn max_tokens_propagates() {
        let messages = vec![user_msg("test")];
        let tools = vec![];
        let params = ToolRequestParams::builder(&messages, &tools, 4096, "gemini-flash").build();
        assert_eq!(params.max_output_tokens, 4096);
    }
}

mod tool_result_params_tests {
    use super::*;

    #[test]
    fn builder_minimal() {
        let history = vec![user_msg("what's the weather?")];
        let calls = vec![make_tool_call("get_weather")];
        let results = vec![make_tool_result("sunny")];
        let params =
            ToolResultParams::builder(&history, &calls, &results, 512, "gemini-2.5-flash").build();
        assert_eq!(params.conversation_history.len(), 1);
        assert_eq!(params.tool_calls.len(), 1);
        assert_eq!(params.tool_results.len(), 1);
        assert_eq!(params.max_output_tokens, 512);
        assert_eq!(params.model, "gemini-2.5-flash");
        assert!(params.sampling.is_none());
    }

    #[test]
    fn builder_with_sampling() {
        let history = vec![];
        let calls = vec![];
        let results = vec![];
        let sampling = SamplingParams {
            temperature: Some(0.5),
            top_p: None,
            top_k: None,
            stop_sequences: None,
            presence_penalty: None,
            frequency_penalty: None,
        };
        let params =
            ToolResultParams::builder(&history, &calls, &results, 256, "gemini-flash-lite")
                .with_sampling(&sampling)
                .build();
        assert!(params.sampling.is_some());
        let s = params.sampling.unwrap();
        assert!((s.temperature.unwrap() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn multiple_tool_calls_and_results() {
        let history = vec![user_msg("use multiple tools")];
        let calls = vec![make_tool_call("tool_a"), make_tool_call("tool_b")];
        let results = vec![make_tool_result("result_a"), make_tool_result("result_b")];
        let params =
            ToolResultParams::builder(&history, &calls, &results, 1024, "gemini-2.5-flash").build();
        assert_eq!(params.tool_calls.len(), 2);
        assert_eq!(params.tool_results.len(), 2);
    }

    #[test]
    fn empty_history() {
        let history: Vec<AiMessage> = vec![];
        let calls = vec![make_tool_call("my_tool")];
        let results = vec![make_tool_result("ok")];
        let params = ToolResultParams::builder(&history, &calls, &results, 512, "model").build();
        assert!(params.conversation_history.is_empty());
        assert_eq!(params.tool_calls.len(), 1);
    }
}
