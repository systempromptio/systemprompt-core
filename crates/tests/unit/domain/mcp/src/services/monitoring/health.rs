//! Unit tests for MCP health monitoring types

use systemprompt_mcp::services::monitoring::health::HealthStatus;

// ============================================================================
// HealthStatus as_str Tests
// ============================================================================

#[test]
fn test_health_status_healthy_as_str() {
    assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
}

#[test]
fn test_health_status_degraded_as_str() {
    assert_eq!(HealthStatus::Degraded.as_str(), "degraded");
}

#[test]
fn test_health_status_unhealthy_as_str() {
    assert_eq!(HealthStatus::Unhealthy.as_str(), "unhealthy");
}

#[test]
fn test_health_status_unknown_as_str() {
    assert_eq!(HealthStatus::Unknown.as_str(), "unknown");
}

// ============================================================================
// HealthStatus emoji Tests
// ============================================================================

#[test]
fn test_health_status_healthy_emoji() {
    assert_eq!(HealthStatus::Healthy.emoji(), "\u{2705}");
}

#[test]
fn test_health_status_degraded_emoji() {
    assert_eq!(HealthStatus::Degraded.emoji(), "\u{26a0}\u{fe0f}");
}

#[test]
fn test_health_status_unhealthy_emoji() {
    assert_eq!(HealthStatus::Unhealthy.emoji(), "\u{274c}");
}

#[test]
fn test_health_status_unknown_emoji() {
    assert_eq!(HealthStatus::Unknown.emoji(), "\u{2753}");
}

// ============================================================================
// HealthStatus Equality and Clone Tests
// ============================================================================

#[test]
fn test_health_status_equality() {
    assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
    assert_eq!(HealthStatus::Degraded, HealthStatus::Degraded);
    assert_eq!(HealthStatus::Unhealthy, HealthStatus::Unhealthy);
    assert_eq!(HealthStatus::Unknown, HealthStatus::Unknown);
}

#[test]
fn test_health_status_inequality() {
    assert_ne!(HealthStatus::Healthy, HealthStatus::Degraded);
    assert_ne!(HealthStatus::Degraded, HealthStatus::Unhealthy);
    assert_ne!(HealthStatus::Unhealthy, HealthStatus::Unknown);
    assert_ne!(HealthStatus::Unknown, HealthStatus::Healthy);
}

#[test]
fn test_health_status_clone() {
    let status = HealthStatus::Healthy;
    let cloned = status.clone();
    assert_eq!(status, cloned);
}

#[test]
fn test_health_status_copy() {
    let status = HealthStatus::Degraded;
    let copied = status;
    assert_eq!(status, copied);
}

// ============================================================================
// HealthStatus Debug Tests
// ============================================================================

#[test]
fn test_health_status_debug() {
    assert!(format!("{:?}", HealthStatus::Healthy).contains("Healthy"));
    assert!(format!("{:?}", HealthStatus::Degraded).contains("Degraded"));
    assert!(format!("{:?}", HealthStatus::Unhealthy).contains("Unhealthy"));
    assert!(format!("{:?}", HealthStatus::Unknown).contains("Unknown"));
}

// ============================================================================
// HealthStatus All Variants Tests
// ============================================================================

#[test]
fn test_health_status_all_variants_as_str() {
    let variants = [
        (HealthStatus::Healthy, "healthy"),
        (HealthStatus::Degraded, "degraded"),
        (HealthStatus::Unhealthy, "unhealthy"),
        (HealthStatus::Unknown, "unknown"),
    ];

    for (status, expected) in variants {
        assert_eq!(status.as_str(), expected);
    }
}

#[test]
fn test_health_status_all_variants_emoji_non_empty() {
    let variants = [
        HealthStatus::Healthy,
        HealthStatus::Degraded,
        HealthStatus::Unhealthy,
        HealthStatus::Unknown,
    ];

    for status in variants {
        assert!(!status.emoji().is_empty());
    }
}
