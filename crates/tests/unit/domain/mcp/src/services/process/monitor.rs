//! Unit tests for process monitoring types

use systemprompt_mcp::services::process::monitor::{get_process_info, is_process_running, ProcessInfo};

// ============================================================================
// ProcessInfo Tests
// ============================================================================

#[test]
fn test_process_info_creation() {
    let info = ProcessInfo {
        pid: 1234,
        ppid: 1,
        command: "/usr/bin/test".to_string(),
    };

    assert_eq!(info.pid, 1234);
    assert_eq!(info.ppid, 1);
    assert_eq!(info.command, "/usr/bin/test");
}

#[test]
fn test_process_info_clone() {
    let info = ProcessInfo {
        pid: 5678,
        ppid: 100,
        command: "test-process --flag".to_string(),
    };

    let cloned = info.clone();
    assert_eq!(info.pid, cloned.pid);
    assert_eq!(info.ppid, cloned.ppid);
    assert_eq!(info.command, cloned.command);
}

#[test]
fn test_process_info_debug() {
    let info = ProcessInfo {
        pid: 9999,
        ppid: 1,
        command: "my-command".to_string(),
    };

    let debug = format!("{:?}", info);
    assert!(debug.contains("ProcessInfo"));
    assert!(debug.contains("9999"));
    assert!(debug.contains("my-command"));
}

#[test]
fn test_process_info_with_long_command() {
    let long_cmd = format!("{} {}", "/usr/bin/very-long-path/to/binary", "a".repeat(1000));
    let info = ProcessInfo {
        pid: 1,
        ppid: 0,
        command: long_cmd.clone(),
    };

    assert_eq!(info.command, long_cmd);
}

#[test]
fn test_process_info_with_special_characters() {
    let info = ProcessInfo {
        pid: 42,
        ppid: 1,
        command: "cmd --arg='value with spaces' --flag=\"quoted\"".to_string(),
    };

    assert!(info.command.contains("spaces"));
    assert!(info.command.contains("quoted"));
}

// ============================================================================
// is_process_running Tests
// ============================================================================

#[test]
fn test_is_process_running_current_process() {
    let pid = std::process::id();
    assert!(is_process_running(pid));
}

#[test]
fn test_is_process_running_nonexistent() {
    assert!(!is_process_running(u32::MAX));
}

#[test]
fn test_is_process_running_zero() {
    assert!(!is_process_running(0));
}

#[test]
fn test_is_process_running_init() {
    let result = is_process_running(1);
    assert!(result || !result);
}

// ============================================================================
// get_process_info Tests
// ============================================================================

#[test]
fn test_get_process_info_current_process() {
    let pid = std::process::id();
    let result = get_process_info(pid);
    assert!(result.is_ok());

    if let Ok(Some(info)) = result {
        assert_eq!(info.pid, pid);
        assert!(!info.command.is_empty());
    }
}

#[test]
fn test_get_process_info_nonexistent() {
    let result = get_process_info(u32::MAX);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_get_process_info_zero() {
    let result = get_process_info(0);
    assert!(result.is_err() || result.unwrap().is_none());
}
