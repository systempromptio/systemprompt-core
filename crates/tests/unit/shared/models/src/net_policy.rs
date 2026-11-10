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
