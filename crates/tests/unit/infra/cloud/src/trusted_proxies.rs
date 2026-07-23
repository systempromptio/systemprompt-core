//! Unit tests for the trusted-proxy defaults and Fly-peer coverage check.

use systemprompt_cloud::trusted_proxies::{
    covers_fly_peer, default_cloud_trusted_proxies, default_local_trusted_proxies,
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
        "104.16.0.0/13",
        "2606:4700::/32",
    ] {
        assert!(strings.contains(&expected.to_owned()), "missing {expected}");
    }
    assert!(covers_fly_peer(&nets));
}

#[test]
fn local_defaults_are_private_only() {
    let nets = default_local_trusted_proxies();
    let strings = as_strings(&nets);
    assert!(strings.contains(&"127.0.0.0/8".to_owned()));
    assert!(strings.contains(&"192.168.0.0/16".to_owned()));
    assert!(!strings.iter().any(|s| s == "fc00::/7"));
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
