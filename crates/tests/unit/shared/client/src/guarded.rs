// Connect-time SSRF guard: the resolver and redirect policy `guarded_client`
// installs. Parse-time cases live in the models test crate.

mod guarded_client_tests {
    use std::io::Error as IoError;

    use reqwest::dns::Resolve;
    use systemprompt_client::{
        DEFAULT_MAX_REDIRECTS, GuardedClientConfig, GuardedConnectError, GuardedResolver,
        guarded_client,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn resolved(resolver: &GuardedResolver, host: &str) -> Result<Vec<String>, String> {
        let name = host.parse().map_err(|_| "bad name".to_owned())?;
        let fut = Resolve::resolve(resolver, name);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e: IoError| e.to_string())?
            .block_on(fut)
            .map(|addrs| addrs.map(|a| a.ip().to_string()).collect())
            .map_err(|e| e.to_string())
    }

    fn resolve_error(resolver: &GuardedResolver, host: &str) -> GuardedConnectError {
        let name = host.parse().expect("hostname");
        let fut = Resolve::resolve(resolver, name);
        let err = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(fut)
            .err()
            .expect("resolution must be refused");
        *err.downcast::<GuardedConnectError>()
            .expect("the resolver surfaces its own error type")
    }

    #[test]
    fn a_name_resolving_into_a_blocked_range_is_refused() {
        let resolver = GuardedResolver::new(Vec::new());
        let err = resolved(&resolver, "localhost").expect_err("loopback must be refused");
        assert!(
            err.contains("blocked address"),
            "expected a blocked-address refusal, got {err}"
        );
    }

    #[test]
    fn the_refusal_names_the_host_and_the_address_it_resolved_to() {
        let resolver = GuardedResolver::new(Vec::new());
        match resolve_error(&resolver, "localhost") {
            GuardedConnectError::BlockedAddress { host, addr } => {
                assert_eq!(host, "localhost");
                assert!(addr.is_loopback(), "localhost resolved to {addr}");
            },
            other => panic!("expected BlockedAddress, got {other:?}"),
        }
    }

    #[test]
    fn a_trusted_name_resolving_into_a_blocked_range_is_allowed() {
        let resolver = GuardedResolver::new(vec!["localhost".to_owned()]);
        let addrs = resolved(&resolver, "localhost").expect("trusted host must resolve");
        assert!(!addrs.is_empty());
    }

    #[test]
    fn loopback_stays_resolvable_under_the_default_config() {
        let config = GuardedClientConfig::default();
        assert!(config.allow_loopback);
        let resolver = GuardedResolver::new(vec!["localhost".to_owned()]);
        assert!(resolved(&resolver, "localhost").is_ok());
    }

    #[test]
    fn denying_loopback_removes_the_localhost_exemption() {
        let config = GuardedClientConfig::default().deny_loopback();
        assert!(!config.allow_loopback);
    }

    fn guard_error(err: &reqwest::Error) -> Option<String> {
        let mut source = std::error::Error::source(err);
        while let Some(inner) = source {
            if let Some(guarded) = inner.downcast_ref::<GuardedConnectError>() {
                return Some(guarded.to_string());
            }
            source = inner.source();
        }
        None
    }

    async fn redirect_once_to(target: &'static str) -> String {
        redirect_once_to_owned(target.to_owned()).await
    }

