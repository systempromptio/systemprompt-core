//! Tests for high velocity detection, combined scanner detection, and struct
//! tests

use systemprompt_security::ScannerDetector;

// ============================================================================
// High Velocity Detection Tests
// ============================================================================

#[test]
fn test_is_high_velocity_normal() {
    assert!(!ScannerDetector::is_high_velocity(10, 60));
    assert!(!ScannerDetector::is_high_velocity(30, 60));
}

#[test]
fn test_is_high_velocity_high() {
    assert!(ScannerDetector::is_high_velocity(31, 60));
    assert!(ScannerDetector::is_high_velocity(100, 60));
    assert!(ScannerDetector::is_high_velocity(60, 60));
}

#[test]
fn test_is_high_velocity_zero_duration() {
    assert!(!ScannerDetector::is_high_velocity(100, 0));
}

#[test]
fn test_is_high_velocity_negative_duration() {
    assert!(!ScannerDetector::is_high_velocity(100, -1));
}

#[test]
fn test_is_high_velocity_edge_case() {
    assert!(!ScannerDetector::is_high_velocity(30, 60));
    assert!(ScannerDetector::is_high_velocity(31, 60));
}

#[test]
fn test_is_high_velocity_one_second() {
    assert!(ScannerDetector::is_high_velocity(1, 1));
    assert!(!ScannerDetector::is_high_velocity(0, 1));
}

// ============================================================================
// Combined Scanner Detection Tests
// ============================================================================

#[test]
fn test_is_scanner_path_only() {
    assert!(ScannerDetector::is_scanner(
        Some("/wp-admin"),
        Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0"),
        None,
        None
    ));
}

#[test]
fn test_is_scanner_user_agent_only() {
    assert!(ScannerDetector::is_scanner(
        Some("/api/data"),
        Some("Nmap Scripting Engine"),
        None,
        None
    ));
}

#[test]
fn test_is_scanner_high_velocity_only() {
    assert!(ScannerDetector::is_scanner(
        Some("/api/data"),
        Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0"),
        Some(100),
        Some(60)
    ));
}

#[test]
fn test_is_scanner_no_user_agent() {
    assert!(ScannerDetector::is_scanner(
        Some("/api/data"),
        None,
        None,
        None
    ));
}

#[test]
fn test_is_scanner_all_legitimate() {
    assert!(!ScannerDetector::is_scanner(
        Some("/api/data"),
        Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0"),
        Some(10),
        Some(60)
    ));
}

#[test]
fn test_is_scanner_no_path() {
    assert!(!ScannerDetector::is_scanner(
        None,
        Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0"),
        None,
        None
    ));
}

#[test]
fn test_is_scanner_partial_velocity_data() {
    assert!(!ScannerDetector::is_scanner(
        Some("/api/data"),
        Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0"),
        Some(100),
        None
    ));
    assert!(!ScannerDetector::is_scanner(
        Some("/api/data"),
        Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0"),
        None,
        Some(60)
    ));
}

// ============================================================================
// ScannerDetector Struct Tests
// ============================================================================

#[test]
fn test_scanner_detector_debug() {
    let detector = ScannerDetector;
    let debug_str = format!("{:?}", detector);
    assert!(debug_str.contains("ScannerDetector"));
}
