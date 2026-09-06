//! Tests for AI request record types.

use systemprompt_ai::models::{
    AiRequestRecord, AiRequestRecordBuilder, CacheInfo, RequestStatus, TokenInfo,
};
use systemprompt_identifiers::{
    Actor, ActorKind, AiRequestId, ContextId, GatewayConversationId, McpExecutionId,
    ProviderRequestId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_test_fixtures::{fixture_user_id, usage};

const TEST_CONTEXT_ID_A: &str = "00000000-0000-4000-8000-000000000001";

mod token_info_tests {
    use super::*;

    #[test]
    fn default_token_info_has_none_values() {
        let info = TokenInfo::default();
        assert!(info.tokens_used.is_none());
        assert!(info.input_tokens.is_none());
        assert!(info.output_tokens.is_none());
        assert!(info.reasoning_tokens.is_none());
    }

    #[test]
    fn token_info_can_be_created_with_values() {
        let info = TokenInfo {
            tokens_used: Some(1500),
            input_tokens: Some(1000),
            output_tokens: Some(500),
            reasoning_tokens: Some(120),
        };
        assert_eq!(info.reasoning_tokens, Some(120));
        assert_eq!(info.tokens_used, Some(1500));
        assert_eq!(info.input_tokens, Some(1000));
        assert_eq!(info.output_tokens, Some(500));
    }
}

mod cache_info_tests {
    use super::*;

    #[test]
    fn default_cache_info_has_false_hit() {
        let info = CacheInfo::default();
        assert!(!info.hit);
        assert!(info.read_tokens.is_none());
        assert!(info.creation_tokens.is_none());
    }

    #[test]
    fn cache_info_with_cache_hit() {
        let info = CacheInfo {
            hit: true,
            read_tokens: Some(500),
            creation_tokens: Some(100),
        };
        assert!(info.hit);
        assert_eq!(info.read_tokens, Some(500));
        assert_eq!(info.creation_tokens, Some(100));
    }
}

mod request_status_tests {
    use super::*;

    #[test]
    fn pending_status_as_str() {
        assert_eq!(RequestStatus::Pending.as_str(), "pending");
    }

    #[test]
    fn completed_status_as_str() {
        assert_eq!(RequestStatus::Completed.as_str(), "completed");
    }

    #[test]
    fn failed_status_as_str() {
        assert_eq!(RequestStatus::Failed.as_str(), "failed");
    }

    // The `ai_requests_routed_has_provider` constraint is keyed on this exact
    // string; drifting it silently makes every rejection insert fail.
    #[test]
    fn rejected_status_as_str() {
        assert_eq!(RequestStatus::Rejected.as_str(), "rejected");
    }
}

mod ai_request_record_builder_tests {
    use super::*;

    fn test_user_id() -> UserId {
        fixture_user_id()
    }

    #[test]
    fn builder_leaves_provider_none_when_routing_never_resolved() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .model("gpt-4")
        .build();

        assert_eq!(record.provider, None);
        assert_eq!(record.model.as_deref(), Some("gpt-4"));
    }

    #[test]
    fn builder_leaves_model_none_when_the_body_was_never_read() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .build();

        assert_eq!(record.model, None);
        assert_eq!(record.provider.as_deref(), Some("openai"));
    }

    // `rejected()` carries no error message of its own — the rejection path
    // stamps one afterwards via `update_error` — unlike `failed()`.
    #[test]
    fn builder_marks_a_pre_routing_rejection_without_an_error_message() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .rejected()
        .build();

        assert_eq!(record.status, RequestStatus::Rejected);
        assert_eq!(record.provider, None);
        assert_eq!(record.model, None);
        assert_eq!(record.error_message, None);
    }

    #[test]
    fn builder_creates_record_with_required_fields() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .build();

        assert_eq!(record.request_id, "req-123");
        assert_eq!(record.provider.as_deref(), Some("openai"));
        assert_eq!(record.model.as_deref(), Some("gpt-4"));
        assert_eq!(record.status, RequestStatus::Pending);
    }

    #[test]
    fn builder_sets_session_id() {
        let session_id = SessionId::new("session-456");
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("anthropic")
        .model("claude-3")
        .session_id(session_id.clone())
        .build();

        assert_eq!(record.session_id, Some(session_id));
    }

    #[test]
    fn builder_sets_task_id() {
        let task_id = TaskId::new("task-789");
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("gemini")
        .model("gemini-pro")
        .task_id(task_id.clone())
        .build();

        assert_eq!(record.task_id, Some(task_id));
    }

    #[test]
    fn builder_sets_context_id() {
        let context_id = ContextId::new_unchecked(TEST_CONTEXT_ID_A);
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .build();

        assert_eq!(record.context_id, context_id);
    }

    #[test]
    fn builder_sets_trace_id() {
        let trace_id = TraceId::new("trace-xyz");
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .trace_id(trace_id.clone())
        .build();

        assert_eq!(record.trace_id, Some(trace_id));
    }

    #[test]
    fn builder_sets_max_tokens() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .max_tokens(4096)
        .build();

        assert_eq!(record.max_tokens, Some(4096));
    }

    #[test]
    fn builder_sets_tokens() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .usage(Some(usage().input(1000).output(500).build()))
        .build();

        assert_eq!(record.tokens.input_tokens, Some(1000));
        assert_eq!(record.tokens.output_tokens, Some(500));
        assert_eq!(record.tokens.tokens_used, Some(1500));
    }

    #[test]
    fn builder_tokens_with_only_input() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .usage(Some(usage().input(1000).build()))
        .build();

        assert_eq!(record.tokens.input_tokens, Some(1000));
        assert_eq!(record.tokens.output_tokens, Some(0));
        assert_eq!(record.tokens.tokens_used, Some(1000));
    }

    #[test]
    fn builder_tokens_with_only_output() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .usage(Some(usage().output(500).build()))
        .build();

        assert_eq!(record.tokens.input_tokens, Some(0));
        assert_eq!(record.tokens.output_tokens, Some(500));
        assert_eq!(record.tokens.tokens_used, Some(500));
    }

    #[test]
    fn builder_sets_cache_info() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("anthropic")
        .model("claude-3")
        .usage(Some(usage().cache_read(500).cache_creation(100).build()))
        .build();

        assert!(record.cache.hit);
        assert_eq!(record.cache.read_tokens, Some(500));
        assert_eq!(record.cache.creation_tokens, Some(100));
    }

    #[test]
    fn builder_sets_streaming() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .streaming(true)
        .build();

        assert!(record.is_streaming);
    }

    #[test]
    fn builder_sets_cost() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .cost(150)
        .build();

        assert_eq!(record.cost_microdollars, 150);
    }

    #[test]
    fn builder_sets_latency() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .latency(250)
        .build();

        assert_eq!(record.latency_ms, 250);
    }

    #[test]
    fn builder_sets_completed_status() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .completed()
        .build();

        assert_eq!(record.status, RequestStatus::Completed);
        assert!(record.error_message.is_none());
    }

    #[test]
    fn builder_sets_failed_status_with_message() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-123"),
            test_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .failed("Rate limit exceeded")
        .build();

        assert_eq!(record.status, RequestStatus::Failed);
        assert_eq!(
            record.error_message,
            Some("Rate limit exceeded".to_string())
        );
    }

    #[test]
    fn builder_chain_all_methods() {
        let session_id = SessionId::new("session");
        let task_id = TaskId::new("task");
        let context_id = ContextId::new_unchecked(TEST_CONTEXT_ID_A);
        let trace_id = TraceId::new("trace");

        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-full"),
            test_user_id(),
            context_id.clone(),
        )
        .provider("anthropic")
        .model("claude-3-opus")
        .session_id(session_id)
        .task_id(task_id)
        .trace_id(trace_id)
        .max_tokens(8192)
        .usage(Some(
            usage().input(2000).output(1000).cache_read(500).build(),
        ))
        .streaming(true)
        .cost(500)
        .latency(1500)
        .completed()
        .build();

        assert_eq!(record.request_id, "req-full");
        assert_eq!(record.provider.as_deref(), Some("anthropic"));
        assert_eq!(record.model.as_deref(), Some("claude-3-opus"));
        assert_eq!(record.max_tokens, Some(8192));
        assert_eq!(
            record.tokens.tokens_used,
            Some(3500),
            "tokens_used counts the cache read"
        );
        assert!(record.cache.hit);
        assert!(record.is_streaming);
        assert_eq!(record.cost_microdollars, 500);
        assert_eq!(record.latency_ms, 1500);
        assert_eq!(record.status, RequestStatus::Completed);
    }
}

