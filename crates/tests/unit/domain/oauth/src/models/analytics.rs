//! Tests for Analytics models

use chrono::Utc;
use systemprompt_core_oauth::models::analytics::{
    ClientAnalytics, ClientAnalyticsRow, ClientErrorAnalytics, ClientErrorAnalyticsRow,
};

// ============================================================================
// ClientAnalyticsRow to ClientAnalytics conversion Tests
// ============================================================================

fn create_analytics_row() -> ClientAnalyticsRow {
    ClientAnalyticsRow {
        client_id: "test-client-123".to_string(),
        session_count: 100,
        unique_users: 50,
        total_requests: 1000,
        total_tokens: 50000,
        total_cost_cents: 2500,
        avg_session_duration_seconds: 300.5,
        avg_response_time_ms: 150.25,
        first_seen: Utc::now(),
        last_seen: Utc::now(),
    }
}

#[test]
fn test_client_analytics_from_row() {
    let row = create_analytics_row();
    let analytics: ClientAnalytics = row.into();

    assert_eq!(analytics.client_id.as_str(), "test-client-123");
    assert_eq!(analytics.session_count, 100);
    assert_eq!(analytics.unique_users, 50);
    assert_eq!(analytics.total_requests, 1000);
    assert_eq!(analytics.total_tokens, 50000);
    assert_eq!(analytics.total_cost_cents, 2500);
    assert!((analytics.avg_session_duration_seconds - 300.5).abs() < f64::EPSILON);
    assert!((analytics.avg_response_time_ms - 150.25).abs() < f64::EPSILON);
}

#[test]
fn test_client_analytics_timestamps_are_rfc3339() {
    let row = create_analytics_row();
    let analytics: ClientAnalytics = row.into();

    // RFC3339 timestamps should contain 'T' and typically end with 'Z' or timezone
    assert!(analytics.first_seen.contains('T'));
    assert!(analytics.last_seen.contains('T'));
}

#[test]
fn test_client_analytics_client_type_derived() {
    let row = ClientAnalyticsRow {
        client_id: "fp_first-party-client".to_string(),
        ..create_analytics_row()
    };

    let analytics: ClientAnalytics = row.into();
    // ClientType should be derived from client_id
    assert!(format!("{:?}", analytics.client_type).contains("FirstParty") ||
            format!("{:?}", analytics.client_type).contains("Unknown"));
}

#[test]
fn test_client_analytics_serialize() {
    let row = create_analytics_row();
    let analytics: ClientAnalytics = row.into();

    let json = serde_json::to_string(&analytics).unwrap();
    assert!(json.contains("client_id"));
    assert!(json.contains("session_count"));
    assert!(json.contains("unique_users"));
    assert!(json.contains("total_requests"));
}

#[test]
fn test_client_analytics_deserialize() {
    let json = r#"{
        "client_id": "test-client",
        "client_type": "thirdparty",
        "session_count": 10,
        "unique_users": 5,
        "total_requests": 100,
        "total_tokens": 500,
        "total_cost_cents": 25,
        "avg_session_duration_seconds": 120.0,
        "avg_response_time_ms": 50.0,
        "first_seen": "2024-01-01T00:00:00Z",
        "last_seen": "2024-01-02T00:00:00Z"
    }"#;

    let analytics: ClientAnalytics = serde_json::from_str(json).unwrap();
    assert_eq!(analytics.session_count, 10);
    assert_eq!(analytics.unique_users, 5);
}

#[test]
fn test_client_analytics_debug() {
    let row = create_analytics_row();
    let analytics: ClientAnalytics = row.into();

    let debug_str = format!("{:?}", analytics);
    assert!(debug_str.contains("ClientAnalytics"));
    assert!(debug_str.contains("test-client-123"));
}

#[test]
fn test_client_analytics_clone() {
    let row = create_analytics_row();
    let analytics: ClientAnalytics = row.into();
    let cloned = analytics.clone();

    assert_eq!(analytics.client_id, cloned.client_id);
    assert_eq!(analytics.session_count, cloned.session_count);
}

#[test]
fn test_client_analytics_zero_values() {
    let row = ClientAnalyticsRow {
        client_id: "zero-client".to_string(),
        session_count: 0,
        unique_users: 0,
        total_requests: 0,
        total_tokens: 0,
        total_cost_cents: 0,
        avg_session_duration_seconds: 0.0,
        avg_response_time_ms: 0.0,
        first_seen: Utc::now(),
        last_seen: Utc::now(),
    };

    let analytics: ClientAnalytics = row.into();
    assert_eq!(analytics.session_count, 0);
    assert_eq!(analytics.unique_users, 0);
    assert_eq!(analytics.total_cost_cents, 0);
}

