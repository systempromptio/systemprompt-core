//! Topology-correct `server.trusted_proxies` defaults and coverage checks.
//!
//! Cloud deployments always sit behind Cloudflare → Fly, so a generated cloud
//! profile must trust the private/Fly/Cloudflare ranges — omitting the Fly
//! peer range makes every request resolve to the proxy's private address and
//! silently discards forwarded client-IP headers, a footgun that has caused a
//! production attribution outage. Local profiles trust loopback and RFC1918
//! so a local reverse proxy works out of the box.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use ipnet::IpNet;

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
pub fn default_cloud_trusted_proxies() -> Vec<IpNet> {
    let mut nets = parse_ranges(proxies::PRIVATE_RANGES);
    nets.extend(parse_ranges(proxies::FLY_PRIVATE_RANGES));
    nets.extend(parse_ranges(proxies::CLOUDFLARE_RANGES));
    nets
}

#[must_use]
pub fn default_local_trusted_proxies() -> Vec<IpNet> {
    parse_ranges(proxies::PRIVATE_RANGES)
}

#[must_use]
pub fn covers_fly_peer(trusted: &[IpNet]) -> bool {
    parse_ranges(proxies::FLY_PRIVATE_RANGES)
        .iter()
        .all(|fly| trusted.iter().any(|net| net.contains(fly)))
}
