use systemprompt_models::net::{
    OutboundUrlError, validate_outbound_url, validate_outbound_url_with_trust,
};

mod validate_outbound_url_tests {
    use super::*;

    #[test]
    fn accepts_https() {
        let url = validate_outbound_url("https://example.com/hook").expect("https allowed");
        assert_eq!(url.scheme(), "https");
    }

    #[test]
    fn accepts_loopback_http() {
        assert!(validate_outbound_url("http://localhost:8080/h").is_ok());
        assert!(validate_outbound_url("http://127.0.0.1/h").is_ok());
        assert!(validate_outbound_url("http://[::1]/h").is_ok());
    }

    #[test]
    fn rejects_cloud_metadata_ip() {
        assert!(matches!(
            validate_outbound_url("https://169.254.169.254/latest/meta-data"),
            Err(OutboundUrlError::BlockedHost(_))
        ));
    }

    #[test]
    fn rejects_rfc1918_ranges() {
        for url in [
            "https://10.0.0.5/h",
            "https://192.168.1.1/h",
            "https://172.20.0.1/h",
        ] {
            assert!(
                matches!(
                    validate_outbound_url(url),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{url} should be blocked",
            );
        }
    }

    #[test]
    fn allows_172_outside_private_block() {
        assert!(validate_outbound_url("https://172.32.0.1/h").is_ok());
        assert!(validate_outbound_url("https://172.15.0.1/h").is_ok());
    }

    #[test]
    fn rejects_non_loopback_http() {
        assert!(matches!(
            validate_outbound_url("http://example.com/h"),
            Err(OutboundUrlError::NonLoopbackHttp)
        ));
    }

    #[test]
    fn rejects_non_http_scheme() {
        assert!(matches!(
            validate_outbound_url("ftp://example.com/h"),
            Err(OutboundUrlError::Scheme(_))
        ));
    }

    #[test]
    fn rejects_malformed_url() {
        assert!(matches!(
            validate_outbound_url("not a url"),
            Err(OutboundUrlError::Parse(_))
        ));
    }
}

mod ssrf_adversarial_tests {
    use super::*;

    // -- IPv4 loopback (127/8) ------------------------------------------------

    // Pins the current production policy: loopback is allow-listed (incl. over
    // https) for local-development webhooks. See finding F-T1e-002 — tightening
    // this in production deployments needs a config-flag conversation.
    #[test]
    fn accepts_ipv4_loopback_over_https_by_design() {
        for ip in ["127.0.0.1", "127.1.2.3", "127.255.255.255"] {
            let url = format!("https://{ip}/h");
            assert!(
                validate_outbound_url(&url).is_ok(),
                "{url} is allow-listed (loopback whitelist by design)",
            );
        }
    }

    // -- IPv4 RFC 1918 private ranges -----------------------------------------

