// Loopback policy on the guarded client: the redirect chain may reach the
// local host only when it started there under the configured policy, and the
// untrusted policy refuses loopback and private targets outright.

mod guarded_policy_tests {
    use std::net::SocketAddr;

    use systemprompt_client::{GuardedClientConfig, GuardedConnectError, guarded_client_builder};
    use systemprompt_models::net::OutboundUrlError;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const PUBLIC_NAME: &str = "public.example";

    struct Landing {
        addr: SocketAddr,
        hits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Landing {
        fn hits(&self) -> usize {
            self.hits.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    async fn landing() -> Landing {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = std::sync::Arc::clone(&hits);
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _read = socket.read(&mut buf).await;
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _written = socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nlanded")
                    .await;
            }
        });
        Landing { addr, hits }
    }

    async fn redirector(target: String) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _read = socket.read(&mut buf).await;
                let response = format!(
                    "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\n\r\n"
                );
                let _written = socket.write_all(response.as_bytes()).await;
            }
        });
        addr
    }

    fn client_for_public_name(
        config: &GuardedClientConfig,
        redirect: SocketAddr,
    ) -> reqwest::Client {
        guarded_client_builder(config)
            .resolve(PUBLIC_NAME, redirect)
            .build()
            .expect("client")
    }

    fn redirect_refusal(err: &reqwest::Error) -> Option<&OutboundUrlError> {
        let mut source = std::error::Error::source(err);
        while let Some(inner) = source {
            if let Some(GuardedConnectError::RedirectRefused { source, .. }) =
                inner.downcast_ref::<GuardedConnectError>()
            {
                return Some(source);
            }
            source = inner.source();
        }
        None
    }

    fn trusted_public(config: GuardedClientConfig) -> GuardedClientConfig {
        config.with_trusted_hosts(vec![PUBLIC_NAME.to_owned()])
    }

    async fn public_redirect_to(
        config: GuardedClientConfig,
        target: &str,
    ) -> (reqwest::Error, Landing) {
        let landing = landing().await;
        let target = target.replace("{port}", &landing.addr.port().to_string());
        let redirect = redirector(target).await;
        let client = client_for_public_name(&trusted_public(config), redirect);
        let err = client
            .get(format!("http://{PUBLIC_NAME}:{}/start", redirect.port()))
            .send()
            .await
            .expect_err("the hop must be refused");
        (err, landing)
    }

    #[tokio::test]
    async fn a_public_first_url_redirecting_to_loopback_is_refused_when_configured() {
        let (err, landing) = public_redirect_to(
            GuardedClientConfig::for_configured(),
            "http://127.0.0.1:{port}/",
        )
        .await;
        assert!(
            matches!(
                redirect_refusal(&err),
                Some(OutboundUrlError::NonLoopbackHttp)
            ),
            "{err:?}"
        );
        assert_eq!(
            landing.hits(),
            0,
            "the loopback target never sees a request"
        );
    }

    #[tokio::test]
    async fn a_public_first_url_redirecting_to_loopback_is_refused_when_untrusted() {
        let (err, landing) = public_redirect_to(
            GuardedClientConfig::for_untrusted(),
            "http://127.0.0.1:{port}/",
        )
        .await;
        assert!(redirect_refusal(&err).is_some(), "{err:?}");
        assert_eq!(landing.hits(), 0);
    }

    #[tokio::test]
    async fn a_public_first_url_redirecting_to_https_loopback_is_refused_as_loopback() {
        for config in [
            GuardedClientConfig::for_configured(),
            GuardedClientConfig::for_untrusted(),
        ] {
            let (err, landing) = public_redirect_to(config, "https://127.0.0.1:{port}/").await;
            assert!(
                matches!(redirect_refusal(&err), Some(OutboundUrlError::Loopback(host)) if host == "127.0.0.1"),
                "{err:?}"
            );
            assert_eq!(landing.hits(), 0);
        }
    }

    #[tokio::test]
    async fn a_public_first_url_redirecting_to_localhost_or_mapped_loopback_is_refused() {
        for target in [
            "https://localhost:{port}/",
            "https://[::ffff:127.0.0.1]:{port}/",
            "https://[::1]:{port}/",
            "https://0.0.0.0:{port}/",
            "https://2130706433:{port}/",
        ] {
            let (err, landing) =
                public_redirect_to(GuardedClientConfig::for_configured(), target).await;
            assert!(redirect_refusal(&err).is_some(), "{target}: {err:?}");
            assert_eq!(landing.hits(), 0, "{target}");
        }
    }

    #[tokio::test]
    async fn a_loopback_first_url_may_follow_a_loopback_redirect_when_configured() {
        let landing = landing().await;
        let redirect = redirector(format!("http://127.0.0.1:{}/", landing.addr.port())).await;
        let client = guarded_client_builder(&GuardedClientConfig::for_configured())
            .build()
            .expect("client");

        let response = client
            .get(format!("http://127.0.0.1:{}/start", redirect.port()))
            .send()
            .await
            .expect("loopback-to-loopback is followed");

        assert_eq!(response.text().await.expect("body"), "landed");
        assert_eq!(landing.hits(), 1);
    }

    #[test]
    fn the_untrusted_policy_refuses_loopback_and_private_first_urls() {
        let config = GuardedClientConfig::for_untrusted();
        for url in [
            "https://127.0.0.1/",
            "https://localhost/",
            "https://app.localhost/",
            "https://[::1]/",
            "https://[::ffff:127.0.0.1]/",
            "https://[::ffff:10.0.0.1]/",
            "https://0.0.0.0/",
            "https://10.0.0.1/",
            "https://169.254.169.254/latest/meta-data",
            "https://100.64.0.1/",
            "https://[fd00::1]/",
            "https://2130706433/",
            "https://0x7f000001/",
            "https://0177.0.0.1/",
            "https://017700000001/",
        ] {
            assert!(config.check_url(url).is_err(), "{url} must be refused");
        }
        assert!(config.check_url("https://example.com/").is_ok());
    }

    #[test]
    fn the_configured_policy_accepts_a_loopback_first_url() {
        let config = GuardedClientConfig::for_configured();
        assert!(config.check_url("http://127.0.0.1:8080/").is_ok());
        assert!(config.check_url("http://localhost:8080/").is_ok());
        assert!(config.check_url("https://169.254.169.254/").is_err());
    }

    #[tokio::test]
    async fn the_untrusted_client_refuses_a_hostname_resolving_to_loopback() {
        let landing = landing().await;
        let client = guarded_client_builder(&GuardedClientConfig::for_untrusted())
            .build()
            .expect("client");

        let err = client
            .get(format!("https://localhost:{}/", landing.addr.port()))
            .send()
            .await
            .expect_err("loopback resolution is refused");

        let mut source = std::error::Error::source(&err);
        let mut blocked = false;
        while let Some(inner) = source {
            if matches!(
                inner.downcast_ref::<GuardedConnectError>(),
                Some(GuardedConnectError::BlockedAddress { .. })
            ) {
                blocked = true;
            }
            source = inner.source();
        }
        assert!(blocked, "{err:?}");
        assert_eq!(landing.hits(), 0);
    }
}
