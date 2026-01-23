//! Unit tests for ProcessManager

use systemprompt_mcp::services::process::ProcessManager;

// ============================================================================
// ProcessManager Creation Tests
// ============================================================================

#[test]
fn test_process_manager_new() {
    let manager = ProcessManager::new();
    let debug = format!("{:?}", manager);
    assert!(debug.contains("ProcessManager"));
}

#[test]
fn test_process_manager_default() {
    let manager = ProcessManager::default();
    let debug = format!("{:?}", manager);
    assert!(debug.contains("ProcessManager"));
}

#[test]
fn test_process_manager_clone() {
    let manager = ProcessManager::new();
    let cloned = manager.clone();
    let debug = format!("{:?}", cloned);
    assert!(debug.contains("ProcessManager"));
}

#[test]
fn test_process_manager_copy() {
    let manager = ProcessManager::new();
    let copied = manager;
    let _original_debug = format!("{:?}", manager);
    let _copied_debug = format!("{:?}", copied);
}

// ============================================================================
// ProcessManager Static Method Tests
// ============================================================================

#[test]
fn test_process_manager_is_running_invalid_pid() {
    let result = ProcessManager::is_running(0);
    assert!(!result);
}

#[test]
fn test_process_manager_is_running_nonexistent_pid() {
    let result = ProcessManager::is_running(u32::MAX);
    assert!(!result);
}

#[test]
fn test_process_manager_is_running_current_process() {
    let pid = std::process::id();
    let result = ProcessManager::is_running(pid);
    assert!(result);
}

#[test]
fn test_process_manager_find_pid_by_port_unused() {
    let result = ProcessManager::find_pid_by_port(59999);
    assert!(result.is_ok());
}

#[test]
fn test_process_manager_find_process_on_port_with_name_unused() {
    let result = ProcessManager::find_process_on_port_with_name(59998, "nonexistent");
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}
