use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use systemprompt_models::net::{
    LoopbackPolicy, OutboundUrlError, is_loopback_host, pin_resolved, validate_outbound_url,
    validate_outbound_url_with_trust,
};

mod loopback_policy_tests {
    use super::*;

    const LOOPBACK_URLS: [&str; 7] = [
        "https://localhost/h",
        "https://LOCALHOST./h",
        "https://api.localhost/h",
        "https://127.0.0.1/h",
        "https://127.8.9.10/h",
        "https://[::1]/h",
        "https://2130706433/h",
    ];

    #[test]
    fn deny_refuses_every_loopback_spelling() {
        for url in LOOPBACK_URLS {
            assert!(
                validate_outbound_url(url, LoopbackPolicy::Deny).is_err(),
                "{url} must be refused"
            );
        }
    }

    #[test]
    fn allow_accepts_every_loopback_spelling() {
        for url in LOOPBACK_URLS {
            assert!(
                validate_outbound_url(url, LoopbackPolicy::Allow).is_ok(),
                "{url} must be accepted"
            );
        }
    }

    #[test]
    fn deny_names_a_loopback_domain_as_loopback() {
        assert!(matches!(
            validate_outbound_url("https://localhost:8443/h", LoopbackPolicy::Deny),
            Err(OutboundUrlError::Loopback(host)) if host == "localhost"
        ));
    }

    #[test]
    fn deny_refuses_plain_http_to_loopback() {
        assert!(matches!(
            validate_outbound_url("http://127.0.0.1:8080/h", LoopbackPolicy::Deny),
            Err(OutboundUrlError::NonLoopbackHttp)
        ));
    }

    #[test]
    fn allow_still_applies_the_block_list_to_non_loopback_hosts() {
        for url in [
            "https://0.0.0.0/h",
            "https://10.1.2.3/h",
            "https://169.254.169.254/latest/meta-data",
            "https://100.64.0.1/h",
            "https://[fd00::1]/h",
            "https://[fe80::1]/h",
            "https://[::ffff:169.254.169.254]/h",
            "https://[::ffff:127.0.0.1]/h",
            "https://0xa9fea9fe/h",
            "https://0251.0376.0251.0376/h",
        ] {
            assert!(
                matches!(
                    validate_outbound_url(url, LoopbackPolicy::Allow),
                    Err(OutboundUrlError::BlockedHost(_))
                ),
                "{url} must be blocked"
            );
        }
    }

    #[test]
    fn a_trusted_host_stays_reachable_under_deny() {
        let trusted = ["mock-inference"];
        assert!(
            validate_outbound_url_with_trust(
                "http://mock-inference/h",
                &trusted,
                LoopbackPolicy::Deny
            )
            .is_ok()
        );
    }

    #[test]
    fn loopback_host_detection_covers_names_and_literals() {
        let url = url::Url::parse("https://a.b.localhost/").expect("url");
        assert!(is_loopback_host(&url.host().expect("host")));
        let url = url::Url::parse("https://localhost.example/").expect("url");
        assert!(!is_loopback_host(&url.host().expect("host")));
    }
}

mod pin_resolved_tests {
    use super::*;

    fn url(raw: &str) -> url::Url {
        url::Url::parse(raw).expect("url")
    }

    const NO_TRUST: [&str; 0] = [];

    #[test]
    fn a_public_resolution_is_pinned_to_its_first_address() {
        let addr = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
        let pinned = pin_resolved(
            &url("https://git.example.com/org/repo.git"),
            &[addr],
            &NO_TRUST,
            LoopbackPolicy::Deny,
        )
        .expect("public address pinned");
        assert_eq!(pinned.host(), "git.example.com");
        assert_eq!(pinned.port(), 443);
        assert_eq!(pinned.addr(), addr);
        assert_eq!(
            pinned.curl_resolve_entry(),
            "git.example.com:443:93.184.216.34"
        );
    }

    #[test]
    fn an_ipv6_pin_is_bracketed_for_curl() {
        let addr = IpAddr::V6("2606:2800:220:1::1".parse::<Ipv6Addr>().expect("v6"));
        let pinned = pin_resolved(
            &url("https://git.example.com:8443/repo.git"),
            &[addr],
            &NO_TRUST,
            LoopbackPolicy::Deny,
        )
        .expect("pinned");
        assert_eq!(
            pinned.curl_resolve_entry(),
            "git.example.com:8443:[2606:2800:220:1::1]"
        );
    }

    #[test]
    fn any_private_address_in_the_resolution_refuses_the_pin() {
        let public = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
        for private in [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6(Ipv4Addr::new(127, 0, 0, 1).to_ipv6_mapped()),
        ] {
            let result = pin_resolved(
                &url("https://rebind.example.com/repo.git"),
                &[public, private],
                &NO_TRUST,
                LoopbackPolicy::Deny,
            );
            assert!(
                matches!(result, Err(OutboundUrlError::BlockedAddress { addr, .. }) if addr == private),
                "{private}: {result:?}"
            );
        }
    }

