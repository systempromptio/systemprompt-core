use std::ffi::OsString;

use systemprompt_bridge::context::{BridgeContext, ProxyMode, StartupFault};
use systemprompt_bridge::proxy::identity::InstallId;
use systemprompt_bridge::proxy::{self, DEFAULT_PROXY_PORT, MAX_CANDIDATE_PORT, portfile};

fn sandbox<T>(config: &tempfile::TempDir, state: &tempfile::TempDir, f: impl FnOnce() -> T) -> T {
    let vars: Vec<(&str, Option<OsString>)> = vec![
        (
            "XDG_CONFIG_HOME",
            Some(config.path().as_os_str().to_owned()),
        ),
        ("XDG_STATE_HOME", Some(state.path().as_os_str().to_owned())),
    ];
    temp_env::with_vars(vars, f)
}

fn fault_for<'a>(faults: &'a [StartupFault], component: &str) -> &'a StartupFault {
    faults
        .iter()
        .find(|fault| fault.component == component)
        .unwrap_or_else(|| {
            panic!("no `{component}` fault was recorded; faults: {faults:?}");
        })
}

fn seed_port_file(body: &[u8]) {
    let path = portfile::portfile_path().expect("the sandbox resolves a port file path");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, body).expect("seed the port record");
}

fn free_candidate_port() -> Option<(u16, std::net::TcpListener)> {
    // Why: the whole candidate range is shared with every other test process
    // on this machine, so the port is chosen by binding rather than assumed.
    (DEFAULT_PROXY_PORT..=MAX_CANDIDATE_PORT)
        .rev()
        .find_map(|port| {
            std::net::TcpListener::bind(("127.0.0.1", port))
                .ok()
                .map(|listener| (port, listener))
        })
}

#[test]
fn a_corrupt_port_file_degrades_the_start_instead_of_stopping_it() {
    let config = tempfile::tempdir().expect("config tempdir");
    let state = tempfile::tempdir().expect("state tempdir");
    sandbox(&config, &state, || {
        seed_port_file(b"{ not json");

        let ctx = BridgeContext::start(ProxyMode::Attach)
            .expect("a corrupt port record must not stop the commands that repair it");

        let fault = fault_for(&ctx.startup_faults, "proxy port file");
        assert!(
            fault.error.contains("parse"),
            "the fault names what could not be read: {fault}"
        );
        assert_eq!(
            ctx.proxy.port(),
            DEFAULT_PROXY_PORT,
            "an unreadable record falls back to the default port rather than guessing"
        );
    });
}

#[test]
fn a_corrupt_registry_cache_degrades_the_start_and_leaves_the_registry_empty() {
    let config = tempfile::tempdir().expect("config tempdir");
    let state = tempfile::tempdir().expect("state tempdir");
    sandbox(&config, &state, || {
        let meta = systemprompt_bridge::config::paths::bridge_metadata_dir()
            .expect("the sandbox resolves a metadata dir");
        std::fs::create_dir_all(&meta).expect("mkdir");
        std::fs::write(
            meta.join(systemprompt_bridge::config::paths::MCP_SERVERS_FRAGMENT),
            b"[ {not json",
        )
        .expect("seed a corrupt registry cache");

        let ctx = BridgeContext::start(ProxyMode::Attach)
            .expect("a corrupt registry cache must not stop the start");

        let fault = fault_for(&ctx.startup_faults, "mcp registry cache");
        assert!(
            fault.error.contains("parse"),
            "the fault names the unreadable cache: {fault}"
        );
        assert!(
            ctx.mcp_registry().is_empty(),
            "a cache that cannot be parsed publishes no routes at all"
        );
    });
}

#[test]
fn a_recorded_port_held_by_an_unidentified_listener_is_not_followed() {
    let config = tempfile::tempdir().expect("config tempdir");
    let state = tempfile::tempdir().expect("state tempdir");
    let Some((port, squatter)) = free_candidate_port() else {
        panic!("no candidate port in {DEFAULT_PROXY_PORT}..={MAX_CANDIDATE_PORT} is bindable");
    };

    sandbox(&config, &state, || {
        let ours = InstallId::establish().expect("the sandbox mints an install id");
        portfile::write(port, &ours).expect("record the port the squatter holds");

        let ctx = BridgeContext::start(ProxyMode::Attach)
            .expect("a squatted port must not stop the start");

        let fault = fault_for(&ctx.startup_faults, "proxy port file");
        assert!(
            fault.error.contains("unidentified listener"),
            "the fault says why the recorded port was refused: {fault}"
        );
        assert_eq!(
            ctx.proxy.port(),
            DEFAULT_PROXY_PORT,
            "a port answering as something other than this bridge is abandoned, not attached to"
        );
        assert!(
            !matches!(ctx.proxy.role(), proxy::ProxyRole::Serving(_)),
            "attach binds nothing: {:?}",
            ctx.proxy.role()
        );
    });

    drop(squatter);
}

#[test]
fn a_clean_sandbox_starts_without_a_proxy_or_registry_fault() {
    // Why: the negative control for the three tests above — without it a
    // fault recorded on every start would pass them all.
    let config = tempfile::tempdir().expect("config tempdir");
    let state = tempfile::tempdir().expect("state tempdir");
    sandbox(&config, &state, || {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("a clean start");
        let components: Vec<&str> = ctx
            .startup_faults
            .iter()
            .map(|fault| fault.component)
            .collect();
        assert!(
            !components.contains(&"proxy port file"),
            "faults: {:?}",
            ctx.startup_faults
        );
        assert!(
            !components.contains(&"mcp registry cache"),
            "faults: {:?}",
            ctx.startup_faults
        );
    });
}
