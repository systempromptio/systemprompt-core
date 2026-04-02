use systemprompt_agent::services::agent_orchestration::port_manager::{PortManager, ProcessInfo};

#[test]
fn test_port_manager_new() {
    let pm = PortManager::new();
    let debug_str = format!("{:?}", pm);
    assert!(debug_str.contains("PortManager"));
}

#[test]
fn test_port_manager_default() {
    let pm = PortManager::default();
    let debug_str = format!("{:?}", pm);
    assert!(debug_str.contains("PortManager"));
}

#[test]
fn test_port_manager_copy() {
    let pm = PortManager::new();
    let copied = pm;
    let debug_str = format!("{:?}", copied);
    assert!(debug_str.contains("PortManager"));
}

#[test]
fn test_port_manager_clone() {
    let pm = PortManager::new();
    let cloned = pm.clone();
    let debug_str = format!("{:?}", cloned);
    assert!(debug_str.contains("PortManager"));
}

#[test]
fn test_process_info_construction() {
    let info = ProcessInfo {
        pid: 12345,
        command: "/usr/bin/systemprompt agent-worker --port 8080".to_string(),
    };

    assert_eq!(info.pid, 12345);
    assert!(info.command.contains("systemprompt"));
}

#[test]
fn test_process_info_debug() {
    let info = ProcessInfo {
        pid: 42,
        command: "test-cmd".to_string(),
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("ProcessInfo"));
    assert!(debug_str.contains("42"));
    assert!(debug_str.contains("test-cmd"));
}

#[test]
fn test_process_info_clone() {
    let info = ProcessInfo {
        pid: 999,
        command: "cloned-cmd".to_string(),
    };

    let cloned = info.clone();
    assert_eq!(cloned.pid, 999);
    assert_eq!(cloned.command, "cloned-cmd");
}

#[test]
fn test_process_info_empty_command() {
    let info = ProcessInfo {
        pid: 1,
        command: String::new(),
    };

    assert_eq!(info.pid, 1);
    assert!(info.command.is_empty());
}

#[test]
fn test_process_info_large_pid() {
    let info = ProcessInfo {
        pid: u32::MAX,
        command: "max-pid".to_string(),
    };

    assert_eq!(info.pid, u32::MAX);
}
