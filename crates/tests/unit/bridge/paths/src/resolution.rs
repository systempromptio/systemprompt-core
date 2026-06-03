use systemprompt_bridge::config::paths::{
    bridge_metadata_dir, bridge_staging_dir, bridge_working_dir, cowork3p_sessions_root,
    org_plugins_system, org_plugins_user,
};

fn ends_with(path: &std::path::Path, suffix: &str) -> bool {
    path.to_str()
        .unwrap_or_else(|| panic!("path is not valid UTF-8: {path:?}"))
        .ends_with(suffix)
}

#[test]
fn working_dir_honours_xdg_state_home() {
    temp_env::with_vars([("XDG_STATE_HOME", Some("/tmp/sp-state"))], || {
        let p = bridge_working_dir().expect("working dir resolves with XDG_STATE_HOME set");
        assert!(
            ends_with(&p, "sp-state/systemprompt-bridge"),
            "unexpected working dir: {p:?}"
        );
    });
}

#[test]
fn staging_dir_is_working_dir_staging() {
    temp_env::with_vars([("XDG_STATE_HOME", Some("/tmp/sp-state"))], || {
        let p = bridge_staging_dir().expect("staging dir resolves with XDG_STATE_HOME set");
        assert!(
            ends_with(&p, "sp-state/systemprompt-bridge/staging"),
            "unexpected staging dir: {p:?}"
        );
    });
}

#[test]
fn metadata_dir_is_working_dir_metadata() {
    temp_env::with_vars([("XDG_STATE_HOME", Some("/tmp/sp-state"))], || {
        let p = bridge_metadata_dir().expect("metadata dir resolves with XDG_STATE_HOME set");
        assert!(
            ends_with(&p, "sp-state/systemprompt-bridge/metadata"),
            "unexpected metadata dir: {p:?}"
        );
    });
}

#[test]
fn org_plugins_user_honours_xdg_data_home() {
    temp_env::with_vars([("XDG_DATA_HOME", Some("/tmp/sp-data"))], || {
        let p = org_plugins_user().expect("user org-plugins resolves with XDG_DATA_HOME set");
        assert!(
            ends_with(&p, "sp-data/Claude/org-plugins"),
            "unexpected org-plugins user dir: {p:?}"
        );
    });
}

#[test]
fn cowork3p_sessions_root_honours_xdg_config_home() {
    temp_env::with_vars([("XDG_CONFIG_HOME", Some("/tmp/sp-cfg"))], || {
        let p = cowork3p_sessions_root().expect("sessions root resolves with XDG_CONFIG_HOME set");
        assert!(
            ends_with(&p, "sp-cfg/Claude-3p/local-agent-mode-sessions"),
            "unexpected sessions root: {p:?}"
        );
    });
}

#[test]
fn org_plugins_system_is_constant_on_linux() {
    let p = org_plugins_system().expect("system org-plugins is a constant on Linux");
    assert!(
        ends_with(&p, "Claude/org-plugins"),
        "unexpected system org-plugins dir: {p:?}"
    );
}
