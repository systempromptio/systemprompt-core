use systemprompt_mcp::services::process::utils::{kill_process, process_exists};

#[test]
fn process_exists_current_process() {
    let pid = std::process::id();
    assert!(process_exists(pid));
}

#[test]
fn process_exists_nonexistent_high_pid() {
    assert!(!process_exists(4_194_305));
}

#[test]
fn process_exists_very_high_pid() {
    assert!(!process_exists(u32::MAX - 1));
}

#[test]
fn process_exists_pid_one_does_not_panic() {
    let result = process_exists(1);
    assert!(result || !result);
}

#[test]
fn kill_process_nonexistent_returns_false() {
    let result = kill_process(4_194_305);
    assert!(!result);
}

#[test]
fn kill_process_very_high_pid_returns_false() {
    let result = kill_process(u32::MAX - 1);
    assert!(!result);
}
