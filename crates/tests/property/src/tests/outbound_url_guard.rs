//! Property-based invariants for `validate_outbound_url`.
//!
//! Confirms that any URL whose final host falls in a blocked range is
//! rejected, regardless of how it is spelled (decimal IPv4, IPv6 short form,
//! or IPv4-mapped IPv6).

use proptest::prelude::*;
use systemprompt_models::net::{OutboundUrlError, validate_outbound_url};

fn ipv4_in_blocked_range(a: u8, b: u8, c: u8, d: u8) -> bool {
    // RFC 1918, loopback, link-local, CGNAT (RFC 6598), unspecified, broadcast.
    matches!(
        (a, b, c, d),
        (10, _, _, _)
            | (127, _, _, _)
            | (169, 254, _, _)
            | (172, 16..=31, _, _)
            | (192, 168, _, _)
            | (100, 64..=127, _, _)
            | (0, 0, 0, 0)
            | (255, 255, 255, 255)
    )
}

proptest! {
    /// Any IPv4 address in a documented blocked range must be rejected
    /// when used as the host of an `https://` URL.
    #[test]
    fn any_blocked_ipv4_is_rejected(
        a in any::<u8>(),
        b in any::<u8>(),
        c in any::<u8>(),
        d in any::<u8>(),
    ) {
        prop_assume!(ipv4_in_blocked_range(a, b, c, d));
        // 127/8 over https is rejected by the BlockedHost branch; loopback
        // fast-path only kicks in for the special-cased loopback names
        // `localhost`, `127.0.0.1`, `::1`. We construct arbitrary 127.x.y.z
        // — which is not the literal `127.0.0.1` — so the fast-path does
        // NOT cover them and `BlockedHost` is the expected variant for all
        // ranges except the literal loopback IP.
        let url = format!("https://{a}.{b}.{c}.{d}/h");
        let res = validate_outbound_url(&url);
        match (a, b, c, d) {
            (127, 0, 0, 1) => prop_assert!(res.is_ok()),
            _ => prop_assert!(
                matches!(res, Err(OutboundUrlError::BlockedHost(_))),
                "{url} must be blocked, got {res:?}",
            ),
        }
    }

    /// An IPv4-mapped IPv6 address whose embedded IPv4 falls in a blocked
    /// range must be rejected — it must not bypass the v4 block list.
    #[test]
    fn any_blocked_v4_mapped_ipv6_is_rejected(
        a in any::<u8>(),
        b in any::<u8>(),
        c in any::<u8>(),
        d in any::<u8>(),
    ) {
        prop_assume!(ipv4_in_blocked_range(a, b, c, d));
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
