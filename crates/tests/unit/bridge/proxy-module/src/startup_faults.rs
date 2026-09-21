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
fn a_recorded_port_with_nothing_listening_is_abandoned() {
    let config = tempfile::tempdir().expect("config tempdir");
    let state = tempfile::tempdir().expect("state tempdir");
    let Some((port, squatter)) = free_candidate_port() else {
        panic!("no candidate port in {DEFAULT_PROXY_PORT}..={MAX_CANDIDATE_PORT} is bindable");
    };
    drop(squatter);

    sandbox(&config, &state, || {
        let ours = InstallId::establish().expect("the sandbox mints an install id");
        portfile::write(port, &ours).expect("record a port nothing is serving");

        let ctx = BridgeContext::start(ProxyMode::Attach)
            .expect("an unreachable recorded port must not stop the start");

        assert_eq!(
            ctx.proxy.port(),
            DEFAULT_PROXY_PORT,
            "an unreachable port proves no ownership, so the record is abandoned"
        );
        assert!(
            !ctx.startup_faults
                .iter()
                .any(|fault| fault.component == "proxy port file"),
            "abandoning an unreachable record is routine, not a fault: {:?}",
            ctx.startup_faults
        );
    });
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
// Append to crates/tests/unit/bridge/proxy-module/src/startup_faults.rs.
#[test]
fn loopback_secret_io_failure_claims_no_port_and_repaired_retry_serves_health() {
    let config = tempfile::tempdir().expect("config tempdir");
    let state = tempfile::tempdir().expect("state tempdir");
    sandbox(&config, &state, || {
        let secret_path = systemprompt_bridge::proxy::secret::secret_path()
            .expect("the sandbox resolves the production secret path");
        std::fs::create_dir_all(secret_path.parent().expect("secret parent"))
            .expect("secret parent");
        std::fs::create_dir(&secret_path).expect("directory blocks secret file creation");

        let failed = BridgeContext::start(ProxyMode::Serve).expect("startup reports proxy fault");
        let fault = fault_for(&failed.startup_faults, "loopback secret");
        assert!(fault.error.contains("directory"), "{fault}");
        let proxy::ProxyRole::Failed(failure) = failed.proxy.role() else {
            panic!(
                "secret failure must not bind or attach: {:?}",
                failed.proxy.role()
            );
        };
        assert!(
            failure.tried_ports().is_empty(),
            "secret failure occurs before any candidate port is tried"
        );
        drop(failed);

        std::fs::remove_dir(&secret_path).expect("repair blocking directory");
        let repaired = BridgeContext::start(ProxyMode::Serve).expect("repaired proxy starts");
        assert!(matches!(
            repaired.proxy.role(),
            proxy::ProxyRole::Serving(_)
        ));
        assert!(
            !repaired
                .startup_faults
                .iter()
                .any(|item| item.component == "loopback secret"),
            "repaired retry clears the secret fault: {:?}",
            repaired.startup_faults
        );
        let secret = std::fs::read_to_string(&secret_path).expect("repaired secret persisted");
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], repaired.proxy.port()));
        let timeout = std::time::Duration::from_secs(5);
        let mut stream = std::net::TcpStream::connect_timeout(&address, timeout)
            .expect("connect to repaired proxy");
        stream
            .set_read_timeout(Some(timeout))
            .expect("set health read timeout");
        stream
            .set_write_timeout(Some(timeout))
            .expect("set health write timeout");
        std::io::Write::write_all(
            &mut stream,
            format!(
                "GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
                secret.trim()
            )
            .as_bytes(),
        )
        .expect("write health request");
        let mut response = String::new();
        std::io::Read::read_to_string(&mut stream, &mut response).expect("read health response");
        assert_eq!(response.lines().next(), Some("HTTP/1.1 200 OK"));
    });
}
#[test]
fn unpublished_port_remains_live_and_repair_restores_peer_discovery() {
    let config = tempfile::tempdir().expect("config tempdir");
    let state = tempfile::tempdir().expect("state tempdir");
    sandbox(&config, &state, || {
        let record = portfile::portfile_path().expect("sandbox port record");
        std::fs::create_dir_all(record.parent().expect("record parent")).expect("record parent");
        std::fs::create_dir(&record).expect("directory blocks port publication");

        let serving = BridgeContext::start(ProxyMode::Serve)
            .expect("port publication failure must not discard the live listener");
        let proxy::ProxyRole::Serving(served) = serving.proxy.role() else {
            panic!("listener remains live: {:?}", serving.proxy.role());
        };
        let port = served.port;
        let port_faults: Vec<_> = serving
            .startup_faults
            .iter()
            .filter(|fault| fault.component == "proxy port file")
            .collect();
        assert_eq!(
            port_faults.len(),
            2,
            "the unreadable preference and failed publication are both diagnosed: {:?}",
            serving.startup_faults
        );
        assert!(
            matches!(serving.proxy.peer(), proxy::peer::PeerIdentity::Ours(ref peer) if peer.port == port)
        );
        assert!(
            proxy::portfile::read(serving.install_id()).is_err(),
            "another process still cannot discover an unpublished listener"
        );

        std::fs::remove_dir(&record).expect("repair record path");
        proxy::portfile::write(port, serving.install_id()).expect("publish existing live listener");
        let attached = BridgeContext::start(ProxyMode::Attach).expect("attach after repair");
        assert_eq!(attached.proxy.port(), port);
        assert!(
            matches!(attached.proxy.peer(), proxy::peer::PeerIdentity::Ours(ref peer) if peer.port == port)
        );
        let repaired = proxy::portfile::read(serving.install_id())
            .expect("read repaired record")
            .expect("record exists");
        assert_eq!(repaired.port, port);
        assert_eq!(repaired.pid, std::process::id());
    });
}