    #[test]
    fn rejects_ipv4_private_10_8() {
        for ip in ["10.0.0.1", "10.255.255.255", "10.42.42.42"] {
            assert!(
                matches!(
                    validate_outbound_url(&format!("https://{ip}/h")),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{ip} should be blocked",
            );
        }
    }

    #[test]
    fn rejects_ipv4_private_172_16_12() {
        for ip in ["172.16.0.1", "172.31.255.255", "172.20.10.10"] {
            assert!(
                matches!(
                    validate_outbound_url(&format!("https://{ip}/h")),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{ip} should be blocked",
            );
        }
    }

    #[test]
    fn rejects_ipv4_private_192_168_16() {
        for ip in ["192.168.0.1", "192.168.255.254"] {
            assert!(
                matches!(
                    validate_outbound_url(&format!("https://{ip}/h")),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{ip} should be blocked",
            );
        }
    }

    // -- IPv4 link-local + cloud metadata -------------------------------------

    #[test]
    fn rejects_ipv4_link_local_169_254_0_0_16() {
        for ip in ["169.254.0.1", "169.254.169.254", "169.254.255.255"] {
            assert!(
                matches!(
                    validate_outbound_url(&format!("https://{ip}/h")),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{ip} should be blocked (link-local / AWS+GCP metadata)",
            );
        }
    }

    // -- IPv4 CGNAT shared (RFC 6598) -----------------------------------------

    #[test]
    fn rejects_ipv4_cgnat_shared_100_64_10() {
        for ip in ["100.64.0.1", "100.127.255.254", "100.100.100.100"] {
            assert!(
                matches!(
                    validate_outbound_url(&format!("https://{ip}/h")),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{ip} should be blocked (CGNAT)",
            );
        }
    }

    #[test]
    fn allows_100_outside_cgnat_block() {
        // 100.0.0.0/24 and 100.128.0.0/9 are public.
        assert!(validate_outbound_url("https://100.63.255.255/h").is_ok());
        assert!(validate_outbound_url("https://100.128.0.1/h").is_ok());
    }

    // -- IPv4 unspecified + broadcast -----------------------------------------

    #[test]
    fn rejects_ipv4_unspecified_and_broadcast() {
        for ip in ["0.0.0.0", "255.255.255.255"] {
            assert!(
                matches!(
                    validate_outbound_url(&format!("https://{ip}/h")),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{ip} should be blocked",
            );
        }
    }

    // -- IPv6 loopback / link-local / ULA -------------------------------------

    #[test]
    fn rejects_ipv6_loopback_over_https() {
        // ::1 is loopback; the loopback fast-path accepts it (operator opt-in
        // for local dev). The point of this test is to pin that behaviour:
        // any other IPv6 in ::1's space is `unspecified`/loopback and blocked.
        assert!(validate_outbound_url("https://[::1]/h").is_ok());
        assert!(matches!(
            validate_outbound_url("https://[::]/h"),
            Err(OutboundUrlError::BlockedHost(_))
        ));
    }

    #[test]
    fn rejects_ipv6_link_local_fe80() {
        for host in ["[fe80::1]", "[febf::ffff:ffff:ffff:ffff]"] {
            let url = format!("https://{host}/h");
            assert!(
                matches!(
                    validate_outbound_url(&url),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{url} should be blocked (fe80::/10)",
            );
        }
    }

    #[test]
    fn rejects_ipv6_unique_local_fc00_7() {
        for host in ["[fc00::1]", "[fd00::1]", "[fdff::ffff]"] {
            let url = format!("https://{host}/h");
            assert!(
                matches!(
                    validate_outbound_url(&url),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{url} should be blocked (fc00::/7)",
            );
        }
    }

    #[test]
    fn rejects_ipv6_aws_metadata_endpoint() {
        assert!(matches!(
            validate_outbound_url("https://[fd00:ec2::254]/latest/meta-data"),
            Err(OutboundUrlError::BlockedHost(_))
        ));
    }

    // -- IPv4-mapped IPv6 (must not bypass v4 blocks) -------------------------

    #[test]
    fn rejects_ipv4_mapped_ipv6_loopback() {
        // ::ffff:127.0.0.1 — a hand-crafted v4-mapped address must be treated
        // as the underlying IPv4 (loopback) rather than falling through the
        // generic IPv6 branch, which would otherwise accept it.
        for host in ["[::ffff:127.0.0.1]", "[::ffff:7f00:1]"] {
            let url = format!("https://{host}/h");
            assert!(
                matches!(
                    validate_outbound_url(&url),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{url} should be blocked (v4-mapped loopback)",
            );
        }
    }

    #[test]
    fn rejects_ipv4_mapped_ipv6_metadata() {
        for host in ["[::ffff:169.254.169.254]", "[::ffff:a9fe:a9fe]"] {
            let url = format!("https://{host}/h");
            assert!(
                matches!(
                    validate_outbound_url(&url),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{url} should be blocked (v4-mapped metadata)",
            );
        }
    }

    #[test]
    fn rejects_ipv4_mapped_ipv6_rfc1918() {
        for host in [
            "[::ffff:10.0.0.1]",
            "[::ffff:192.168.1.1]",
            "[::ffff:172.16.0.1]",
        ] {
            let url = format!("https://{host}/h");
            assert!(
                matches!(
                    validate_outbound_url(&url),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{url} should be blocked (v4-mapped RFC1918)",
            );
        }
    }

    // -- Scheme allow-list ----------------------------------------------------

    #[test]
    fn rejects_file_scheme() {
        assert!(matches!(
            validate_outbound_url("file:///etc/passwd"),
            Err(OutboundUrlError::Scheme(_)) | Err(OutboundUrlError::Parse(_))
        ));
    }

    #[test]
    fn rejects_gopher_scheme() {
        assert!(matches!(
            validate_outbound_url("gopher://example.com/_GET"),
            Err(OutboundUrlError::Scheme(_))
        ));
    }

    #[test]
    fn rejects_ftp_scheme() {
        assert!(matches!(
            validate_outbound_url("ftp://example.com/x"),
            Err(OutboundUrlError::Scheme(_))
        ));
    }

    #[test]
    fn rejects_data_scheme() {
        // data: URLs have no host; the guard rejects them either at the host
        // check or scheme check — either is acceptable.
        let err = validate_outbound_url("data:,Hello%2C%20World").unwrap_err();
        assert!(matches!(
            err,
            OutboundUrlError::Scheme(_) | OutboundUrlError::Parse(_)
        ));
    }

    // -- URL parser oddities --------------------------------------------------

    #[test]
    fn userinfo_does_not_change_host_evaluation() {
        assert!(validate_outbound_url("http://user:pass@127.0.0.1/").is_ok());
        assert!(matches!(
            validate_outbound_url("http://user:pass@1.2.3.4/"),
            Err(OutboundUrlError::NonLoopbackHttp)
        ));
        assert!(matches!(
            validate_outbound_url("https://user:pass@169.254.169.254/"),
            Err(OutboundUrlError::BlockedHost(_))
        ));
    }

    #[test]
    fn fragment_after_host_does_not_shift_host() {
        let url = validate_outbound_url("https://example.com/#@127.0.0.1").expect("valid");
        assert_eq!(url.host_str(), Some("example.com"));
    }

    #[test]
    fn rejects_url_with_embedded_control_chars() {
        // Whatwg URL parser strips tab/CR/LF; check both that a leading/inline
        // control char does not cause the guard to misread the host.
        let with_tab = validate_outbound_url("https://exa\tmple.com/h").expect("tabs stripped");
        assert_eq!(with_tab.host_str(), Some("example.com"));
        assert!(!with_tab.as_str().contains('\t'));
        let with_lf = validate_outbound_url("https://exa\nmple.com/h").expect("LFs stripped");
        assert_eq!(with_lf.host_str(), Some("example.com"));
        assert!(!with_lf.as_str().contains('\n'));
    }

    #[test]
    fn percent_encoded_ipv4_is_decoded_and_classified() {
        let res = validate_outbound_url("http://%31%32%37.0.0.1/h");
        // The url crate may either decode or reject percent-encoded host
        // characters depending on version; both are safe outcomes (decoded =
        // loopback accepted, parse error = closed). Reject any outcome that
        // accepts the URL with a non-loopback host.
        match res {
            Ok(u) => {
                assert_eq!(u.host_str(), Some("127.0.0.1"), "must decode to loopback");
            },
            Err(OutboundUrlError::Parse(_)) => {},
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn idn_homograph_is_treated_as_public_domain() {
        // xn--lcalhst-0za is a Punycode label that visually resembles
        // "localhost" but is a distinct domain.
        let res =
            validate_outbound_url("https://xn--lcalhst-0za.example/h").expect("public domain");
        assert!(res.host_str().unwrap().starts_with("xn--"));
    }

    // -- Trusted-http allowlist (sealed-network opt-in) ----------------------

    #[test]
    fn trusted_http_host_accepts_plain_http() {
        let trusted = ["mock-inference"];
        let url =
            validate_outbound_url_with_trust("http://mock-inference:8080/v1/messages", &trusted)
                .expect("trusted http host accepted");
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("mock-inference"));
    }

    #[test]
    fn trusted_http_match_is_case_insensitive() {
        let trusted = ["Mock-Inference"];
        assert!(validate_outbound_url_with_trust("http://MOCK-INFERENCE:80/h", &trusted).is_ok());
        assert!(validate_outbound_url_with_trust("http://mock-inference/h", &trusted).is_ok());
    }

    #[test]
    fn trusted_http_does_not_match_substring_or_sibling() {
        let trusted = ["mock-inference"];
        assert!(matches!(
            validate_outbound_url_with_trust("http://other-inference/h", &trusted),
            Err(OutboundUrlError::NonLoopbackHttp)
        ));
        assert!(matches!(
            validate_outbound_url_with_trust("http://api.mock-inference/h", &trusted),
            Err(OutboundUrlError::NonLoopbackHttp)
        ));
    }

    #[test]
    fn empty_trusted_list_matches_legacy_behaviour() {
        assert!(matches!(
            validate_outbound_url_with_trust("http://example.com/h", &[] as &[&str]),
            Err(OutboundUrlError::NonLoopbackHttp)
        ));
        assert!(validate_outbound_url_with_trust("https://example.com/h", &[] as &[&str]).is_ok());
    }

    #[test]
    fn trusted_host_under_https_still_passes() {
        let trusted = ["mock-inference"];
        assert!(validate_outbound_url_with_trust("https://mock-inference/h", &trusted).is_ok());
    }

    #[test]
    fn trusted_list_does_not_unblock_metadata_ip_for_others() {
        let trusted = ["mock-inference"];
        assert!(matches!(
            validate_outbound_url_with_trust("https://169.254.169.254/x", &trusted),
            Err(OutboundUrlError::BlockedHost(_))
        ));
    }

    #[test]
    fn trusted_list_is_name_based_not_resolved() {
        let trusted = ["mock-inference"];
        // The guard does not resolve "mock-inference" — it accepts the URL based
        // on the literal hostname. This is the documented behaviour.
        let url = validate_outbound_url_with_trust("http://mock-inference:8080/h", &trusted)
            .expect("trusted name accepted");
        assert_eq!(url.host_str(), Some("mock-inference"));
    }

    #[test]
    fn legacy_entry_point_remains_strict() {
        assert!(matches!(
            validate_outbound_url("http://mock-inference/h"),
            Err(OutboundUrlError::NonLoopbackHttp)
        ));
    }

    #[test]
    fn hostnames_are_not_resolved_at_validation_time() {
        // The parse-time guard is a pre-filter only: a public-looking name is
        // accepted here and rejected at connect time by `GuardedResolver`
        // (see `guarded_client_tests`).
        assert!(validate_outbound_url("https://example.com/h").is_ok());
    }
}

mod guarded_client_tests {
    use std::io::Error as IoError;

    use reqwest::dns::Resolve;
    use systemprompt_models::net::{
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

    /// A listener that answers `hits` requests. Every request but the last is
    /// a 302 to `next` (or, when `next` is `None`, back to itself); the last
    /// is a 200 with the body `landed`.
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

/// Real-DNS cases for the cloud-metadata names an attacker would actually use.
/// They need outbound DNS, so they are gated on `SP_SSRF_NET_TESTS=1` and
/// otherwise pass after printing that they were skipped.
mod ssrf_live_dns_tests {
    use reqwest::dns::Resolve;
    use systemprompt_models::net::{GuardedConnectError, GuardedResolver};

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
