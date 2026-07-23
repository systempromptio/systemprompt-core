use systemprompt_bridge::proxy;

#[test]
fn the_module_wide_proxy_starts_once_and_publishes_its_loopback_origin() {
    let temp = tempfile::tempdir().expect("config tempdir");
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        assert!(
            proxy::handle().is_none(),
            "no proxy is running before start_default"
        );

        let first = proxy::start_default().expect("the default proxy binds");
        let second = proxy::start_default().expect("a second call reuses the running proxy");
        assert_eq!(
            first.port, second.port,
            "start_default is idempotent, not additive"
        );
        assert_eq!(first.port, proxy::DEFAULT_PROXY_PORT);

        assert_eq!(
            proxy::handle().map(|h| h.port),
            Some(first.port),
            "the started handle is published module-wide"
        );
        assert_eq!(
            proxy::loopback_origin(),
            format!("http://127.0.0.1:{}", first.port)
        );
        assert_eq!(
            proxy::mcp_url("acme"),
            format!("http://127.0.0.1:{}/mcp/acme", first.port)
        );

        let bearer = proxy::loopback_bearer().expect("the loopback bearer is available");
        assert!(
            bearer.starts_with("Bearer ") && bearer.len() > "Bearer ".len(),
            "loopback bearer is a non-empty Bearer credential"
        );

        assert!(
            temp.path()
                .join("systemprompt")
                .join("bridge-loopback.key")
                .is_file(),
            "starting the proxy mints the loopback secret in the sandbox"
        );
    });
}

#[test]
fn block_on_and_runtime_handle_share_the_process_runtime() {
    let value = proxy::block_on(async { 40 + 2 }).expect("the shared runtime builds");
    assert_eq!(value, 42);

    let handle = proxy::runtime_handle().expect("a handle onto the same runtime");
    let spawned = proxy::block_on(async move {
        handle
            .spawn(async { "from the shared runtime" })
            .await
            .expect("spawned task completes")
    })
    .expect("runtime available");
    assert_eq!(spawned, "from the shared runtime");
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
        proxy::reload_runtime_config();
        proxy::runtime_config()
            .load()
            .gateway_base
            .as_str()
            .to_owned()
    });
    assert_eq!(gateway, "http://reloaded.invalid:7700");
}
