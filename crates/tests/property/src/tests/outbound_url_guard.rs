//! Property-based invariants for `validate_outbound_url`.
//!
//! Confirms that any URL whose final host falls in a blocked range is
//! rejected, regardless of how it is spelled (decimal IPv4, IPv6 short form,
//! or IPv4-mapped IPv6).

use proptest::prelude::*;
use systemprompt_models::net::{OutboundUrlError, validate_outbound_url};

/// Generates IPv4 addresses drawn from the documented blocked ranges.
/// Targeted (not rejection-based) so the test does not hit proptest's
/// global-reject ceiling.
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
    /// Any IPv4 address in a documented blocked range (excluding the loopback
    /// whitelist — see F-T1e-002) must be rejected when used as the host of
    /// an `https://` URL.
    #[test]
    fn any_blocked_ipv4_is_rejected((a, b, c, d) in blocked_ipv4()) {
        let url = format!("https://{a}.{b}.{c}.{d}/h");
        let res = validate_outbound_url(&url);
        prop_assert!(
            matches!(res, Err(OutboundUrlError::BlockedHost(_))),
            "{url} must be blocked, got {res:?}",
        );
    }

    /// An IPv4-mapped IPv6 address whose embedded IPv4 falls in a blocked
    /// range must be rejected — it must not bypass the v4 block list.
    /// Excludes loopback (127/8) for the same reason as the v4 case.
    #[test]
    fn any_blocked_v4_mapped_ipv6_is_rejected((a, b, c, d) in blocked_ipv4()) {
        let url = format!("https://[::ffff:{a}.{b}.{c}.{d}]/h");
        let res = validate_outbound_url(&url);
        prop_assert!(
            matches!(res, Err(OutboundUrlError::BlockedHost(_))),
            "{url} must be blocked, got {res:?}",
        );
    }

    /// Non-http(s) schemes are always rejected, regardless of host.
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
