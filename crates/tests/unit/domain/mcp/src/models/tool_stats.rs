//! Unit tests for ToolStats model

use systemprompt_core_mcp::models::ToolStats;

fn create_test_stats() -> ToolStats {
    ToolStats {
        tool_name: "search".to_string(),
        server_name: "api-server".to_string(),
        total_executions: 100,
        success_count: 95,
        error_count: 5,
        avg_duration_ms: Some(150),
        min_duration_ms: Some(50),
        max_duration_ms: Some(500),
    }
}

// ============================================================================
// ToolStats Field Access Tests
// ============================================================================

#[test]
fn test_tool_stats_fields() {
    let stats = create_test_stats();

    assert_eq!(stats.tool_name, "search");
    assert_eq!(stats.server_name, "api-server");
    assert_eq!(stats.total_executions, 100);
    assert_eq!(stats.success_count, 95);
    assert_eq!(stats.error_count, 5);
    assert_eq!(stats.avg_duration_ms, Some(150));
    assert_eq!(stats.min_duration_ms, Some(50));
    assert_eq!(stats.max_duration_ms, Some(500));
}

#[test]
fn test_tool_stats_no_duration_data() {
    let mut stats = create_test_stats();
    stats.avg_duration_ms = None;
    stats.min_duration_ms = None;
    stats.max_duration_ms = None;

    assert!(stats.avg_duration_ms.is_none());
    assert!(stats.min_duration_ms.is_none());
    assert!(stats.max_duration_ms.is_none());
}

#[test]
fn test_tool_stats_zero_executions() {
    let stats = ToolStats {
        tool_name: "new-tool".to_string(),
        server_name: "new-server".to_string(),
        total_executions: 0,
        success_count: 0,
        error_count: 0,
        avg_duration_ms: None,
        min_duration_ms: None,
        max_duration_ms: None,
    };

    assert_eq!(stats.total_executions, 0);
    assert_eq!(stats.success_count, 0);
    assert_eq!(stats.error_count, 0);
}

#[test]
fn test_tool_stats_all_errors() {
    let stats = ToolStats {
        tool_name: "buggy-tool".to_string(),
        server_name: "unstable-server".to_string(),
        total_executions: 10,
        success_count: 0,
        error_count: 10,
        avg_duration_ms: Some(5000),
        min_duration_ms: Some(1000),
        max_duration_ms: Some(10000),
    };

    assert_eq!(stats.total_executions, 10);
    assert_eq!(stats.success_count, 0);
    assert_eq!(stats.error_count, 10);
}

// ============================================================================
// ToolStats Clone Tests
// ============================================================================

#[test]
fn test_tool_stats_clone() {
    let stats = create_test_stats();
    let cloned = stats.clone();

    assert_eq!(stats.tool_name, cloned.tool_name);
    assert_eq!(stats.server_name, cloned.server_name);
    assert_eq!(stats.total_executions, cloned.total_executions);
    assert_eq!(stats.avg_duration_ms, cloned.avg_duration_ms);
}

// ============================================================================
// ToolStats Debug Tests
// ============================================================================

#[test]
fn test_tool_stats_debug() {
    let stats = create_test_stats();
    let debug_str = format!("{:?}", stats);

    assert!(debug_str.contains("ToolStats"));
    assert!(debug_str.contains("search"));
    assert!(debug_str.contains("api-server"));
}

// ============================================================================
// ToolStats Serialization Tests
// ============================================================================

#[test]
fn test_tool_stats_serialize() {
    let stats = create_test_stats();
    let json = serde_json::to_string(&stats).unwrap();

    assert!(json.contains("search"));
    assert!(json.contains("api-server"));
    assert!(json.contains("100"));
    assert!(json.contains("95"));
}

#[test]
fn test_tool_stats_deserialize() {
    let stats = create_test_stats();
    let json = serde_json::to_string(&stats).unwrap();
    let deserialized: ToolStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats.tool_name, deserialized.tool_name);
    assert_eq!(stats.total_executions, deserialized.total_executions);
    assert_eq!(stats.success_count, deserialized.success_count);
}

#[test]
fn test_tool_stats_roundtrip() {
    let stats = create_test_stats();
    let json = serde_json::to_string(&stats).unwrap();
    let deserialized: ToolStats = serde_json::from_str(&json).unwrap();
    let json2 = serde_json::to_string(&deserialized).unwrap();

    assert_eq!(json, json2);
}
