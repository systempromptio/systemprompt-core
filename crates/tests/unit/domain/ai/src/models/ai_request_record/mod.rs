//! Tests for AI request record types.

use systemprompt_core_ai::models::{
    AiRequestRecord, AiRequestRecordBuilder, AiRequestRecordError, CacheInfo, RequestStatus,
    TokenInfo,
};
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};

mod token_info_tests {
    use super::*;

    #[test]
    fn default_token_info_has_none_values() {
        let info = TokenInfo::default();
        assert!(info.tokens_used.is_none());
        assert!(info.input_tokens.is_none());
        assert!(info.output_tokens.is_none());
    }

    #[test]
    fn token_info_can_be_created_with_values() {
        let info = TokenInfo {
            tokens_used: Some(1500),
            input_tokens: Some(1000),
            output_tokens: Some(500),
        };
        assert_eq!(info.tokens_used, Some(1500));
        assert_eq!(info.input_tokens, Some(1000));
        assert_eq!(info.output_tokens, Some(500));
    }

    #[test]
    fn token_info_is_copy() {
        let info = TokenInfo {
            tokens_used: Some(100),
            input_tokens: Some(50),
            output_tokens: Some(50),
        };
        let copied = info;
        assert_eq!(info.tokens_used, copied.tokens_used);
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

    #[test]
    fn cache_info_is_copy() {
        let info = CacheInfo {
            hit: true,
            read_tokens: Some(200),
            creation_tokens: None,
        };
        let copied = info;
        assert_eq!(info.hit, copied.hit);
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

    #[test]
    fn status_equality() {
        assert_eq!(RequestStatus::Pending, RequestStatus::Pending);
        assert_eq!(RequestStatus::Completed, RequestStatus::Completed);
        assert_eq!(RequestStatus::Failed, RequestStatus::Failed);
        assert_ne!(RequestStatus::Pending, RequestStatus::Completed);
    }

    #[test]
    fn status_is_copy() {
        let status = RequestStatus::Completed;
        let copied = status;
        assert_eq!(status, copied);
    }
}

mod ai_request_record_builder_tests {
    use super::*;

    fn test_user_id() -> UserId {
        UserId::new("test-user-123")
    }

    #[test]
    fn builder_requires_provider() {
        let result = AiRequestRecordBuilder::new("req-123", test_user_id())
            .model("gpt-4")
            .build();

        assert!(matches!(result, Err(AiRequestRecordError::MissingProvider)));
    }

    #[test]
    fn builder_requires_model() {
        let result = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .build();

        assert!(matches!(result, Err(AiRequestRecordError::MissingModel)));
    }

    #[test]
    fn builder_creates_record_with_required_fields() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .build()
            .unwrap();

        assert_eq!(record.request_id, "req-123");
        assert_eq!(record.provider, "openai");
        assert_eq!(record.model, "gpt-4");
        assert_eq!(record.status, RequestStatus::Pending);
    }

    #[test]
    fn builder_sets_session_id() {
        let session_id = SessionId::new("session-456");
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("anthropic")
            .model("claude-3")
            .session_id(session_id.clone())
            .build()
            .unwrap();

        assert_eq!(record.session_id, Some(session_id));
    }

    #[test]
    fn builder_sets_task_id() {
        let task_id = TaskId::new("task-789");
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("gemini")
            .model("gemini-pro")
            .task_id(task_id.clone())
            .build()
            .unwrap();

        assert_eq!(record.task_id, Some(task_id));
    }

    #[test]
    fn builder_sets_context_id() {
        let context_id = ContextId::new("ctx-abc");
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .context_id(context_id.clone())
            .build()
            .unwrap();

        assert_eq!(record.context_id, Some(context_id));
    }

    #[test]
    fn builder_sets_trace_id() {
        let trace_id = TraceId::new("trace-xyz");
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .trace_id(trace_id.clone())
            .build()
            .unwrap();

        assert_eq!(record.trace_id, Some(trace_id));
    }

    #[test]
    fn builder_sets_max_tokens() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .max_tokens(4096)
            .build()
            .unwrap();

        assert_eq!(record.max_tokens, Some(4096));
    }

    #[test]
    fn builder_sets_tokens() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .tokens(Some(1000), Some(500))
            .build()
            .unwrap();

