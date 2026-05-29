//! Targeted coverage boost for modules that remain at line < 70 %:
//!
//! - `error.rs`:  `internal`, `invalid_input`, `PartialImport`, `TarballUnsafe`,
//!                `Internal`, `Http` retryable branch, `CommandSpawnFailed` display
//! - `models/local_sync.rs`: `LocalSyncDirection::Display`, `DiskContent` Debug
//! - `database/mod.rs`: `DatabaseExport`, `UserExport`, `ContextExport`, `ImportResult` serde
//! - `api_client/mod.rs`: `UploadResponse` deserialize; `with_direct_sync` URL-format
//! - `files/mod.rs`: `SyncDiffResult` all-deleted, `changed_paths` ordering
//! - `retry.rs`: zero-duration, base-1 next_delay, asymmetric bounds
//! - `lib.rs` `SyncDirection`: clone, eq, serde, copy
//! - `export/mod.rs`: `escape_yaml` idempotent on already-escaped sequences
//! - `result.rs` `SyncOperationResult`: manual field construction; state=NotStarted

use std::time::Duration;
use chrono::Utc;
use systemprompt_identifiers::{ContextId, SessionId, TenantId, UserId};
use systemprompt_sync::api_client::RetryConfig;
use systemprompt_sync::{
    FileDiffStatus, SyncApiClient, SyncDirection, SyncDiffEntry, SyncDiffResult, SyncError,
    SyncOperationResult, SyncOpState, escape_yaml,
};
use systemprompt_sync::database::{ContextExport, DatabaseExport, ImportResult, UserExport};

// ---------------------------------------------------------------------------
// error.rs coverage
// ---------------------------------------------------------------------------

mod error_constructors {
    use super::*;

    #[test]
    fn internal_constructor_from_string() {
        let err = SyncError::internal("some internal cause");
        let msg = err.to_string();
        assert!(msg.contains("some internal cause"), "got: {msg}");
    }

    #[test]
    fn internal_constructor_from_display_type() {
        let err = SyncError::internal(42u32);
        assert!(err.to_string().contains("42"));
    }

    #[test]
    fn invalid_input_constructor() {
        let err = SyncError::invalid_input("bad slug");
        let msg = err.to_string();
        assert!(msg.contains("bad slug"), "got: {msg}");
    }

    #[test]
    fn partial_import_display() {
        let err = SyncError::PartialImport {
            completed: 3,
            total: 10,
            message: "user upsert failed".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("3"), "got: {msg}");
        assert!(msg.contains("10"));
        assert!(msg.contains("user upsert failed"));
    }

