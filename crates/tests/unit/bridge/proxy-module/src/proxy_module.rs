//! The proxy as owned by a `BridgeContext`: serving, attaching, and the
//! WSL2/Windows port collision — all in one process, because a context is a
//! value and not a process-global.

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::proxy::{self, ProxyRole};

#[test]
fn a_serving_context_binds_a_candidate_port_and_publishes_its_endpoint() {
    let temp = tempfile::tempdir().expect("config tempdir");
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let ctx = BridgeContext::start(ProxyMode::Serve).expect("runtime builds");
        let ProxyRole::Serving(served) = ctx.proxy.role() else {
            panic!(
                "the proxy binds one of its candidate ports: {:?}",
                ctx.proxy.role()
            );
        };
        // Not pinned to DEFAULT_PROXY_PORT: a developer machine legitimately
        // has something else on it, and standing aside is now correct.
        assert!(
            proxy::candidate_ports(ctx.install_id()).contains(&served.port),
            "bound {} which is not a candidate port",
            served.port
        );
        assert!(ctx.proxy.is_serving());
        assert_eq!(ctx.proxy.port(), served.port);
        assert_eq!(
            ctx.proxy.loopback().origin(),
            format!("http://127.0.0.1:{}", served.port)
        );
        assert_eq!(
            ctx.proxy.loopback().mcp_url("acme"),
            format!("http://127.0.0.1:{}/mcp/acme", served.port)
        );

        let bearer = ctx
            .proxy
            .loopback()
            .bearer()
            .expect("the loopback bearer is available");
        assert!(
            bearer.starts_with("Bearer ") && bearer.len() > "Bearer ".len(),
            "loopback bearer is a non-empty Bearer credential"
        );

        let dir = temp.path().join("systemprompt");
        assert!(
            dir.join("bridge-loopback.key").is_file(),
            "starting the proxy mints the loopback secret in the sandbox"
        );
        assert!(
            dir.join("bridge-install.id").is_file(),
            "starting the proxy establishes an install id"
        );

        let record = proxy::portfile::read(ctx.install_id()).expect("the bound port is recorded");
        assert_eq!(
            record.port, served.port,
            "the recorded port is the one actually bound, so other processes can find it"
        );
        assert_eq!(record.pid, std::process::id());

        // A second serving context in the same install finds the first one
        // rather than binding beside it.
        let sibling = BridgeContext::start(ProxyMode::Serve).expect("runtime builds");
        let ProxyRole::AlreadyRunning { port, pid, .. } = sibling.proxy.role() else {
            panic!("a sibling stands down: {:?}", sibling.proxy.role());
        };
        assert_eq!(*port, served.port);
        assert_eq!(*pid, std::process::id());
        assert!(!sibling.proxy.is_serving());
        assert_eq!(sibling.proxy.port(), served.port);

        // An attached context (what `install --apply` and `sync` build) resolves
        // the same port from the record without binding anything.
        let attached = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
        assert!(matches!(attached.proxy.role(), ProxyRole::Attached));
        assert_eq!(attached.proxy.port(), served.port);
        assert_eq!(
            attached
                .proxy
                .loopback()
                .bearer()
                .expect("reads the minted secret"),
            bearer
        );
    });
}

#[test]
fn a_taken_default_port_moves_the_proxy_instead_of_failing() {
    let temp = tempfile::tempdir().expect("config tempdir");

    // Stand in for the other machine's bridge (or, in the real bug, WSL2's
    // relay mirroring a Linux bind onto the Windows loopback).
    let squatter = std::net::TcpListener::bind(("127.0.0.1", proxy::DEFAULT_PROXY_PORT));
    let Ok(squatter) = squatter else {
        eprintln!(
            "skipping: port {} is already in use",
            proxy::DEFAULT_PROXY_PORT
        );
        return;
    };

    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let ctx = BridgeContext::start(ProxyMode::Serve).expect("runtime builds");
        let ProxyRole::Serving(served) = ctx.proxy.role() else {
            panic!(
                "a taken default port must not stop the proxy: {:?}",
                ctx.proxy.role()
            );
        };
        assert_ne!(
            served.port,
            proxy::DEFAULT_PROXY_PORT,
            "the squatter still holds the default port"
        );
        assert!(
            proxy::candidate_ports(ctx.install_id()).contains(&served.port),
            "fell outside the candidate range: {}",
            served.port
        );
        // The whole point of moving: everything downstream has to be able to
        // find the new port, including processes that are not this one.
        assert_eq!(
            ctx.proxy.loopback().origin(),
            format!("http://127.0.0.1:{}", served.port)
        );
        let record =
            proxy::portfile::read(ctx.install_id()).expect("the fallback port is recorded on disk");
        assert_eq!(record.port, served.port);
    });

    drop(squatter);
}

#[test]
fn an_attached_context_without_a_record_names_the_default_port() {
    let temp = tempfile::tempdir().expect("config tempdir");
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
        assert!(matches!(ctx.proxy.role(), ProxyRole::Attached));
        assert_eq!(ctx.proxy.port(), proxy::DEFAULT_PROXY_PORT);
        assert!(
            ctx.proxy.loopback().secret().is_err(),
            "no secret has been minted in this sandbox"
        );
    });
}

#[test]
fn block_on_and_spawn_share_the_context_runtime() {
    let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
    assert_eq!(ctx.block_on(async { 40 + 2 }), 42);
    let spawned = ctx.block_on(async {
        ctx.spawn(async { "from the context runtime" })
            .await
            .expect("spawned task completes")
    });
    assert_eq!(spawned, "from the context runtime");
}

#[test]
fn reloading_the_runtime_config_republishes_the_configured_gateway() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let dir = temp.path().join("systemprompt");
    std::fs::create_dir_all(&dir).expect("config dir");
    std::fs::write(
        dir.join("systemprompt-bridge.toml"),
        "gateway_url = \"http://reloaded.invalid:7700\"\n",
    )
    .expect("seed config");

    let gateway = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
        ctx.proxy.reload_runtime_config();
        ctx.proxy
            .runtime_config()
            .load()
            .gateway_base
            .as_str()
            .to_owned()
    });
    assert_eq!(gateway, "http://reloaded.invalid:7700");
}