    async fn redirect_once_to_owned(target: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _read = socket.read(&mut buf).await;
                let response = format!(
                    "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\n\r\n"
                );
                let _written = socket.write_all(response.as_bytes()).await;
            }
        });
        format!("http://{addr}/start")
    }

    struct HopServer {
        url: String,
        received: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    async fn hop_server(hits: usize, next: Option<String>) -> HopServer {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&received);
        let self_url = format!("http://{addr}/hop");
        tokio::spawn(async move {
            for n in 1..=hits {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = [0u8; 2048];
                let _read = socket.read(&mut buf).await;
                counter.fetch_add(1, Ordering::SeqCst);
                let response = if n == hits {
                    "HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nlanded".to_owned()
                } else {
                    let target = next.clone().unwrap_or_else(|| self_url.clone());
                    format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\n\r\n")
                };
                let _written = socket.write_all(response.as_bytes()).await;
            }
        });
        HopServer {
            url: format!("http://{addr}/hop"),
            received,
        }
    }

    fn hits(server: &HopServer) -> usize {
        server.received.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[tokio::test]
    async fn a_redirect_to_a_blocked_address_is_refused() {
        let url = redirect_once_to("http://169.254.169.254/latest/meta-data").await;
        let client = guarded_client(&GuardedClientConfig::default()).expect("client");
        let err = client.get(&url).send().await.expect_err("must be refused");
        assert!(err.is_redirect(), "expected a redirect refusal, got {err}");
        let reason = guard_error(&err).expect("the guard's verdict is in the source chain");
        assert!(reason.contains("169.254.169.254"), "{reason}");
    }

    #[tokio::test]
    async fn a_redirect_to_a_hostname_resolving_into_a_blocked_range_is_refused() {
        // Why: the parse-time guard passes `localhost` on the hop; only the
        // resolver installed on the client can see that it lands on loopback.
        let landing = hop_server(1, None).await;
        let hop = redirect_once_to_owned(landing.url.replace("127.0.0.1", "localhost")).await;
        let client =
            guarded_client(&GuardedClientConfig::default().deny_loopback()).expect("client");

        let err = client.get(&hop).send().await.expect_err("must be refused");

        let reason = guard_error(&err).expect("the guard's verdict is in the source chain");
        assert!(
            reason.contains("localhost resolves to blocked address 127.0.0.1")
                || reason.contains("localhost resolves to blocked address ::1"),
            "{reason}"
        );
        assert_eq!(hits(&landing), 0, "nothing may reach the blocked host");
    }

    #[tokio::test]
    async fn a_chain_of_public_hops_ending_in_a_blocked_range_is_refused_at_the_end() {
        let landing = hop_server(1, None).await;
        let second = redirect_once_to_owned(landing.url.replace("127.0.0.1", "localhost")).await;
        let first = redirect_once_to_owned(second).await;
        let client = guarded_client(
            &GuardedClientConfig::default()
                .with_trusted_hosts(vec!["127.0.0.1".to_owned()])
                .deny_loopback(),
        )
        .expect("client");

        let err = client
            .get(&first)
            .send()
            .await
            .expect_err("must be refused");

        let reason = guard_error(&err).expect("guard verdict");
        assert!(
            reason.contains("localhost resolves to blocked address"),
            "{reason}"
        );
        assert_eq!(hits(&landing), 0);
    }

    #[tokio::test]
    async fn an_https_to_http_downgrade_hop_is_refused_before_connecting() {
        let landing = hop_server(1, None).await;
        let hop = redirect_once_to_owned(landing.url.replace("127.0.0.1", "example.com")).await;
        let client = guarded_client(&GuardedClientConfig::default()).expect("client");

        let err = client.get(&hop).send().await.expect_err("must be refused");

        assert!(err.is_redirect(), "{err}");
        let reason = guard_error(&err).expect("guard verdict");
        assert!(
            reason.contains("http url only permitted for loopback hosts"),
            "{reason}"
        );
        assert_eq!(hits(&landing), 0, "a refused hop never opens a socket");
    }

    #[tokio::test]
    async fn one_hop_past_the_cap_is_refused_as_too_many_redirects() {
        let cap = DEFAULT_MAX_REDIRECTS;
        let server = hop_server(cap + 2, None).await;
        let client = guarded_client(
            &GuardedClientConfig::default().with_trusted_hosts(vec!["127.0.0.1".to_owned()]),
        )
        .expect("client");

        let err = client
            .get(&server.url)
            .send()
            .await
            .expect_err("must be refused");

        let reason = guard_error(&err).expect("guard verdict");
        assert_eq!(reason, format!("more than {cap} redirects"));
        assert_eq!(
            hits(&server),
            cap + 1,
            "the initial request plus exactly `cap` followed hops reach the wire"
        );
    }

    #[tokio::test]
    async fn a_chain_within_the_cap_is_followed_to_the_end() {
        let server = hop_server(DEFAULT_MAX_REDIRECTS + 1, None).await;
        let client = guarded_client(
            &GuardedClientConfig::default().with_trusted_hosts(vec!["127.0.0.1".to_owned()]),
        )
        .expect("client");

        let response = client.get(&server.url).send().await.expect("followed");

        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.text().await.expect("body"), "landed");
    }

    #[tokio::test]
    async fn a_redirect_to_trusted_loopback_is_followed() {
        // Why: `allow_loopback` is the operator exemption for local services;
        // the resolver must let `localhost` through when it is granted.
        let landing = hop_server(1, None).await;
        let hop = redirect_once_to_owned(landing.url.replace("127.0.0.1", "localhost")).await;
        let client = guarded_client(&GuardedClientConfig::default()).expect("client");

        let response = client.get(&hop).send().await.expect("followed");

        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(hits(&landing), 1);
    }

    #[tokio::test]
    async fn a_zero_redirect_cap_surfaces_the_redirect_unfollowed() {
        let url = redirect_once_to("https://example.com/next").await;
        let client =
            guarded_client(&GuardedClientConfig::default().with_max_redirects(0)).expect("client");
        let response = client
            .get(&url)
            .send()
            .await
            .expect("the 3xx is the answer");
        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
        assert_eq!(
            response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok()),
            Some("https://example.com/next"),
            "nothing was followed, so the caller sees where it would have gone"
        );
    }
}