mod builder_optional_ids_tests {
    use super::*;

    #[test]
    fn builder_sets_actor() {
        let user = fixture_user_id();
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-actor"),
            user.clone(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .actor(Actor::agent(user, "claude-code"))
        .build();
        assert!(matches!(record.actor.kind, ActorKind::Agent { .. }));
    }

    #[test]
    fn builder_defaults_actor_to_user() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-d"),
            fixture_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .build();
        assert!(matches!(record.actor.kind, ActorKind::User));
    }

    #[test]
    fn builder_sets_gateway_conversation_id() {
        let gw = GatewayConversationId::new_unchecked("ctx_0123456789abcdef");
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-g"),
            fixture_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .gateway_conversation_id(gw.clone())
        .build();
        assert_eq!(record.gateway_conversation_id, Some(gw));
    }

    #[test]
    fn builder_sets_provider_request_id() {
        let prid = ProviderRequestId::new_unchecked("prov-1");
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-p"),
            fixture_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .provider_request_id(prid.clone())
        .build();
        assert_eq!(record.provider_request_id, Some(prid));
    }

    #[test]
    fn builder_sets_mcp_execution_id() {
        let mid = McpExecutionId::new("mcp-1");
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-m"),
            fixture_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .mcp_execution_id(mid.clone())
        .build();
        assert_eq!(record.mcp_execution_id, Some(mid));
    }

    #[test]
    fn builder_tokens_with_no_values() {
        let record = AiRequestRecordBuilder::new(
            AiRequestId::new("req-n"),
            fixture_user_id(),
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        )
        .provider("openai")
        .model("gpt-4")
        .usage(None)
        .build();
        assert!(record.tokens.tokens_used.is_none());
    }
}

mod ai_request_record_tests {
    use super::*;

    #[test]
    fn builder_method_creates_builder() {
        let user_id = fixture_user_id();
        let builder = AiRequestRecord::builder(
            AiRequestId::new("req-456"),
            user_id,
            ContextId::new_unchecked(TEST_CONTEXT_ID_A),
        );

        let record = builder.provider("test").model("test-model").build();

        assert_eq!(record.request_id, "req-456");
    }
}
