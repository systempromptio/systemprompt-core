//! The host id the harness presents to an instance, and which manifest entries
//! target a host.
//!
//! Every rule takes the host id as a parameter, so the same code answers for
//! any host; the harness passes [`HOST_ID`] on every route, in hook bodies and
//! in consumer plans. A manifest applies nothing unless its `enabled_hosts`
//! names the host (absent means empty). A skill with no `hosts` list targets
//! every host; a rule targets only the hosts it names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const HOST_ID: &str = "harness";

pub const HOST_HEADER_VALUE: &str = HOST_ID;

#[must_use]
pub fn enables_host<S: AsRef<str>>(enabled_hosts: &[S], host: &str) -> bool {
    enabled_hosts.iter().any(|entry| entry.as_ref() == host)
}

#[must_use]
pub fn entry_targets_host<S: AsRef<str>>(hosts: &[S], host: &str) -> bool {
    hosts.is_empty() || hosts.iter().any(|entry| entry.as_ref() == host)
}

#[must_use]
pub fn rule_targets_host<S: AsRef<str>>(hosts: &[S], host: &str) -> bool {
    hosts.iter().any(|entry| entry.as_ref() == host)
}
