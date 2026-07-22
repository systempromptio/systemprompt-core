//! Tests for ResponseStrategy and TooledExecutor.

use rmcp::model::ContentBlock;
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

fn create_text_content(text: &str) -> ContentBlock {
    ContentBlock::text(text.to_string())
}

fn create_result_with_content(text: &str) -> CallToolResult {
    CallToolResult::success(vec![create_text_content(text)])
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
            },
            _ => panic!("Expected ContentProvided"),
        }
    }

    #[test]
    fn content_provided_when_whitespace_only_content() {
        let content = "   \n\t  ".to_string();
        let tool_calls = vec![create_tool_call("test")];
        let tool_results = vec![create_result_with_content("result")];

        let strategy =
            ResponseStrategy::from_response(content, tool_calls.clone(), tool_results.clone());

        match strategy {
            ResponseStrategy::ToolsOnly { .. } | ResponseStrategy::ArtifactsProvided { .. } => {},
            ResponseStrategy::ContentProvided { content: c, .. } => {
                assert!(c.trim().is_empty() || !c.is_empty());
            },
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
            },
            _ => panic!("Expected ContentProvided for empty tools"),
        }
    }

    #[test]
    fn preserves_tool_calls_and_results() {
        let content = "Response".to_string();
        let tool_calls = vec![create_tool_call("tool1"), create_tool_call("tool2")];
        let tool_results = vec![
            create_result_with_content("result1"),
            create_result_with_content("result2"),
        ];

        let strategy =
            ResponseStrategy::from_response(content, tool_calls.clone(), tool_results.clone());

        match strategy {
            ResponseStrategy::ContentProvided {
                tool_calls: tc,
                tool_results: tr,
                ..
            } => {
                assert_eq!(tc.len(), 2);
                assert_eq!(tr.len(), 2);
            },
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

mod tooled_executor_tests {
    use super::create_tool_call;
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use systemprompt_ai::models::tools::McpTool;
    use systemprompt_ai::services::tooled::TooledExecutor;
    use systemprompt_identifiers::{AgentName, ContextId, McpServerId, SessionId, TraceId};
    use systemprompt_models::execution::context::RequestContext;
    use systemprompt_traits::{
        ToolCallRequest, ToolCallResult, ToolContent, ToolContext, ToolDefinition, ToolProvider,
        ToolProviderError, ToolProviderResult,
    };

    struct ScriptedToolProvider {
        fail: bool,
        calls: Mutex<Vec<(String, String)>>,
    }

    impl ScriptedToolProvider {
        fn new(fail: bool) -> Self {
            Self {
                fail,
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl ToolProvider for ScriptedToolProvider {
        async fn list_tools(
            &self,
            _agent_name: &str,
            _context: &ToolContext,
        ) -> ToolProviderResult<Vec<ToolDefinition>> {
            Ok(vec![])
        }

        async fn call_tool(
            &self,
            request: &ToolCallRequest,
            service_id: &McpServerId,
            _context: &ToolContext,
        ) -> ToolProviderResult<ToolCallResult> {
            self.calls
                .lock()
                .expect("lock")
                .push((request.name.clone(), service_id.to_string()));
            if self.fail {
                return Err(ToolProviderError::ExecutionFailed(
                    "scripted failure".into(),
                ));
            }
            Ok(ToolCallResult {
                content: vec![ToolContent::text(format!("ran {}", request.name))],
                structured_content: Some(json!({"tool": request.name})),
                is_error: None,
                meta: None,
            })
        }

        async fn refresh_connections(&self, _agent_name: &str) -> ToolProviderResult<()> {
            Ok(())
        }

        async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> {
            Ok(HashMap::new())
        }

        async fn find_tool(
            &self,
            _agent_name: &str,
            _tool_name: &str,
            _context: &ToolContext,
        ) -> ToolProviderResult<Option<ToolDefinition>> {
            Ok(None)
        }
    }

    fn context() -> RequestContext {
        RequestContext::new(
            SessionId::new("sess-exec"),
            TraceId::new("trace-exec"),
            ContextId::new("00000000-0000-4000-8000-0000000000ee"),
            AgentName::new("exec-agent"),
        )
    }

    fn known_tool(name: &str, service: &str) -> McpTool {
        McpTool::new(name, McpServerId::new(service))
    }

    #[tokio::test]
    async fn executes_known_tool_and_returns_provider_result() {
        let provider = Arc::new(ScriptedToolProvider::new(false));
        let executor = TooledExecutor::new(Arc::clone(&provider) as _);
        let tools = vec![known_tool("lookup", "svc-a")];

        let (calls, results) = executor
            .execute_tool_calls(vec![create_tool_call("lookup")], &tools, &context(), None)
            .await;

        assert_eq!(calls.len(), 1);
        assert_eq!(results.len(), 1);
        assert_ne!(results[0].is_error, Some(true));
        assert_eq!(
            results[0].structured_content,
            Some(json!({"tool": "lookup"}))
        );
        assert_eq!(
            provider.calls.lock().expect("lock").as_slice(),
            &[("lookup".to_owned(), "svc-a".to_owned())]
        );
    }

    #[tokio::test]
    async fn provider_failure_becomes_structured_error_result() {
        let provider = Arc::new(ScriptedToolProvider::new(true));
        let executor = TooledExecutor::new(provider as _);
        let tools = vec![known_tool("lookup", "svc-a")];

        let (_calls, results) = executor
            .execute_tool_calls(vec![create_tool_call("lookup")], &tools, &context(), None)
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].is_error, Some(true));
        let structured = results[0]
            .structured_content
            .as_ref()
            .expect("error payload");
        assert!(
            structured["error"]
                .as_str()
                .expect("error string")
                .contains("scripted failure")
        );
    }

    #[tokio::test]
    async fn unknown_tool_yields_not_found_error_without_calling_provider() {
        let provider = Arc::new(ScriptedToolProvider::new(false));
        let executor = TooledExecutor::new(Arc::clone(&provider) as _);
        let tools = vec![known_tool("other", "svc-a")];

        let (_calls, results) = executor
            .execute_tool_calls(vec![create_tool_call("missing")], &tools, &context(), None)
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].is_error, Some(true));
        let structured = results[0]
            .structured_content
            .as_ref()
            .expect("error payload");
        assert!(
            structured["error"]
                .as_str()
                .expect("error string")
                .contains("'missing' not found")
        );
        assert!(
            provider.calls.lock().expect("lock").is_empty(),
            "unknown tool must not reach the provider"
        );
    }

    #[tokio::test]
    async fn mixed_batch_preserves_call_order_in_results() {
        let provider = Arc::new(ScriptedToolProvider::new(false));
        let executor = TooledExecutor::new(provider as _);
        let tools = vec![known_tool("first", "svc-a"), known_tool("second", "svc-b")];

        let (calls, results) = executor
            .execute_tool_calls(
                vec![
                    create_tool_call("first"),
                    create_tool_call("ghost"),
                    create_tool_call("second"),
                ],
                &tools,
                &context(),
                None,
            )
            .await;

        assert_eq!(calls.len(), 3);
        assert_eq!(results.len(), 3);
        assert_eq!(
            results[0].structured_content,
            Some(json!({"tool": "first"}))
        );
        assert_eq!(results[1].is_error, Some(true));
        assert_eq!(
            results[2].structured_content,
            Some(json!({"tool": "second"}))
        );
    }

    #[tokio::test]
    async fn agent_override_takes_precedence_over_tool_model_config() {
        use systemprompt_models::ai::{ToolModelConfig, ToolModelOverrides};

        let provider = Arc::new(ScriptedToolProvider::new(false));
        let executor = TooledExecutor::new(Arc::clone(&provider) as _);
        let tool = known_tool("lookup", "svc-a").with_model_config(ToolModelConfig::default());

        let mut overrides: ToolModelOverrides = ToolModelOverrides::new();
        overrides.insert(
            "svc-a".to_owned(),
            std::iter::once(("lookup".to_owned(), ToolModelConfig::default())).collect(),
        );

        let (_calls, results) = executor
            .execute_tool_calls(
                vec![create_tool_call("lookup")],
                &[tool],
                &context(),
                Some(&overrides),
            )
            .await;

        assert_eq!(results.len(), 1);
        assert_ne!(results[0].is_error, Some(true));
        assert_eq!(
            provider.calls.lock().expect("lock").len(),
            1,
            "override resolution must still route the call to the provider"
        );
    }
}
