//! The arms of `proxy` that return an exit code instead of serving.
//!
//! The serving path blocks on Ctrl-C forever, so only the early returns are
//! reachable in-process; this covers those.

use std::process::ExitCode;

use systemprompt_bridge::cli::proxy::test_api::cmd_proxy;
use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use tempfile::TempDir;

fn in_sandbox<R>(f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("home");
    let root = home.path().display().to_string();
    let out = temp_env::with_vars(
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
        f,
    );
    drop(home);
    out
}

#[test]
fn running_the_proxy_command_from_an_attached_context_is_an_internal_error() {
    let code = in_sandbox(|| {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("attach context");
        assert!(
            !ctx.proxy.is_serving(),
            "attach mode is the precondition this arm reports on"
        );
        cmd_proxy(&ctx)
    });

    assert_eq!(
        format!("{code:?}"),
        format!("{:?}", ExitCode::from(70)),
        "attach mode must exit 70 rather than silently doing nothing"
    );
}

#[test]
fn the_attached_arm_returns_promptly_rather_than_blocking_on_a_ctrl_c_handler() {
    let started = std::time::Instant::now();
    in_sandbox(|| {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("attach context");
        cmd_proxy(&ctx)
    });

    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "an attached context must return before reaching the serving loop"
    );
}
