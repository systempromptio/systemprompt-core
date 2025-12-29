//! Unit tests for execution-related identifier types.

use std::collections::HashSet;
use systemprompt_identifiers::{
    ExecutionStepId, LogId, TokenId, ArtifactId, ToDbValue, DbValue
};

// ============================================================================
// ExecutionStepId Tests
// ============================================================================

#[test]
fn test_execution_step_id_new() {
    let id = ExecutionStepId::new("step-123");
    assert_eq!(id.as_str(), "step-123");
}

#[test]
fn test_execution_step_id_generate() {
    let id = ExecutionStepId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_execution_step_id_generate_unique() {
    let id1 = ExecutionStepId::generate();
    let id2 = ExecutionStepId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_execution_step_id_display() {
    let id = ExecutionStepId::new("display-step");
    assert_eq!(format!("{}", id), "display-step");
}

#[test]
fn test_execution_step_id_from_string() {
    let id: ExecutionStepId = String::from("from-string-step").into();
    assert_eq!(id.as_str(), "from-string-step");
}

#[test]
fn test_execution_step_id_from_str() {
    let id: ExecutionStepId = "from-str-step".into();
    assert_eq!(id.as_str(), "from-str-step");
}

#[test]
fn test_execution_step_id_as_ref() {
    let id = ExecutionStepId::new("as-ref-step");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-step");
}

#[test]
fn test_execution_step_id_clone_and_eq() {
    let id1 = ExecutionStepId::new("clone-step");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_execution_step_id_hash() {
    let id1 = ExecutionStepId::new("hash-step");
    let id2 = ExecutionStepId::new("hash-step");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_execution_step_id_serialize_json() {
    let id = ExecutionStepId::new("serialize-step");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-step\"");
}

#[test]
fn test_execution_step_id_deserialize_json() {
    let id: ExecutionStepId = serde_json::from_str("\"deserialize-step\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-step");
}

#[test]
fn test_execution_step_id_to_db_value() {
    let id = ExecutionStepId::new("db-value-step");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-step"));
}

// ============================================================================
// LogId Tests
// ============================================================================

#[test]
fn test_log_id_new() {
    let id = LogId::new("log-123");
    assert_eq!(id.as_str(), "log-123");
}

#[test]
fn test_log_id_generate() {
    let id = LogId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_log_id_generate_unique() {
    let id1 = LogId::generate();
    let id2 = LogId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_log_id_display() {
    let id = LogId::new("display-log");
    assert_eq!(format!("{}", id), "display-log");
}

#[test]
fn test_log_id_from_string() {
    let id: LogId = String::from("from-string-log").into();
    assert_eq!(id.as_str(), "from-string-log");
}

#[test]
fn test_log_id_from_str() {
    let id: LogId = "from-str-log".into();
    assert_eq!(id.as_str(), "from-str-log");
}

#[test]
fn test_log_id_as_ref() {
    let id = LogId::new("as-ref-log");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-log");
}

#[test]
fn test_log_id_clone_and_eq() {
    let id1 = LogId::new("clone-log");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_log_id_hash() {
    let id1 = LogId::new("hash-log");
    let id2 = LogId::new("hash-log");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_log_id_serialize_json() {
    let id = LogId::new("serialize-log");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-log\"");
}

#[test]
fn test_log_id_deserialize_json() {
    let id: LogId = serde_json::from_str("\"deserialize-log\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-log");
}

#[test]
fn test_log_id_to_db_value() {
    let id = LogId::new("db-value-log");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-log"));
}

// ============================================================================
// TokenId Tests
// ============================================================================

#[test]
fn test_token_id_new() {
    let id = TokenId::new("token-123");
    assert_eq!(id.as_str(), "token-123");
}

#[test]
fn test_token_id_generate() {
    let id = TokenId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_token_id_generate_unique() {
    let id1 = TokenId::generate();
    let id2 = TokenId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_token_id_display() {
    let id = TokenId::new("display-token");
    assert_eq!(format!("{}", id), "display-token");
}

#[test]
fn test_token_id_from_string() {
    let id: TokenId = String::from("from-string-token").into();
    assert_eq!(id.as_str(), "from-string-token");
}

#[test]
fn test_token_id_from_str() {
    let id: TokenId = "from-str-token".into();
    assert_eq!(id.as_str(), "from-str-token");
}

#[test]
fn test_token_id_as_ref() {
    let id = TokenId::new("as-ref-token");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-token");
}

#[test]
fn test_token_id_clone_and_eq() {
    let id1 = TokenId::new("clone-token");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_token_id_hash() {
    let id1 = TokenId::new("hash-token");
    let id2 = TokenId::new("hash-token");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_token_id_serialize_json() {
    let id = TokenId::new("serialize-token");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-token\"");
}

#[test]
fn test_token_id_deserialize_json() {
    let id: TokenId = serde_json::from_str("\"deserialize-token\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-token");
}

#[test]
fn test_token_id_to_db_value() {
    let id = TokenId::new("db-value-token");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-token"));
}

// ============================================================================
// ArtifactId Tests
// ============================================================================

#[test]
fn test_artifact_id_new() {
    let id = ArtifactId::new("artifact-123");
    assert_eq!(id.as_str(), "artifact-123");
}

#[test]
fn test_artifact_id_generate() {
    let id = ArtifactId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_artifact_id_generate_unique() {
    let id1 = ArtifactId::generate();
    let id2 = ArtifactId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_artifact_id_display() {
    let id = ArtifactId::new("display-artifact");
    assert_eq!(format!("{}", id), "display-artifact");
}

#[test]
fn test_artifact_id_from_string() {
    let id: ArtifactId = String::from("from-string-artifact").into();
    assert_eq!(id.as_str(), "from-string-artifact");
}

#[test]
fn test_artifact_id_from_str() {
    let id: ArtifactId = "from-str-artifact".into();
    assert_eq!(id.as_str(), "from-str-artifact");
}

#[test]
fn test_artifact_id_as_ref() {
    let id = ArtifactId::new("as-ref-artifact");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-artifact");
}

#[test]
fn test_artifact_id_clone_and_eq() {
    let id1 = ArtifactId::new("clone-artifact");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_artifact_id_hash() {
    let id1 = ArtifactId::new("hash-artifact");
    let id2 = ArtifactId::new("hash-artifact");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_artifact_id_serialize_json() {
    let id = ArtifactId::new("serialize-artifact");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-artifact\"");
}

#[test]
fn test_artifact_id_deserialize_json() {
    let id: ArtifactId = serde_json::from_str("\"deserialize-artifact\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-artifact");
}

#[test]
fn test_artifact_id_to_db_value() {
    let id = ArtifactId::new("db-value-artifact");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-artifact"));
}
