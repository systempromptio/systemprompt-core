//! Tests for the trace-model filter builders.
//!
//! Exercises `AiRequestFilter`, `ToolExecutionFilter`, `TraceListFilter`, and
//! `LogSearchFilter` — each is a small const-fn builder over an `Option`-bag.

use chrono::{TimeZone, Utc};
use systemprompt_identifiers::AiRequestId;
use systemprompt_runtime::{
    AiRequestFilter, LogSearchFilter, RequestCursor, RequestCursorError, ToolExecutionFilter,
    TraceListFilter,
};

mod ai_request_filter {
    use super::*;

    #[test]
    fn new_initialises_with_limit_only() {
        let f = AiRequestFilter::new(25);
        assert_eq!(f.limit, 25);
        assert!(f.since.is_none());
        assert!(f.model.is_none());
        assert!(f.provider.is_none());
    }

    #[test]
    fn with_since_sets_value() {
        let ts = Utc
            .with_ymd_and_hms(2026, 1, 2, 3, 4, 5)
            .single()
            .expect("date");
        let f = AiRequestFilter::new(10).with_since(ts);
        assert_eq!(f.since, Some(ts));
    }

    #[test]
    fn with_until_and_before_set_the_upper_bounds() {
        let ts = Utc
            .with_ymd_and_hms(2026, 9, 12, 0, 0, 0)
            .single()
            .expect("date");
        let cursor = RequestCursor {
            created_at: ts,
            id: AiRequestId::new("req_last"),
        };
        let f = AiRequestFilter::new(10)
            .with_until(ts)
            .with_before(cursor.clone());
        assert_eq!(f.until, Some(ts));
        assert_eq!(f.before, Some(cursor));
    }

    #[test]
    fn request_cursor_round_trips_through_its_wire_form() {
        let ts = Utc
            .with_ymd_and_hms(2026, 9, 12, 13, 14, 15)
            .single()
            .expect("date");
        let cursor = RequestCursor {
            created_at: ts,
            id: AiRequestId::new("req_last"),
        };
        let wire = cursor.to_string();
        assert_eq!(wire, "2026-09-12T13:14:15.000000Z@req_last");
        assert_eq!(wire.parse::<RequestCursor>(), Ok(cursor));
    }

    #[test]
    fn request_cursor_rejects_malformed_input_with_a_typed_reason() {
        assert_eq!(
            "".parse::<RequestCursor>(),
            Err(RequestCursorError::MissingSeparator)
        );
        assert_eq!(
            "2026-09-12T13:14:15Z".parse::<RequestCursor>(),
            Err(RequestCursorError::MissingSeparator)
        );
        assert_eq!(
            "2026-09-12T13:14:15Z|req".parse::<RequestCursor>(),
            Err(RequestCursorError::MissingSeparator)
        );
        assert_eq!(
            "2026-09-12T13:14:15Z@".parse::<RequestCursor>(),
            Err(RequestCursorError::EmptyId)
        );
        assert!(matches!(
            "not-a-date@req".parse::<RequestCursor>(),
            Err(RequestCursorError::InvalidTimestamp { stamp, .. }) if stamp == "not-a-date"
        ));
    }

    #[test]
    fn with_model_and_provider_are_independent() {
        let f = AiRequestFilter::new(5)
            .with_model("claude-opus-4-7".to_owned())
            .with_provider("anthropic".to_owned());
        assert_eq!(f.model.as_deref(), Some("claude-opus-4-7"));
        assert_eq!(f.provider.as_deref(), Some("anthropic"));
    }

    #[test]
    fn debug_and_clone() {
        let f = AiRequestFilter::new(1).with_provider("openai".to_owned());
        let cloned = f.clone();
        assert_eq!(cloned.provider, f.provider);
        assert!(format!("{f:?}").contains("AiRequestFilter"));
    }
}

mod tool_execution_filter {
    use super::*;

    #[test]
    fn new_initialises_with_limit_only() {
        let f = ToolExecutionFilter::new(50);
        assert_eq!(f.limit, 50);
        assert!(f.since.is_none());
        assert!(f.name.is_none());
        assert!(f.server.is_none());
        assert!(f.status.is_none());
    }

    #[test]
    fn builders_set_each_field() {
        let ts = Utc::now();
        let f = ToolExecutionFilter::new(10)
            .with_since(ts)
            .with_name("read_file".to_owned())
            .with_server("filesystem".to_owned())
            .with_status("ok".to_owned());
        assert_eq!(f.since, Some(ts));
        assert_eq!(f.name.as_deref(), Some("read_file"));
        assert_eq!(f.server.as_deref(), Some("filesystem"));
        assert_eq!(f.status.as_deref(), Some("ok"));
    }
}

mod trace_list_filter {
    use super::*;

    #[test]
    fn new_initialises_defaults() {
        let f = TraceListFilter::new(100);
        assert_eq!(f.limit, 100);
        assert!(!f.has_mcp);
        assert!(!f.include_system);
        assert!(f.agent.is_none());
        assert!(f.status.is_none());
        assert!(f.tool.is_none());
        assert!(f.since.is_none());
    }

    #[test]
    fn boolean_flags_toggle() {
        let f = TraceListFilter::new(1)
            .with_has_mcp(true)
            .with_include_system(true);
        assert!(f.has_mcp);
        assert!(f.include_system);
    }

    #[test]
    fn string_builders_combine() {
        let f = TraceListFilter::new(1)
            .with_agent("planner".to_owned())
            .with_status("completed".to_owned())
            .with_tool("grep".to_owned());
        assert_eq!(f.agent.as_deref(), Some("planner"));
        assert_eq!(f.status.as_deref(), Some("completed"));
        assert_eq!(f.tool.as_deref(), Some("grep"));
    }

    #[test]
    fn with_since() {
        let ts = Utc
            .with_ymd_and_hms(2026, 5, 26, 0, 0, 0)
            .single()
            .expect("date");
        let f = TraceListFilter::new(1).with_since(ts);
        assert_eq!(f.since, Some(ts));
    }
}

mod log_search_filter {
    use super::*;

    #[test]
    fn new_pattern_and_limit() {
        let f = LogSearchFilter::new("oom".to_owned(), 200);
        assert_eq!(f.pattern, "oom");
        assert_eq!(f.limit, 200);
        assert!(f.level.is_none());
        assert!(f.since.is_none());
    }

    #[test]
    fn with_level_sets_value() {
        let f = LogSearchFilter::new("panic".to_owned(), 10).with_level("ERROR".to_owned());
        assert_eq!(f.level.as_deref(), Some("ERROR"));
    }

    #[test]
    fn with_since_sets_value() {
        let ts = Utc::now();
        let f = LogSearchFilter::new("p".to_owned(), 1).with_since(ts);
        assert_eq!(f.since, Some(ts));
    }
}
