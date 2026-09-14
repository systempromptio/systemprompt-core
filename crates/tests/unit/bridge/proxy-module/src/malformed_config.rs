//! A proxy that cannot read its config never binds: serving inference against
//! the brand-default gateway with whatever credential is on disk is worse than
//! not serving, so the role is `Failed(Config)` and the fault is recorded.

use std::ffi::OsString;

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::proxy::{ProxyFailure, ProxyRole};

fn sandbox<T>(config: &tempfile::TempDir, state: &tempfile::TempDir, f: impl FnOnce() -> T) -> T {
    let vars: Vec<(&str, Option<OsString>)> = vec![
        (
            "XDG_CONFIG_HOME",
            Some(config.path().as_os_str().to_owned()),
        ),
        ("XDG_STATE_HOME", Some(state.path().as_os_str().to_owned())),
        ("SP_BRIDGE_CONFIG", None),
    ];
    temp_env::with_vars(vars, f)
}

#[test]
fn a_malformed_config_file_makes_the_proxy_refuse_to_serve() {
    let config = tempfile::tempdir().expect("config tempdir");
    let state = tempfile::tempdir().expect("state tempdir");
    sandbox(&config, &state, || {
        let dir = config.path().join("systemprompt");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(
            dir.join("systemprompt-bridge.toml"),
            b"gateway_url = [ not toml",
        )
        .expect("seed");

        let ctx = BridgeContext::start(ProxyMode::Serve)
            .expect("a refused proxy is a state the context reports, not a start failure");

        let ProxyRole::Failed(ProxyFailure::Config(error)) = ctx.proxy.role() else {
            panic!(
                "a malformed config must refuse to serve: {:?}",
                ctx.proxy.role()
            );
        };
        assert!(
            error.to_string().contains("not valid TOML"),
            "the failure names the parse error: {error}"
        );
        assert!(!ctx.proxy.is_serving());
        assert!(
            ctx.startup_faults
                .iter()
                .any(|fault| fault.component == "config"),
            "the refusal is surfaced as a startup fault: {:?}",
            ctx.startup_faults
        );
        assert!(
            !dir.join("bridge-loopback.key").exists(),
            "no loopback secret is minted for a proxy that will not serve"
        );
    });
}
