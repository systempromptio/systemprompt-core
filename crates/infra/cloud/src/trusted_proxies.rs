//! Topology-correct `server.trusted_proxies` defaults and coverage checks.
//!
//! Cloud deployments always sit behind Cloudflare → Fly, so a generated cloud
//! profile must trust the private/Fly/Cloudflare ranges — omitting the Fly
//! peer range makes every request resolve to the proxy's private address and
//! silently discards forwarded client-IP headers, a footgun that has caused a
//! production attribution outage. Traffic that enters through Fly's *public*
//! edge rather than the private mesh appends a further hop from
//! [`proxies::FLY_PUBLIC_RANGES`], so that range must be trusted too or the
//! same misattribution returns. Local profiles trust loopback and RFC1918 so a
//! local reverse proxy works out of the box.
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
    nets.extend(parse_ranges(proxies::FLY_PUBLIC_RANGES));
    nets.extend(parse_ranges(proxies::CLOUDFLARE_RANGES));
    nets
}

#[must_use]
pub fn default_local_trusted_proxies() -> Vec<IpNet> {
    parse_ranges(proxies::PRIVATE_RANGES)
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
