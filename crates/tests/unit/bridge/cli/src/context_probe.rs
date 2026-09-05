//! Whether a `BridgeContext` can be stood up inside the test workspace.

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use tempfile::TempDir;

#[test]
fn a_bridge_context_can_be_started_in_attach_mode_without_binding_a_port() {
    let home = TempDir::new().expect("home");
    let root = home.path().display().to_string();
    let built = temp_env::with_vars(
        [
            ("HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(format!("{root}/.config"))),
            ("XDG_STATE_HOME", Some(format!("{root}/.state"))),
            ("XDG_DATA_HOME", Some(format!("{root}/.data"))),
            ("XDG_CACHE_HOME", Some(format!("{root}/.cache"))),
            ("SP_BRIDGE_PAT", None),
            ("SP_BRIDGE_CONFIG", None),
            ("SUDO_USER", None),
        ],
        || BridgeContext::start(ProxyMode::Attach),
    );

    let ctx = built.expect("attach mode builds a context without serving");
    assert!(
        !ctx.proxy.is_serving(),
        "attach mode must not stand up a listener"
    );
    assert!(
        !format!("{ctx:?}").is_empty(),
        "the context renders for diagnostics"
    );
}
