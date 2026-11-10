//! IP ranges the outbound guard refuses: private, loopback, link-local,
//! CGNAT, reserved, multicast, and their IPv4-in-IPv6 embeddings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[must_use]
pub fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => is_blocked_v4(v4),
        std::net::IpAddr::V6(v6) => is_blocked_v6(v6),
    }
}

// Why: RFC 4291 §2.5.5.2 maps `::ffff:0:0/96` to IPv4 and §2.5.5.1 embeds an
// IPv4 address in the low 32 bits of `::/96`; RFC 6052 `64:ff9b::/96` and RFC
// 8215 `64:ff9b:1::/48` are NAT64 prefixes, through which
// `64:ff9b::a9fe:a9fe` reaches 169.254.169.254. RFC 3056 6to4 `2002::/16`
// carries an IPv4 in bits 16-47, and RFC 4380 Teredo `2001::/32` carries the
// server IPv4 in bits 32-63 and the client IPv4, inverted, in the low 32 bits.
// Each embedded IPv4 is judged by the IPv4 table.
pub(super) fn is_blocked_v6(ip: std::net::Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_blocked_v4(v4);
    }
    let segments = ip.segments();
    let embedded = embedded_v4(&segments);
    if !embedded.is_empty() {
        return embedded.into_iter().any(is_blocked_v4);
    }
    let is_unique_local = (segments[0] & 0xfe00) == 0xfc00;
    let is_link_local = (segments[0] & 0xffc0) == 0xfe80;
    let is_multicast = (segments[0] & 0xff00) == 0xff00;
    ip.is_loopback() || ip.is_unspecified() || is_unique_local || is_link_local || is_multicast
}

const fn v4_from(high: u16, low: u16) -> std::net::Ipv4Addr {
    let [a, b] = high.to_be_bytes();
    let [c, d] = low.to_be_bytes();
    std::net::Ipv4Addr::new(a, b, c, d)
}

fn embedded_v4(segments: &[u16; 8]) -> Vec<std::net::Ipv4Addr> {
    let is_nat64 = segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0];
    let is_local_nat64 = segments[..3] == [0x64, 0xff9b, 1];
    let is_ipv4_compatible = segments[..6] == [0, 0, 0, 0, 0, 0];
    if is_nat64 || is_local_nat64 || is_ipv4_compatible {
        return vec![v4_from(segments[6], segments[7])];
    }
    if segments[0] == 0x2002 {
        return vec![v4_from(segments[1], segments[2])];
    }
    if segments[0] == 0x2001 && segments[1] == 0 {
        return vec![
            v4_from(segments[2], segments[3]),
            v4_from(!segments[6], !segments[7]),
        ];
    }
    Vec::new()
}

// Why: RFC 6598 reserves `100.64.0.0/10` for shared carrier-grade NAT, not
// public hosts.
fn is_cgnat_shared_v4(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 100 && (64..=127).contains(&b)
}

// Why: `0.0.0.0/8` routes to the local host on Linux, `192.0.0.0/24` (RFC
// 6890) and `198.18.0.0/15` (RFC 2544) are IETF-reserved, and `224.0.0.0/4` /
// `240.0.0.0/4` are multicast and reserved — none is a public host.
const fn is_reserved_v4(ip: std::net::Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    a == 0 || (a == 192 && b == 0 && c == 0) || (a == 198 && (b == 18 || b == 19)) || a >= 224
}

pub(super) fn is_blocked_v4(ip: std::net::Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || is_cgnat_shared_v4(ip)
        || is_reserved_v4(ip)
}
