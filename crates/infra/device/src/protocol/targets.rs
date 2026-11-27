//! The host the device presents to an instance, and which manifest entries
//! target a host.
//!
//! Every rule takes the host as a parameter, so the same code answers for any
//! host; the device passes `HostKind::Harness` on every route, in hook bodies
//! and in consumer plans. A manifest applies nothing unless its `enabled_hosts`
//! names the host (absent means empty). A skill with no `hosts` list targets
//! every host; a rule targets only the hosts it names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::HostKind;

#[must_use]
pub fn enables_host(enabled_hosts: &[HostKind], host: HostKind) -> bool {
    enabled_hosts.contains(&host)
}

#[must_use]
pub fn entry_targets_host<S: AsRef<str>>(hosts: &[S], host: HostKind) -> bool {
    hosts.is_empty() || rule_targets_host(hosts, host)
}

#[must_use]
pub fn rule_targets_host<S: AsRef<str>>(hosts: &[S], host: HostKind) -> bool {
    hosts.iter().any(|entry| entry.as_ref() == host.as_str())
}
