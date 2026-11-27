//! The host fields of the signed manifest: the hosts a user's manifest
//! enables, the protocol filters and extensions it carries for them, and the
//! hosts it names when the bridge floor is computed.
//!
//! Every projection is over the hosts the instance serves. A host the
//! instance withholds is absent from all three fields whatever the user's
//! stored preferences say, so a stale preference row never names a host an
//! older bridge cannot parse.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_manifest::services::ServicesConfig;
use systemprompt_models::bridge::host::{HostKind, InstanceHosts};
use systemprompt_models::bridge::ids::HostExtensionId;
use systemprompt_oauth::repository::EnabledHostPrefs;

#[must_use]
pub fn user_enabled_hosts(prefs: &EnabledHostPrefs, instance: &InstanceHosts) -> Vec<HostKind> {
    instance
        .enabled
        .iter()
        .copied()
        .filter(|host| prefs.admits(*host, instance))
        .collect()
}

#[must_use]
pub fn host_model_protocols_for(
    rows: Vec<(HostKind, Vec<String>)>,
    instance: &InstanceHosts,
) -> BTreeMap<HostKind, Vec<String>> {
    rows.into_iter()
        .filter(|(host, _)| instance.enabled.contains(host))
        .collect()
}

#[must_use]
pub fn host_extensions_for(
    services: &ServicesConfig,
    enabled_hosts: &[HostKind],
) -> BTreeMap<HostKind, Vec<HostExtensionId>> {
    services
        .instance_host_extensions()
        .into_iter()
        .filter(|(host, _)| enabled_hosts.contains(host))
        .collect()
}

pub fn named<'a>(
    enabled_hosts: &'a [HostKind],
    host_model_protocols: &'a BTreeMap<HostKind, Vec<String>>,
    host_extensions: &'a BTreeMap<HostKind, Vec<HostExtensionId>>,
) -> impl Iterator<Item = HostKind> + 'a {
    enabled_hosts
        .iter()
        .chain(host_model_protocols.keys())
        .chain(host_extensions.keys())
        .copied()
}
