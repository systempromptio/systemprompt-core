use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::proxy::peer::PeerIdentity;
use systemprompt_bridge::proxy::{self, ProxyRole};

#[test]
fn a_serving_proxy_identifies_itself_on_its_own_port() {
    let temp = tempfile::tempdir().expect("config tempdir");
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let ctx = BridgeContext::start(ProxyMode::Serve).expect("runtime builds");
        let ProxyRole::Serving(served) = ctx.proxy.role() else {
            panic!("the proxy binds a candidate port: {:?}", ctx.proxy.role());
        };
        let port = served.port;

        let PeerIdentity::Ours(who) = ctx.proxy.peer() else {
            panic!("a serving proxy probes as our own install");
        };
        assert_eq!(who.port, port);
        assert_eq!(who.pid, std::process::id());
        assert!(who.install_id.same_install(ctx.install_id()));

        assert!(ctx.proxy.is_serving());
        assert!(ctx.proxy.served().is_some());

        let rendered = format!("{:?}", ctx.proxy);
        assert!(
            rendered.contains("ProxyHandle") && rendered.contains("Serving"),
            "the handle's Debug names its role: {rendered}"
        );
    });
}

#[test]
fn forgetting_the_recorded_port_removes_the_record() {
    let temp = tempfile::tempdir().expect("config tempdir");
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let ctx = BridgeContext::start(ProxyMode::Serve).expect("runtime builds");
        assert!(
            proxy::portfile::read(ctx.install_id()).is_some(),
            "serving records the bound port"
        );

        ctx.proxy.forget_recorded_port();

        assert!(
            proxy::portfile::read(ctx.install_id()).is_none(),
            "shutdown clears our own record so the next start is free to re-choose"
        );
    });
}

#[test]
fn an_attached_proxy_serves_nothing_and_claims_no_peer_of_its_own() {
    let temp = tempfile::tempdir().expect("config tempdir");
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
        assert!(!ctx.proxy.is_serving());
        assert!(ctx.proxy.served().is_none(), "attach binds nothing");
        assert!(
            !matches!(ctx.proxy.peer(), PeerIdentity::Ours(_)),
            "nothing of this install is serving, so the port is not ours"
        );
        assert!(
            proxy::portfile::read(ctx.install_id()).is_none(),
            "attaching records no port of its own"
        );
    });
}

#[test]
fn a_recorded_port_now_held_by_another_install_is_abandoned() {
    let other = tempfile::tempdir().expect("other install tempdir");
    let ours = tempfile::tempdir().expect("our config tempdir");

    // Push the foreign proxy off the default port so the assertion below can
    // tell "kept the record" from "fell back to the default".
    let squatter = std::net::TcpListener::bind(("127.0.0.1", proxy::DEFAULT_PROXY_PORT));
    let Ok(squatter) = squatter else {
        eprintln!(
            "skipping: port {} is already in use",
            proxy::DEFAULT_PROXY_PORT
        );
        return;
    };

    // A bridge from a different install (a different config dir) holding a port.
    let foreign = temp_env::with_var("XDG_CONFIG_HOME", Some(other.path().as_os_str()), || {
        BridgeContext::start(ProxyMode::Serve).expect("runtime builds")
    });
    let ProxyRole::Serving(served) = foreign.proxy.role() else {
        panic!("the foreign proxy binds a port: {:?}", foreign.proxy.role());
    };
    let foreign_port = served.port;
    assert_ne!(foreign_port, proxy::DEFAULT_PROXY_PORT);

    temp_env::with_var("XDG_CONFIG_HOME", Some(ours.path().as_os_str()), || {
        let install = systemprompt_bridge::proxy::identity::InstallId::establish();
        assert!(
            !install.same_install(foreign.install_id()),
            "the sandboxes must be two distinct installs for this test to mean anything"
        );
        proxy::portfile::write(foreign_port, &install).expect("record the stale port");

        let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
        assert!(matches!(ctx.proxy.role(), ProxyRole::Attached));
        assert_eq!(
            ctx.proxy.port(),
            proxy::DEFAULT_PROXY_PORT,
            "a record pointing at another install's proxy is discarded, not followed"
        );
        assert_ne!(ctx.proxy.port(), foreign_port);
    });

    drop(foreign);
    drop(squatter);
}
