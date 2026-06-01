use systemprompt_models::subprocess::{
    AGENT_NAME_ENV, MCP_SERVICE_ID_ENV, environ_identifies_child,
};

fn environ(vars: &[&str]) -> Vec<u8> {
    let mut blob = Vec::new();
    for v in vars {
        blob.extend_from_slice(v.as_bytes());
        blob.push(0);
    }
    blob
}

#[test]
fn matches_agent_child_with_marker_and_name() {
    let env = environ(&["PATH=/usr/bin", "SYSTEMPROMPT_SUBPROCESS=1", "AGENT_NAME=greeter"]);
    assert!(environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}

#[test]
fn matches_mcp_child_with_marker_and_name() {
    let env = environ(&["SYSTEMPROMPT_SUBPROCESS=1", "MCP_SERVICE_ID=files"]);
    assert!(environ_identifies_child(&env, MCP_SERVICE_ID_ENV, "files"));
}

#[test]
fn rejects_missing_subprocess_marker() {
    let env = environ(&["AGENT_NAME=greeter"]);
    assert!(!environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}

#[test]
fn rejects_marker_with_wrong_name() {
    let env = environ(&["SYSTEMPROMPT_SUBPROCESS=1", "AGENT_NAME=other"]);
    assert!(!environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}

#[test]
fn rejects_name_as_substring() {
    let env = environ(&["SYSTEMPROMPT_SUBPROCESS=1", "AGENT_NAME=greeter-staging"]);
    assert!(!environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}

#[test]
fn rejects_empty_environ() {
    assert!(!environ_identifies_child(&[], AGENT_NAME_ENV, "greeter"));
}

#[test]
fn rejects_unrelated_process() {
    let env = environ(&["PATH=/usr/bin", "HOME=/root", "TERM=xterm"]);
    assert!(!environ_identifies_child(&env, AGENT_NAME_ENV, "greeter"));
}