    #[test]
    fn localhost_resolution_is_pinned_only_under_allow() {
        let loopback = [IpAddr::V4(Ipv4Addr::LOCALHOST)];
        let target = url("https://localhost:9443/repo.git");
        assert!(pin_resolved(&target, &loopback, &NO_TRUST, LoopbackPolicy::Deny).is_err());
        let pinned = pin_resolved(&target, &loopback, &NO_TRUST, LoopbackPolicy::Allow)
            .expect("allowed loopback pinned");
        assert_eq!(pinned.curl_resolve_entry(), "localhost:9443:127.0.0.1");
    }

    #[test]
    fn an_empty_resolution_is_refused() {
        assert!(matches!(
            pin_resolved(
                &url("https://git.example.com/"),
                &[],
                &NO_TRUST,
                LoopbackPolicy::Deny
            ),
            Err(OutboundUrlError::Unresolved(_))
        ));
    }
}

mod embedded_v4_tests {
    use super::*;
    use systemprompt_models::net::is_blocked_ip;

    fn v6(raw: &str) -> IpAddr {
        IpAddr::V6(raw.parse::<Ipv6Addr>().expect("v6"))
    }

    #[test]
    fn six_to_four_embedding_a_blocked_v4_is_blocked() {
        for raw in [
            "2002:7f00:1::",
            "2002:a00:1::1",
            "2002:a9fe:a9fe::",
            "2002:c0a8:101::",
            "2002:6440:1::",
            "2002::",
        ] {
            assert!(is_blocked_ip(v6(raw)), "{raw} must be blocked");
        }
    }

    #[test]
    fn six_to_four_embedding_a_public_v4_is_allowed() {
        assert!(!is_blocked_ip(v6("2002:808:808::1")));
    }

    #[test]
    fn teredo_with_a_blocked_server_or_client_is_blocked() {
        for raw in [
            "2001:0:7f00:1::",
            "2001:0:a9fe:a9fe::",
            "2001:0:808:808::80ff:fffe",
            "2001:0:808:808::f5ff:fffe",
            "2001:0:808:808::5601:5601",
        ] {
            assert!(is_blocked_ip(v6(raw)), "{raw} must be blocked");
        }
    }

    #[test]
    fn teredo_with_public_server_and_client_is_allowed() {
        assert!(!is_blocked_ip(v6("2001:0:808:808::f7f7:f7f7")));
    }

    #[test]
    fn nat64_prefixes_embedding_a_blocked_v4_are_blocked() {
        for raw in ["64:ff9b::7f00:1", "64:ff9b::a9fe:a9fe", "64:ff9b:1::a00:1"] {
            assert!(is_blocked_ip(v6(raw)), "{raw} must be blocked");
        }
        assert!(!is_blocked_ip(v6("64:ff9b::808:808")));
    }

    #[test]
    fn untrusted_urls_with_embedded_blocked_v4_are_refused() {
        for url in [
            "https://[2002:7f00:1::]/",
            "https://[2001:0:a9fe:a9fe::]/",
            "https://[64:ff9b:1::a00:1]/",
        ] {
            assert!(
                validate_outbound_url(url, LoopbackPolicy::Deny).is_err(),
                "{url}"
            );
        }
    }
}

mod proxy_environment_tests {
    use systemprompt_models::net::{ProxyEnvironment, proxy_host};

    #[test]
    fn proxy_host_reads_the_host_with_or_without_a_scheme() {
        assert_eq!(
            proxy_host("http://user:pw@Proxy.Corp:3128").as_deref(),
            Some("proxy.corp")
        );
        assert_eq!(proxy_host("proxy.corp:8080").as_deref(), Some("proxy.corp"));
        assert_eq!(proxy_host("  "), None);
    }

    #[test]
    fn no_proxy_bypass_matches_exact_suffix_and_wildcard_entries() {
        let env = ProxyEnvironment::new(&[], None, "localhost, .internal.corp,git.example.com");
        assert!(env.bypasses("localhost"));
        assert!(env.bypasses("repo.internal.corp"));
        assert!(env.bypasses("internal.corp"));
        assert!(env.bypasses("GIT.example.com."));
        assert!(!env.bypasses("evilgit.example.com"));
        assert!(!env.bypasses("github.com"));
        assert!(ProxyEnvironment::new(&[], None, "*").bypasses("anything.example"));
        assert!(!ProxyEnvironment::default().bypasses("anything.example"));
    }

    #[test]
    fn a_proxied_host_routes_through_the_https_proxy() {
        let env = ProxyEnvironment::new(
            &["http://proxy.corp:3128"],
            Some("proxy.corp:3128"),
            ".internal.corp",
        );
        assert_eq!(
            env.https_proxy_for("github.com").map(url::Url::as_str),
            Some("http://proxy.corp:3128/")
        );
        assert_eq!(env.proxy_hosts(), ["proxy.corp".to_owned()]);
    }

    #[test]
    fn a_no_proxy_host_or_an_unset_proxy_takes_the_direct_pinned_route() {
        let env = ProxyEnvironment::new(&[], Some("http://proxy.corp:3128"), ".internal.corp");
        assert!(env.https_proxy_for("git.internal.corp").is_none());
        assert!(
            ProxyEnvironment::default()
                .https_proxy_for("github.com")
                .is_none()
        );
    }
}