// Why: these cases need outbound DNS, so they are gated on
// `SP_SSRF_NET_TESTS=1` and otherwise pass after printing that they were
// skipped.
mod ssrf_live_dns_tests {
    use reqwest::dns::Resolve;
    use systemprompt_client::{GuardedConnectError, GuardedResolver};

    const GATE: &str = "SP_SSRF_NET_TESTS";

    fn gated() -> bool {
        if std::env::var(GATE).is_ok_and(|v| v == "1") {
            return true;
        }
        eprintln!("skipped: set {GATE}=1 to run real-DNS SSRF cases");
        false
    }

    async fn refusal(host: &str) -> GuardedConnectError {
        let resolver = GuardedResolver::new(Vec::new());
        let err = Resolve::resolve(&resolver, host.parse().expect("hostname"))
            .await
            .err()
            .expect("resolution must be refused");
        *err.downcast::<GuardedConnectError>()
            .expect("the resolver surfaces its own error type")
    }

    #[tokio::test]
    async fn ssrf_nip_io_name_for_the_metadata_address_is_refused() {
        // skip-ok: needs outbound DNS (SP_SSRF_NET_TESTS=1)
        if !gated() {
            return;
        }
        match refusal("169.254.169.254.nip.io").await {
            GuardedConnectError::BlockedAddress { host, addr } => {
                assert_eq!(host, "169.254.169.254.nip.io");
                assert_eq!(addr.to_string(), "169.254.169.254");
            },
            GuardedConnectError::Unresolvable(host) => {
                panic!("{host} did not resolve; is outbound DNS available?")
            },
            other => panic!("expected BlockedAddress, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn ssrf_gcp_metadata_hostname_is_refused_or_unresolvable() {
        // skip-ok: needs outbound DNS (SP_SSRF_NET_TESTS=1)
        if !gated() {
            return;
        }
        // Why: `metadata.google.internal` only resolves inside GCP, where it
        // points at 169.254.169.254; anywhere else it must not resolve at all.
        match refusal("metadata.google.internal").await {
            GuardedConnectError::BlockedAddress { addr, .. } => {
                assert!(addr.to_string().starts_with("169.254."), "{addr}");
            },
            GuardedConnectError::Unresolvable(_) => {},
            other => panic!("unexpected verdict {other:?}"),
        }
    }
}
