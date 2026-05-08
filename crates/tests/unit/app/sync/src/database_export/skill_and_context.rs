//! Tests for ContextExport.

use chrono::Utc;
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_sync::ContextExport;

mod context_export_tests {
    use super::*;

    #[test]
    fn creation_with_session() {
        let now = Utc::now();
        let context = ContextExport {
            context_id: ContextId::new("ctx_123"),
            user_id: UserId::new("user_456"),
            session_id: Some(SessionId::new("session_789")),
            name: "Test Context".to_string(),
            created_at: now,
            updated_at: now,
        };

        assert_eq!(context.context_id.as_str(), "ctx_123");
        assert_eq!(context.user_id.as_str(), "user_456");
        context.session_id.as_ref().expect("Should have session id");
    }

    #[test]
    fn creation_without_session() {
        let now = Utc::now();
        let context = ContextExport {
            context_id: ContextId::new("ctx_no_session"),
            user_id: UserId::new("user_123"),
            session_id: None,
            name: "No Session Context".to_string(),
            created_at: now,
            updated_at: now,
        };

        assert!(context.session_id.is_none());
    }
}
