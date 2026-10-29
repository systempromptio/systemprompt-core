//! What clearing the Claude Desktop policy may remove.
//!
//! A clear removes what the bridge wrote and nothing else. A policy is the
//! bridge's when its `inferenceGatewayApiKey` is this install's loopback
//! secret (compared by fingerprint): that value is minted per install and an
//! administrator's or MDM's policy never carries it. Anything else is managed
//! elsewhere and is reported, never deleted. Where the bridge's policy also
//! holds keys it does not write, only the bridge's keys go.
//!
//! A policy that routes Claude Desktop to a bridge loopback port but carries
//! another secret was written by a previous install of a bridge (a purge, a
//! reinstall, a second account). The fingerprint stays the only proof an
//! unattended clear acts on, so such a policy is reported as a stale bridge
//! policy and is replaced only when the user asks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use super::policy::WRITTEN_POLICY_KEYS;

/// One policy location as read back from the host.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservedPolicy {
    pub keys: BTreeSet<String>,
    pub api_key_fingerprint: Option<String>,
    pub base_url: Option<String>,
    pub profile_managed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClearPlan {
    NothingWritten,
    RemoveAll,
    RemoveKeys(Vec<&'static str>),
    StaleBridgePolicy,
    ManagedElsewhere,
}

impl ClearPlan {
    #[must_use]
    pub const fn writes(&self) -> bool {
        matches!(self, Self::RemoveAll | Self::RemoveKeys(_))
    }
}

#[must_use]
pub fn plan_clear(observed: Option<&ObservedPolicy>, ours: Option<&str>) -> ClearPlan {
    let Some(observed) = observed.filter(|o| !o.keys.is_empty()) else {
        return ClearPlan::NothingWritten;
    };
    let is_ours = matches!(
        (observed.api_key_fingerprint.as_deref(), ours),
        (Some(theirs), Some(ours)) if theirs == ours
    );
    if !is_ours {
        if !observed.profile_managed && observed.base_url.as_deref().is_some_and(is_bridge_loopback)
        {
            return ClearPlan::StaleBridgePolicy;
        }
        return ClearPlan::ManagedElsewhere;
    }
    let bridge: Vec<&'static str> = WRITTEN_POLICY_KEYS
        .iter()
        .copied()
        .filter(|k| observed.keys.contains(*k))
        .collect();
    if observed
        .keys
        .iter()
        .all(|k| WRITTEN_POLICY_KEYS.contains(&k.as_str()))
    {
        ClearPlan::RemoveAll
    } else {
        ClearPlan::RemoveKeys(bridge)
    }
}

#[must_use]
pub fn is_bridge_loopback(base_url: &str) -> bool {
    let Ok(url) = url::Url::parse(base_url.trim()) else {
        return false;
    };
    let loopback = match url.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    loopback
        && url.port().is_some_and(|port| {
            (crate::proxy::DEFAULT_PROXY_PORT..=crate::proxy::MAX_CANDIDATE_PORT).contains(&port)
        })
}
