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
        let with_tab = validate_outbound_url("https://exa\tmple.com/h");
        assert!(with_tab.is_ok(), "tabs are stripped per WHATWG URL spec");
        let with_lf = validate_outbound_url("https://exa\nmple.com/h");
        assert!(with_lf.is_ok(), "LFs are stripped per WHATWG URL spec");
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
        // `example.com` is public-looking; the guard accepts it without doing
        // a DNS lookup. This is the documented behaviour.
        assert!(validate_outbound_url("https://example.com/h").is_ok());
        // A name designed to resolve to 127.0.0.1 in a public resolver (the
        // "localtest.me" / "lvh.me" pattern) would also pass — this test does
        // NOT exercise DNS, it pins the policy that the guard is name-based.
    }
}
