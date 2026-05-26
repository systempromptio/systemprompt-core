//! Tests for `SyncOperationResult` constructors, builders, and `SyncOpState`
//! serialisation.

use systemprompt_sync::{SyncOpState, SyncOperationResult};

mod success {
    use super::*;

    #[test]
    fn populates_operation_name_and_items_synced() {
        let r = SyncOperationResult::success("files_push", 7);
        assert_eq!(r.operation, "files_push");
        assert_eq!(r.items_synced, 7);
        assert_eq!(r.items_skipped, 0);
        assert!(r.errors.is_empty());
        assert!(r.success);
        assert!(r.details.is_none());
        assert_eq!(r.state, SyncOpState::Completed);
    }

    #[test]
    fn with_details_chains_value() {
        let r = SyncOperationResult::success("op", 1)
            .with_details(serde_json::json!({ "k": "v" }));
        assert_eq!(r.details.as_ref().and_then(|d| d.get("k")).and_then(|v| v.as_str()), Some("v"));
    }
}

mod dry_run {
    use super::*;

    #[test]
    fn populates_items_skipped_and_details() {
        let details = serde_json::json!({ "files": ["a", "b"] });
        let r = SyncOperationResult::dry_run("files_push", 2, details.clone());
        assert_eq!(r.operation, "files_push");
        assert_eq!(r.items_synced, 0);
        assert_eq!(r.items_skipped, 2);
        assert!(r.success);
        assert!(r.errors.is_empty());
        assert_eq!(r.details, Some(details));
        assert_eq!(r.state, SyncOpState::Completed);
    }
}

mod sync_op_state {
    use super::*;

    #[test]
    fn default_is_completed() {
        let s = SyncOpState::default();
        assert_eq!(s, SyncOpState::Completed);
    }

    #[test]
    fn equality_distinguishes_variants() {
        assert_ne!(SyncOpState::NotStarted, SyncOpState::Completed);
        assert_ne!(SyncOpState::Completed, SyncOpState::Failed);
        assert_ne!(
            SyncOpState::Partial { completed: 1, total: 2 },
            SyncOpState::Partial { completed: 2, total: 2 }
        );
    }

    #[test]
    fn copy_and_clone() {
        let s = SyncOpState::Partial { completed: 1, total: 5 };
        let c = s;
        let cl = c.clone();
        assert_eq!(c, cl);
    }

    #[test]
    fn serialise_tagged_snake_case() {
        let not_started = serde_json::to_value(SyncOpState::NotStarted).expect("ser");
        assert_eq!(not_started.get("kind").and_then(|v| v.as_str()), Some("not_started"));

        let partial = serde_json::to_value(SyncOpState::Partial { completed: 1, total: 4 })
            .expect("ser");
        assert_eq!(partial.get("kind").and_then(|v| v.as_str()), Some("partial"));
        assert_eq!(partial.get("completed").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(partial.get("total").and_then(|v| v.as_u64()), Some(4));

        let completed = serde_json::to_value(SyncOpState::Completed).expect("ser");
        assert_eq!(completed.get("kind").and_then(|v| v.as_str()), Some("completed"));

        let failed = serde_json::to_value(SyncOpState::Failed).expect("ser");
        assert_eq!(failed.get("kind").and_then(|v| v.as_str()), Some("failed"));
    }

    #[test]
    fn deserialise_roundtrip() {
        let json = serde_json::json!({"kind": "partial", "completed": 3, "total": 9});
        let s: SyncOpState = serde_json::from_value(json).expect("de");
        assert_eq!(s, SyncOpState::Partial { completed: 3, total: 9 });
    }
}

mod result_serialisation {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let original = SyncOperationResult {
            operation: "x".to_owned(),
            success: false,
            items_synced: 0,
            items_skipped: 0,
            errors: vec!["e".to_owned()],
            details: Some(serde_json::json!(42)),
            state: SyncOpState::Failed,
        };
        let json = serde_json::to_string(&original).expect("ser");
        let parsed: SyncOperationResult = serde_json::from_str(&json).expect("de");
        assert_eq!(parsed.operation, original.operation);
        assert_eq!(parsed.errors, original.errors);
        assert_eq!(parsed.state, original.state);
        assert_eq!(parsed.details, original.details);
    }

    #[test]
    fn state_field_defaults_when_absent() {
        let legacy = serde_json::json!({
            "operation": "old",
            "success": true,
            "items_synced": 0,
            "items_skipped": 0,
            "errors": [],
            "details": null
        });
        let parsed: SyncOperationResult = serde_json::from_value(legacy).expect("de");
        assert_eq!(parsed.state, SyncOpState::Completed);
    }
}
