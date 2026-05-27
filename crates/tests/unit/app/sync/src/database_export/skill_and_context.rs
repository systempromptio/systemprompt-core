//! Tests for ContextExport.

use chrono::Utc;
use systemprompt_identifiers::{ContextId, SessionId};
use systemprompt_sync::ContextExport;
use systemprompt_test_fixtures::fixture_user_id;

mod context_export_tests {
    use super::*;

    #[test]
    fn creation_with_session() {
        let now = Utc::now();
        let ctx = ContextId::generate();
        let context = ContextExport {
            context_id: ctx.clone(),
            user_id: fixture_user_id(),
            session_id: Some(SessionId::generate()),
            name: "Test Context".to_string(),
            created_at: now,
            updated_at: now,
        };

        assert_eq!(context.context_id.as_str(), ctx.as_str());
        assert!(!context.user_id.as_str().is_empty());
        context.session_id.as_ref().expect("Should have session id");
    }

    #[test]
    fn creation_without_session() {
        let now = Utc::now();
        let context = ContextExport {
            context_id: ContextId::generate(),
            user_id: fixture_user_id(),
            session_id: None,
            name: "No Session Context".to_string(),
            created_at: now,
            updated_at: now,
        };

        assert!(context.session_id.is_none());
    }
}
