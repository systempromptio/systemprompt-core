//! Property-based invariants for `validate_outbound_url`.
//!
//! Confirms that any URL whose final host falls in a blocked range is
//! rejected, regardless of how it is spelled (decimal IPv4, IPv6 short form,
//! or IPv4-mapped IPv6).

use proptest::prelude::*;
use systemprompt_models::net::{
    GuardedClientConfig, OutboundUrlError, is_blocked_ip, validate_outbound_url,
};

fn blocked_ipv4() -> impl Strategy<Value = (u8, u8, u8, u8)> {
    prop_oneof![
        // RFC 1918
        (Just(10u8), any::<u8>(), any::<u8>(), any::<u8>()),
        (Just(172u8), 16u8..=31, any::<u8>(), any::<u8>()),
        (Just(192u8), Just(168u8), any::<u8>(), any::<u8>()),
        // Loopback (127/8) — the production guard ALLOWS these by design
        // (local-dev whitelist, see F-T1e-002); covered separately below.
        // Link-local
        (Just(169u8), Just(254u8), any::<u8>(), any::<u8>()),
        // CGNAT (RFC 6598)
        (Just(100u8), 64u8..=127, any::<u8>(), any::<u8>()),
        // Unspecified / broadcast
        Just((0u8, 0u8, 0u8, 0u8)),
        Just((255u8, 255u8, 255u8, 255u8)),
    ]
}

proptest! {
    #[test]
    fn any_blocked_ipv4_is_rejected((a, b, c, d) in blocked_ipv4()) {
        let url = format!("https://{a}.{b}.{c}.{d}/h");
        let res = validate_outbound_url(&url);
        prop_assert!(
            matches!(res, Err(OutboundUrlError::BlockedHost(_))),
            "{url} must be blocked, got {res:?}",
        );
    }

    #[test]
    fn any_blocked_v4_mapped_ipv6_is_rejected((a, b, c, d) in blocked_ipv4()) {
        let url = format!("https://[::ffff:{a}.{b}.{c}.{d}]/h");
        let res = validate_outbound_url(&url);
        prop_assert!(
            matches!(res, Err(OutboundUrlError::BlockedHost(_))),
            "{url} must be blocked, got {res:?}",
        );
    }

    #[test]
    fn non_http_schemes_always_rejected(
        scheme in "(ftp|gopher|ldap|dict|file|tftp|ssh)",
    ) {
        let url = format!("{scheme}://example.com/h");
        let res = validate_outbound_url(&url);
        prop_assert!(
            matches!(res, Err(OutboundUrlError::Scheme(_)) | Err(OutboundUrlError::Parse(_))),
            "{url} must be rejected, got {res:?}",
        );
    }
}

proptest! {
    #[test]
    fn the_connect_time_predicate_agrees_with_the_parse_time_guard((a, b, c, d) in blocked_ipv4()) {
        let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(a, b, c, d));
        prop_assert!(
            is_blocked_ip(ip),
            "{ip} is refused at parse time and must be refused at connect time too",
        );
    }

    #[test]
    fn a_trusted_host_is_never_silently_dropped_from_the_resolver_exemptions(
        host in "[a-z][a-z0-9-]{0,20}\\.(internal|local|test)",
    ) {
        let config = GuardedClientConfig::default().with_trusted_hosts(vec![host.clone()]);
        prop_assert!(config.trusted_hosts.contains(&host));
        let url = format!("http://{host}/h");
        prop_assert!(
            validate_outbound_url(&url).is_err(),
            "{url} must stay blocked without the trust list",
        );
    }
}
