//! Topology-correct `server.trusted_proxies` presets and coverage checks.
//!
//! The preset follows the profile's [`HostingProvider`]. The managed cloud
//! sits behind Cloudflare → Fly, so it trusts the private/Fly/Cloudflare
//! ranges — omitting the Fly peer range makes every request resolve to the
//! proxy's private address and silently discards forwarded client-IP headers.
//! Traffic that enters through Fly's *public* edge rather than the private
//! mesh appends a further hop from [`proxies::FLY_PUBLIC_RANGES`], so that
//! range must be trusted too. GKE adds Google's load-balancer ranges;
//! Kubernetes and local Docker trust loopback and RFC1918 only, so a
//! self-managed ingress is never silently granted a third party's ranges.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use ipnet::IpNet;
use systemprompt_manifest::HostingProvider;

use crate::constants::proxies;

fn parse_ranges(ranges: &[&str]) -> Vec<IpNet> {
    ranges
        .iter()
        .filter_map(|cidr| match cidr.parse::<IpNet>() {
            Ok(net) => Some(net),
            Err(err) => {
                tracing::warn!(error = %err, cidr, "invalid built-in proxy CIDR");
                None
            },
        })
        .collect()
}

#[must_use]
pub fn default_trusted_proxies(provider: HostingProvider) -> Vec<IpNet> {
    let mut nets = parse_ranges(proxies::PRIVATE_RANGES);
    match provider {
        HostingProvider::Systemprompt => {
            nets.extend(parse_ranges(proxies::FLY_PRIVATE_RANGES));
            nets.extend(parse_ranges(proxies::FLY_PUBLIC_RANGES));
            nets.extend(parse_ranges(proxies::CLOUDFLARE_RANGES));
        },
        HostingProvider::Gke => nets.extend(parse_ranges(proxies::GOOGLE_LB_RANGES)),
        HostingProvider::Kubernetes | HostingProvider::DockerLocal => {},
    }
    nets
}

#[must_use]
pub fn covers_fly_peer(trusted: &[IpNet]) -> bool {
    covers_all(proxies::FLY_PRIVATE_RANGES, trusted)
}

#[must_use]
pub fn covers_fly_public_edge(trusted: &[IpNet]) -> bool {
    covers_all(proxies::FLY_PUBLIC_RANGES, trusted)
}

fn covers_all(required: &[&str], trusted: &[IpNet]) -> bool {
    parse_ranges(required)
        .iter()
        .all(|required| trusted.iter().any(|net| net.contains(required)))
}