        assert_eq!(record.tokens.input_tokens, Some(1000));
        assert_eq!(record.tokens.output_tokens, Some(500));
        assert_eq!(record.tokens.tokens_used, Some(1500));
    }

    #[test]
    fn builder_tokens_with_only_input() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .tokens(Some(1000), None)
            .build()
            .unwrap();

        assert_eq!(record.tokens.input_tokens, Some(1000));
        assert_eq!(record.tokens.output_tokens, None);
        assert_eq!(record.tokens.tokens_used, Some(1000));
    }

    #[test]
    fn builder_tokens_with_only_output() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .tokens(None, Some(500))
            .build()
            .unwrap();

        assert_eq!(record.tokens.input_tokens, None);
        assert_eq!(record.tokens.output_tokens, Some(500));
        assert_eq!(record.tokens.tokens_used, Some(500));
    }

    #[test]
    fn builder_sets_cache_info() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("anthropic")
            .model("claude-3")
            .cache(true, Some(500), Some(100))
            .build()
            .unwrap();

        assert!(record.cache.hit);
        assert_eq!(record.cache.read_tokens, Some(500));
        assert_eq!(record.cache.creation_tokens, Some(100));
    }

    #[test]
    fn builder_sets_streaming() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .streaming(true)
            .build()
            .unwrap();

        assert!(record.is_streaming);
    }

    #[test]
    fn builder_sets_cost() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .cost(150)
            .build()
            .unwrap();

        assert_eq!(record.cost_cents, 150);
    }

    #[test]
    fn builder_sets_latency() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .latency(250)
            .build()
            .unwrap();

        assert_eq!(record.latency_ms, 250);
    }

    #[test]
    fn builder_sets_completed_status() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .completed()
            .build()
            .unwrap();

        assert_eq!(record.status, RequestStatus::Completed);
        assert!(record.error_message.is_none());
    }

    #[test]
    fn builder_sets_failed_status_with_message() {
        let record = AiRequestRecordBuilder::new("req-123", test_user_id())
            .provider("openai")
            .model("gpt-4")
            .failed("Rate limit exceeded")
            .build()
            .unwrap();

        assert_eq!(record.status, RequestStatus::Failed);
        assert_eq!(record.error_message, Some("Rate limit exceeded".to_string()));
    }

    #[test]
    fn builder_chain_all_methods() {
        let session_id = SessionId::new("session");
        let task_id = TaskId::new("task");
        let context_id = ContextId::new("context");
        let trace_id = TraceId::new("trace");

        let record = AiRequestRecordBuilder::new("req-full", test_user_id())
            .provider("anthropic")
            .model("claude-3-opus")
            .session_id(session_id)
            .task_id(task_id)
            .context_id(context_id)
            .trace_id(trace_id)
            .max_tokens(8192)
            .tokens(Some(2000), Some(1000))
            .cache(true, Some(500), None)
            .streaming(true)
            .cost(500)
            .latency(1500)
            .completed()
            .build()
            .unwrap();

        assert_eq!(record.request_id, "req-full");
        assert_eq!(record.provider, "anthropic");
        assert_eq!(record.model, "claude-3-opus");
        assert_eq!(record.max_tokens, Some(8192));
        assert_eq!(record.tokens.tokens_used, Some(3000));
        assert!(record.cache.hit);
        assert!(record.is_streaming);
        assert_eq!(record.cost_cents, 500);
        assert_eq!(record.latency_ms, 1500);
        assert_eq!(record.status, RequestStatus::Completed);
    }
}

mod ai_request_record_tests {
    use super::*;

    #[test]
    fn minimal_fallback_creates_failed_record() {
        let record = AiRequestRecord::minimal_fallback("fallback-123".to_string());

        assert_eq!(record.request_id, "fallback-123");
        assert_eq!(record.user_id.to_string(), "unknown");
        assert_eq!(record.provider, "unknown");
        assert_eq!(record.model, "unknown");
        assert_eq!(record.status, RequestStatus::Failed);
        assert!(record.error_message.is_some());
        assert!(record
            .error_message
            .as_ref()
            .unwrap()
            .contains("construction failed"));
    }

    #[test]
    fn builder_method_creates_builder() {
        let user_id = UserId::new("user-123");
        let builder = AiRequestRecord::builder("req-456", user_id);

        let record = builder.provider("test").model("test-model").build().unwrap();

        assert_eq!(record.request_id, "req-456");
    }
}

mod ai_request_record_error_tests {
    use super::*;

    #[test]
    fn missing_provider_error_display() {
        let err = AiRequestRecordError::MissingProvider;
        assert_eq!(err.to_string(), "Provider is required");
    }

    #[test]
    fn missing_model_error_display() {
        let err = AiRequestRecordError::MissingModel;
        assert_eq!(err.to_string(), "Model is required");
    }

    #[test]
    fn error_is_copy() {
        let err = AiRequestRecordError::MissingProvider;
        let copied = err;
        assert!(matches!(copied, AiRequestRecordError::MissingProvider));
    }
}
