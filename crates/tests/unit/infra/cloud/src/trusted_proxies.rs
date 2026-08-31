//! Unit tests for the trusted-proxy defaults and Fly-peer coverage check.

use systemprompt_cloud::trusted_proxies::{
    covers_fly_peer, covers_fly_public_edge, default_cloud_trusted_proxies,
    default_local_trusted_proxies,
};

fn as_strings(nets: &[ipnet::IpNet]) -> Vec<String> {
    nets.iter().map(ToString::to_string).collect()
}

#[test]
fn cloud_defaults_cover_private_fly_and_cloudflare_ranges() {
    let nets = default_cloud_trusted_proxies();
    let strings = as_strings(&nets);
    for expected in [
        "127.0.0.0/8",
        "10.0.0.0/8",
        "fc00::/7",
        "66.241.64.0/18",
        "104.16.0.0/13",
        "2606:4700::/32",
    ] {
        assert!(strings.contains(&expected.to_owned()), "missing {expected}");
    }
    assert!(covers_fly_peer(&nets));
    assert!(covers_fly_public_edge(&nets));
}

#[test]
fn fly_peer_coverage_does_not_imply_public_edge_coverage() {
    let peer_only: Vec<ipnet::IpNet> = vec!["fc00::/7".parse().unwrap()];
    assert!(covers_fly_peer(&peer_only));
    assert!(!covers_fly_public_edge(&peer_only));
}

#[test]
fn covers_fly_public_edge_accepts_exact_and_supernet_ranges() {
    let exact: Vec<ipnet::IpNet> = vec!["66.241.64.0/18".parse().unwrap()];
    assert!(covers_fly_public_edge(&exact));
    let supernet: Vec<ipnet::IpNet> = vec!["66.241.0.0/16".parse().unwrap()];
    assert!(covers_fly_public_edge(&supernet));
    let subnet: Vec<ipnet::IpNet> = vec!["66.241.124.0/24".parse().unwrap()];
    assert!(!covers_fly_public_edge(&subnet));
    assert!(!covers_fly_public_edge(&[]));
}

#[test]
fn local_defaults_are_private_only() {
    let nets = default_local_trusted_proxies();
    let strings = as_strings(&nets);
    assert!(strings.contains(&"127.0.0.0/8".to_owned()));
    assert!(strings.contains(&"192.168.0.0/16".to_owned()));
    assert!(!strings.iter().any(|s| s == "fc00::/7"));
    assert!(!strings.iter().any(|s| s == "66.241.64.0/18"));
    assert!(!strings.iter().any(|s| s == "104.16.0.0/13"));
}

#[test]
fn covers_fly_peer_rejects_empty_and_unrelated_sets() {
    assert!(!covers_fly_peer(&[]));
    let unrelated = vec!["10.0.0.0/8".parse().unwrap()];
    assert!(!covers_fly_peer(&unrelated));
}

#[test]
fn covers_fly_peer_accepts_exact_and_supernet_ranges() {
    let exact = vec!["fc00::/7".parse().unwrap()];
    assert!(covers_fly_peer(&exact));
    let supernet = vec!["fc00::/6".parse().unwrap()];
    assert!(covers_fly_peer(&supernet));
    let subnet = vec!["fdaa::/16".parse().unwrap()];
    assert!(!covers_fly_peer(&subnet));
}

// Why: `parse_ranges` drops a CIDR it cannot parse, logging a warning and
// carrying on. A typo in a built-in range therefore does not fail anything —
// that range is simply absent from the trusted set, and a proxy that should
// have been trusted no longer is, so client IPs resolve to the proxy's address
// and rate limits and bans land on the proxy instead of the caller.
//
// The tests above name six ranges explicitly; there are more than twenty. This
// asserts that none were dropped, by count, so a typo anywhere in the lists
// fails here without this test having to restate them.
#[test]
fn every_built_in_proxy_range_parses_rather_than_being_dropped() {
    use systemprompt_cloud::constants::proxies;

    let declared = proxies::PRIVATE_RANGES.len()
        + proxies::FLY_PRIVATE_RANGES.len()
        + proxies::FLY_PUBLIC_RANGES.len()
        + proxies::CLOUDFLARE_RANGES.len();

    assert_eq!(
        default_cloud_trusted_proxies().len(),
        declared,
        "a built-in proxy CIDR failed to parse and was silently dropped"
    );
}

#[test]
fn every_private_range_parses_for_the_local_default() {
    use systemprompt_cloud::constants::proxies;

    assert_eq!(
        default_local_trusted_proxies().len(),
        proxies::PRIVATE_RANGES.len(),
        "a private-range CIDR failed to parse and was silently dropped"
    );
}