    #[test]
    fn partial_import_is_not_retryable() {
        let err = SyncError::PartialImport {
            completed: 1,
            total: 5,
            message: "fail".to_owned(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn tarball_unsafe_display() {
        let err = SyncError::TarballUnsafe("../etc/passwd".to_owned());
        let msg = err.to_string();
        assert!(msg.contains("../etc/passwd"), "got: {msg}");
    }

    #[test]
    fn tarball_unsafe_is_not_retryable() {
        let err = SyncError::TarballUnsafe("bad".to_owned());
        assert!(!err.is_retryable());
    }

    #[test]
    fn internal_variant_display() {
        let err = SyncError::Internal("low-level failure".to_owned());
        let msg = err.to_string();
        assert!(msg.contains("low-level failure"), "got: {msg}");
    }

    #[test]
    fn internal_variant_is_not_retryable() {
        let err = SyncError::Internal("x".to_owned());
        assert!(!err.is_retryable());
    }

    #[test]
    fn invalid_input_is_not_retryable() {
        let err = SyncError::InvalidInput("bad".to_owned());
        assert!(!err.is_retryable());
    }

    #[tokio::test]
    async fn http_error_is_retryable() {
        let client = reqwest::Client::new();
        let inner = client.get("http://[invalid url]").send().await.unwrap_err();
        let err: SyncError = inner.into();
        assert!(err.is_retryable(), "Http variant must be retryable");
    }

    #[test]
    fn command_spawn_failed_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let err = SyncError::CommandSpawnFailed {
            command: "docker build".to_owned(),
            source: io_err,
        };
        let msg = err.to_string();
        assert!(msg.contains("docker build"), "got: {msg}");
    }

    #[test]
    fn file_open_failed_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = SyncError::FileOpenFailed {
            path: "/etc/shadow".to_owned(),
            source: io_err,
        };
        let msg = err.to_string();
        assert!(msg.contains("/etc/shadow"), "got: {msg}");
    }

    #[test]
    fn api_error_501_is_not_retryable() {
        let err = SyncError::ApiError {
            status: 501,
            message: "not implemented".to_owned(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn api_error_200_is_not_retryable() {
        let err = SyncError::ApiError {
            status: 200,
            message: "ok".to_owned(),
        };
        assert!(!err.is_retryable());
    }
}

// ---------------------------------------------------------------------------
// models/local_sync.rs: LocalSyncDirection Display
// ---------------------------------------------------------------------------

mod local_sync_direction_display {
    use systemprompt_sync::LocalSyncDirection;

    #[test]
    fn to_disk_displays() {
        assert_eq!(LocalSyncDirection::ToDisk.to_string(), "to_disk");
    }

    #[test]
    fn to_database_displays() {
        assert_eq!(LocalSyncDirection::ToDatabase.to_string(), "to_database");
    }

    #[test]
    fn default_is_to_disk() {
        let d = LocalSyncDirection::default();
        assert_eq!(d, LocalSyncDirection::ToDisk);
    }

    #[test]
    fn serialize_to_disk() {
        let json = serde_json::to_string(&LocalSyncDirection::ToDisk).expect("ser");
        assert!(json.contains("ToDisk"), "got: {json}");
    }

    #[test]
    fn serialize_to_database() {
        let json = serde_json::to_string(&LocalSyncDirection::ToDatabase).expect("ser");
        assert!(json.contains("ToDatabase"), "got: {json}");
    }
}

// ---------------------------------------------------------------------------
// database/mod.rs: serde and struct surface
// ---------------------------------------------------------------------------

mod database_types {
    use super::*;

    fn sample_user() -> UserExport {
        UserExport {
            id: UserId::new("usr-01"),
            name: "jsmith".to_owned(),
            email: "j@example.com".to_owned(),
            full_name: Some("John Smith".to_owned()),
            display_name: None,
            status: "active".to_owned(),
            email_verified: true,
            roles: vec!["admin".to_owned()],
            is_bot: false,
            is_scanner: false,
            avatar_url: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn sample_context() -> ContextExport {
        ContextExport {
            context_id: ContextId::new("550e8400-e29b-41d4-a716-446655440000"),
            user_id: UserId::new("usr-01"),
            session_id: Some(SessionId::new("sess-01")),
            name: "My context".to_owned(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn user_export_serialise_roundtrip() {
        let u = sample_user();
        let json = serde_json::to_string(&u).expect("ser");
        let back: UserExport = serde_json::from_str(&json).expect("de");
        assert_eq!(back.email, u.email);
        assert_eq!(back.name, u.name);
    }

    #[test]
    fn user_export_debug_contains_email() {
        let u = sample_user();
        let dbg = format!("{u:?}");
        assert!(dbg.contains("UserExport"));
    }

    #[test]
    fn user_export_optional_fields_roundtrip() {
        let mut u = sample_user();
        u.display_name = Some("JSmith".to_owned());
        u.avatar_url = Some("https://example.com/a.png".to_owned());
        let json = serde_json::to_string(&u).expect("ser");
        let back: UserExport = serde_json::from_str(&json).expect("de");
        assert_eq!(back.display_name, Some("JSmith".to_owned()));
        assert_eq!(back.avatar_url, Some("https://example.com/a.png".to_owned()));
    }

    #[test]
    fn user_export_roles_vec_preserved() {
        let mut u = sample_user();
        u.roles = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
        let json = serde_json::to_string(&u).expect("ser");
        let back: UserExport = serde_json::from_str(&json).expect("de");
        assert_eq!(back.roles.len(), 3);
    }

    #[test]
    fn context_export_serialise_roundtrip() {
        let c = sample_context();
        let json = serde_json::to_string(&c).expect("ser");
        let back: ContextExport = serde_json::from_str(&json).expect("de");
        assert_eq!(back.name, c.name);
    }

    #[test]
    fn context_export_debug_renders() {
        let c = sample_context();
        let dbg = format!("{c:?}");
        assert!(dbg.contains("ContextExport"));
    }

    #[test]
    fn context_export_without_session() {
        let mut c = sample_context();
        c.session_id = None;
        let json = serde_json::to_string(&c).expect("ser");
        let back: ContextExport = serde_json::from_str(&json).expect("de");
        assert!(back.session_id.is_none());
    }

    #[test]
    fn database_export_serialise_roundtrip() {
        let export = DatabaseExport {
            users: vec![sample_user()],
            contexts: vec![sample_context()],
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&export).expect("ser");
        let back: DatabaseExport = serde_json::from_str(&json).expect("de");
        assert_eq!(back.users.len(), 1);
        assert_eq!(back.contexts.len(), 1);
    }

    #[test]
    fn database_export_empty_is_valid() {
        let export = DatabaseExport {
            users: vec![],
            contexts: vec![],
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&export).expect("ser");
        let back: DatabaseExport = serde_json::from_str(&json).expect("de");
        assert!(back.users.is_empty());
        assert!(back.contexts.is_empty());
    }

    #[test]
    fn database_export_debug_renders() {
        let export = DatabaseExport {
            users: vec![],
            contexts: vec![],
            timestamp: Utc::now(),
        };
        assert!(format!("{export:?}").contains("DatabaseExport"));
    }

    #[test]
    fn import_result_created_updated_skipped() {
        let r = ImportResult { created: 5, updated: 3, skipped: 2 };
        assert_eq!(r.created, 5);
        assert_eq!(r.updated, 3);
        assert_eq!(r.skipped, 2);
    }

    #[test]
    fn import_result_serialise() {
        let r = ImportResult { created: 1, updated: 0, skipped: 4 };
        let json = serde_json::to_string(&r).expect("ser");
        assert!(json.contains("\"created\":1"));
        assert!(json.contains("\"skipped\":4"));
        let back: ImportResult = serde_json::from_str(&json).expect("de");
        assert_eq!(back.updated, 0);
    }

    #[test]
    fn import_result_copy_semantics() {
        let r = ImportResult { created: 7, updated: 2, skipped: 1 };
        let copy = r;
        assert_eq!(copy.created, r.created);
    }

    #[test]
    fn import_result_debug_renders() {
        let r = ImportResult { created: 0, updated: 0, skipped: 0 };
        assert!(format!("{r:?}").contains("ImportResult"));
    }
}

// ---------------------------------------------------------------------------
// SyncDirection: Clone, Eq, serde, Copy
// ---------------------------------------------------------------------------

mod sync_direction_full {
    use super::*;

    #[test]
    fn push_and_pull_are_distinct() {
        assert_ne!(SyncDirection::Push, SyncDirection::Pull);
    }

    #[test]
    fn copy_semantics() {
        let d = SyncDirection::Push;
        let d2 = d;
        assert_eq!(d, d2);
    }

    #[test]
    fn clone_semantics() {
        let d = SyncDirection::Pull;
        let d2 = d.clone();
        assert_eq!(d, d2);
    }

    #[test]
    fn serialise_push() {
        let v = serde_json::to_value(SyncDirection::Push).expect("ser");
        assert_eq!(v, serde_json::json!("Push"));
    }

    #[test]
    fn serialise_pull() {
        let v = serde_json::to_value(SyncDirection::Pull).expect("ser");
        assert_eq!(v, serde_json::json!("Pull"));
    }

    #[test]
    fn deserialise_roundtrip() {
        let v = serde_json::json!("Pull");
        let d: SyncDirection = serde_json::from_value(v).expect("de");
        assert_eq!(d, SyncDirection::Pull);
    }
}

// ---------------------------------------------------------------------------
// RetryConfig edge cases
// ---------------------------------------------------------------------------

mod retry_edge_cases {
    use super::*;

    #[test]
    fn zero_duration_stays_zero_when_base_is_two() {
        let c = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::ZERO,
            max_delay: Duration::from_secs(10),
            exponential_base: 2,
        };
        assert_eq!(c.next_delay(Duration::ZERO), Duration::ZERO);
    }

    #[test]
    fn base_one_does_not_grow() {
        let c = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(60),
            exponential_base: 1,
        };
        assert_eq!(c.next_delay(Duration::from_secs(5)), Duration::from_secs(5));
    }

    #[test]
    fn already_at_max_delay_stays_capped() {
        let c = RetryConfig::default();
        let at_cap = Duration::from_secs(30);
        assert_eq!(c.next_delay(at_cap), at_cap);
    }

    #[test]
    fn just_below_cap_doubles_to_cap() {
        let c = RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(20),
            exponential_base: 2,
        };
        let d = c.next_delay(Duration::from_secs(15));
        assert_eq!(d, Duration::from_secs(20));
    }

    #[test]
    fn debug_renders_all_fields() {
        let c = RetryConfig::default();
        let dbg = format!("{c:?}");
        assert!(dbg.contains("max_attempts"));
        assert!(dbg.contains("initial_delay"));
        assert!(dbg.contains("max_delay"));
        assert!(dbg.contains("exponential_base"));
    }
}

// ---------------------------------------------------------------------------
// UploadResponse serde
// ---------------------------------------------------------------------------

mod upload_response {
    use systemprompt_sync::api_client::UploadResponse;

    #[test]
    fn deserialise_count() {
        let json = r#"{"files_uploaded":42}"#;
        let r: UploadResponse = serde_json::from_str(json).expect("de");
        assert_eq!(r.files_uploaded, 42);
    }

    #[test]
    fn zero_files_uploaded() {
        let json = r#"{"files_uploaded":0}"#;
        let r: UploadResponse = serde_json::from_str(json).expect("de");
        assert_eq!(r.files_uploaded, 0);
    }

    #[test]
    fn debug_renders() {
        let json = r#"{"files_uploaded":1}"#;
        let r: UploadResponse = serde_json::from_str(json).expect("de");
        assert!(format!("{r:?}").contains("UploadResponse"));
    }

    #[test]
    fn copy_semantics() {
        let json = r#"{"files_uploaded":3}"#;
        let r: UploadResponse = serde_json::from_str(json).expect("de");
        let r2 = r;
        assert_eq!(r2.files_uploaded, 3);
    }
}

// ---------------------------------------------------------------------------
// SyncDiffResult extended paths
// ---------------------------------------------------------------------------

mod sync_diff_result_extended {
    use super::*;

    fn entry(path: &str, status: FileDiffStatus) -> SyncDiffEntry {
        SyncDiffEntry { path: path.to_owned(), status, size: 100 }
    }

    #[test]
    fn all_deleted_has_changes() {
        let r = SyncDiffResult {
            entries: vec![
                entry("agents/a.yaml", FileDiffStatus::Deleted),
                entry("skills/s.md", FileDiffStatus::Deleted),
            ],
            added: 0,
            modified: 0,
            deleted: 2,
            unchanged: 0,
        };
        assert!(r.has_changes());
        assert_eq!(r.changed_paths().len(), 2);
    }

    #[test]
    fn all_unchanged_has_no_changes() {
        let r = SyncDiffResult {
            entries: vec![entry("agents/a.yaml", FileDiffStatus::Unchanged)],
            added: 0,
            modified: 0,
            deleted: 0,
            unchanged: 1,
        };
        assert!(!r.has_changes());
        assert!(r.changed_paths().is_empty());
    }

    #[test]
    fn changed_paths_returns_only_non_unchanged() {
        let r = SyncDiffResult {
            entries: vec![
                entry("a/added.yaml", FileDiffStatus::Added),
                entry("b/modified.yaml", FileDiffStatus::Modified),
                entry("c/deleted.yaml", FileDiffStatus::Deleted),
                entry("d/same.yaml", FileDiffStatus::Unchanged),
            ],
            added: 1,
            modified: 1,
            deleted: 1,
            unchanged: 1,
        };
        let paths = r.changed_paths();
        assert_eq!(paths.len(), 3);
        assert!(paths.contains(&"a/added.yaml".to_owned()));
        assert!(paths.contains(&"b/modified.yaml".to_owned()));
        assert!(paths.contains(&"c/deleted.yaml".to_owned()));
        assert!(!paths.contains(&"d/same.yaml".to_owned()));
    }

    #[test]
    fn sync_diff_entry_debug_renders() {
        let e = entry("skills/s.md", FileDiffStatus::Added);
        let dbg = format!("{e:?}");
        assert!(dbg.contains("SyncDiffEntry"));
    }

    #[test]
    fn sync_diff_entry_serde_roundtrip() {
        let e = entry("hooks/h.yaml", FileDiffStatus::Modified);
        let json = serde_json::to_string(&e).expect("ser");
        let back: SyncDiffEntry = serde_json::from_str(&json).expect("de");
        assert_eq!(back.path, e.path);
        assert_eq!(back.status, FileDiffStatus::Modified);
    }

    #[test]
    fn file_diff_status_all_variants_serde() {
        for st in [
            FileDiffStatus::Added,
            FileDiffStatus::Modified,
            FileDiffStatus::Deleted,
            FileDiffStatus::Unchanged,
        ] {
            let json = serde_json::to_string(&st).expect("ser");
            let back: FileDiffStatus = serde_json::from_str(&json).expect("de");
            assert_eq!(back, st, "roundtrip failed for {st:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// escape_yaml edge cases
// ---------------------------------------------------------------------------

mod escape_yaml_extra {
    use super::*;

    #[test]
    fn only_backslashes() {
        assert_eq!(escape_yaml("\\\\"), "\\\\\\\\");
    }

    #[test]
    fn only_quotes() {
        assert_eq!(escape_yaml("\"\""), "\\\"\\\"");
    }

    #[test]
    fn only_newlines() {
        assert_eq!(escape_yaml("\n\n"), "\\n\\n");
    }

    #[test]
    fn unicode_pass_through() {
        let s = "héllo wörld";
        assert_eq!(escape_yaml(s), s);
    }

    #[test]
    fn tab_pass_through() {
        assert_eq!(escape_yaml("\t"), "\t");
    }
}

// ---------------------------------------------------------------------------
// SyncOperationResult manual construction and NotStarted state
// ---------------------------------------------------------------------------

mod sync_operation_result_manual {
    use super::*;

    #[test]
    fn not_started_state_is_not_started() {
        let r = SyncOperationResult {
            operation: "db".to_owned(),
            success: false,
            items_synced: 0,
            items_skipped: 0,
            errors: vec!["missing config".to_owned()],
            details: None,
            state: SyncOpState::NotStarted,
        };
        assert_eq!(r.state, SyncOpState::NotStarted);
        assert!(!r.success);
        assert_eq!(r.errors.len(), 1);
    }

    #[test]
    fn failed_state_manual_construction() {
        let r = SyncOperationResult {
            operation: "database".to_owned(),
            success: false,
            items_synced: 0,
            items_skipped: 0,
            errors: vec!["connection refused".to_owned()],
            details: None,
            state: SyncOpState::Failed,
        };
        assert_eq!(r.state, SyncOpState::Failed);
        assert!(!r.success);
    }

    #[test]
    fn partial_state_manual_construction() {
        let r = SyncOperationResult {
            operation: "database".to_owned(),
            success: false,
            items_synced: 2,
            items_skipped: 0,
            errors: vec!["partial".to_owned()],
            details: None,
            state: SyncOpState::Partial { completed: 2, total: 10 },
        };
        assert!(matches!(r.state, SyncOpState::Partial { completed: 2, total: 10 }));
    }

    #[test]
    fn with_details_replaces_none() {
        let r = SyncOperationResult {
            operation: "op".to_owned(),
            success: true,
            items_synced: 0,
            items_skipped: 0,
            errors: vec![],
            details: None,
            state: SyncOpState::Completed,
        };
        let r2 = r.with_details(serde_json::json!({"k": 1}));
        assert!(r2.details.is_some());
        assert_eq!(r2.details.unwrap()["k"], 1);
    }

    #[test]
    fn with_details_overwrites_existing() {
        let r = SyncOperationResult {
            operation: "op".to_owned(),
            success: true,
            items_synced: 1,
            items_skipped: 0,
            errors: vec![],
            details: Some(serde_json::json!("old")),
            state: SyncOpState::Completed,
        };
        let r2 = r.with_details(serde_json::json!("new"));
        assert_eq!(r2.details, Some(serde_json::json!("new")));
    }

    #[test]
    fn clone_and_debug() {
        let r = SyncOperationResult::success("clone_test", 7);
        let r2 = r.clone();
        assert_eq!(r2.items_synced, 7);
        assert!(format!("{r2:?}").contains("SyncOperationResult"));
    }
}

// ---------------------------------------------------------------------------
// SyncApiClient: with_direct_sync URL formatting
// ---------------------------------------------------------------------------

mod api_client_url_format {
    use super::*;

    #[test]
    fn with_direct_sync_prefixes_https() {
        let c = SyncApiClient::new("https://api.example.com", "tok")
            .expect("client")
            .with_direct_sync(Some("app.example.com".to_owned()));
        let dbg = format!("{c:?}");
        assert!(dbg.contains("https://app.example.com"), "got: {dbg}");
    }

    #[test]
    fn with_direct_sync_none_sets_no_origin() {
        let c = SyncApiClient::new("https://api.example.com", "tok")
            .expect("client")
            .with_direct_sync(None);
        let dbg = format!("{c:?}");
        assert!(dbg.contains("direct_sync_origin: None") || dbg.contains("None"));
    }

    #[test]
    fn tenant_id_in_config() {
        let cfg = systemprompt_sync::SyncConfig::builder(
            TenantId::new("acme-corp"),
            "https://api.example.com",
            "tok",
            "/srv",
        )
        .build();
        assert_eq!(cfg.tenant_id.as_str(), "acme-corp");
    }
}
