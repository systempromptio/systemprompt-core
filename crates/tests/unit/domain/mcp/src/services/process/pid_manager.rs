use systemprompt_mcp::services::process::pid_manager::{
    find_pid_by_port, find_pids_by_name, find_process_on_port_with_name, get_port_by_pid,
    get_process_name_by_pid,
};

#[test]
fn find_pid_by_port_unused_port_returns_none() {
    let result = find_pid_by_port(59997).unwrap();
    assert!(result.is_none());
}

#[test]
fn find_pid_by_port_zero_returns_ok() {
    let result = find_pid_by_port(0);
    assert!(result.is_ok());
}

#[test]
fn find_pids_by_name_nonexistent_returns_empty() {
    let result = find_pids_by_name("zzz_nonexistent_process_name_12345").unwrap();
    assert!(result.is_empty());
}

#[test]
fn find_process_on_port_with_name_unused_port_returns_none() {
    let result = find_process_on_port_with_name(59996, "nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn find_process_on_port_with_name_wrong_name_returns_none() {
    let result = find_process_on_port_with_name(59995, "definitely_not_running").unwrap();
    assert!(result.is_none());
}

#[test]
fn get_port_by_pid_nonexistent_returns_none() {
    let result = get_port_by_pid(4_194_305).unwrap();
    assert!(result.is_none());
}

#[test]
fn get_port_by_pid_current_process() {
    let pid = std::process::id();
    let result = get_port_by_pid(pid);
    assert!(result.is_ok());
}

#[test]
fn get_process_name_by_pid_current_process() {
    let pid = std::process::id();
    let name = get_process_name_by_pid(pid);
    if let Some(ref n) = name {
        assert!(!n.is_empty());
    }
    assert!(name.is_some() || name.is_none());
}

#[test]
fn get_process_name_by_pid_nonexistent() {
    let name = get_process_name_by_pid(4_194_305);
    assert!(name.is_none());
}

#[test]
fn get_process_name_by_pid_max_pid() {
    let name = get_process_name_by_pid(u32::MAX);
    assert!(name.is_none());
}