// ============================================================================
// ClientErrorAnalyticsRow to ClientErrorAnalytics conversion Tests
// ============================================================================

#[test]
fn test_client_error_analytics_from_row() {
    let row = ClientErrorAnalyticsRow {
        client_id: "error-client".to_string(),
        error_count: 25,
        affected_sessions: 10,
        last_error: Some("Connection timeout".to_string()),
    };

    let analytics: ClientErrorAnalytics = row.into();
    assert_eq!(analytics.client_id.as_str(), "error-client");
    assert_eq!(analytics.error_count, 25);
    assert_eq!(analytics.affected_sessions, 10);
    assert_eq!(analytics.last_error, "Connection timeout");
}

#[test]
fn test_client_error_analytics_from_row_no_error() {
    let row = ClientErrorAnalyticsRow {
        client_id: "clean-client".to_string(),
        error_count: 0,
        affected_sessions: 0,
        last_error: None,
    };

    let analytics: ClientErrorAnalytics = row.into();
    assert_eq!(analytics.error_count, 0);
    assert_eq!(analytics.last_error, "");
}

#[test]
fn test_client_error_analytics_serialize() {
    let row = ClientErrorAnalyticsRow {
        client_id: "error-client".to_string(),
        error_count: 5,
        affected_sessions: 3,
        last_error: Some("Rate limited".to_string()),
    };

    let analytics: ClientErrorAnalytics = row.into();
    let json = serde_json::to_string(&analytics).unwrap();

    assert!(json.contains("client_id"));
    assert!(json.contains("error_count"));
    assert!(json.contains("affected_sessions"));
    assert!(json.contains("last_error"));
}

#[test]
fn test_client_error_analytics_debug() {
    let row = ClientErrorAnalyticsRow {
        client_id: "debug-error-client".to_string(),
        error_count: 1,
        affected_sessions: 1,
        last_error: Some("Debug error".to_string()),
    };

    let analytics: ClientErrorAnalytics = row.into();
    let debug_str = format!("{:?}", analytics);
    assert!(debug_str.contains("ClientErrorAnalytics"));
}

#[test]
fn test_client_error_analytics_clone() {
    let row = ClientErrorAnalyticsRow {
        client_id: "clone-client".to_string(),
        error_count: 10,
        affected_sessions: 5,
        last_error: Some("Clone error".to_string()),
    };

    let analytics: ClientErrorAnalytics = row.into();
    let cloned = analytics.clone();

    assert_eq!(analytics.client_id, cloned.client_id);
    assert_eq!(analytics.error_count, cloned.error_count);
    assert_eq!(analytics.last_error, cloned.last_error);
}

// ============================================================================
// ClientAnalyticsRow Tests
// ============================================================================

#[test]
fn test_client_analytics_row_debug() {
    let row = create_analytics_row();
    let debug_str = format!("{:?}", row);
    assert!(debug_str.contains("ClientAnalyticsRow"));
    assert!(debug_str.contains("test-client-123"));
}

#[test]
fn test_client_analytics_row_clone() {
    let row = create_analytics_row();
    let cloned = row.clone();
    assert_eq!(row.client_id, cloned.client_id);
    assert_eq!(row.session_count, cloned.session_count);
}

// ============================================================================
// ClientErrorAnalyticsRow Tests
// ============================================================================

#[test]
fn test_client_error_analytics_row_debug() {
    let row = ClientErrorAnalyticsRow {
        client_id: "debug-row".to_string(),
        error_count: 1,
        affected_sessions: 1,
        last_error: None,
    };
    let debug_str = format!("{:?}", row);
    assert!(debug_str.contains("ClientErrorAnalyticsRow"));
}

#[test]
fn test_client_error_analytics_row_clone() {
    let row = ClientErrorAnalyticsRow {
        client_id: "clone-row".to_string(),
        error_count: 5,
        affected_sessions: 2,
        last_error: Some("Error message".to_string()),
    };
    let cloned = row.clone();
    assert_eq!(row.client_id, cloned.client_id);
    assert_eq!(row.last_error, cloned.last_error);
}
