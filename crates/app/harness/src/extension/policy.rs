//! Which extensions an instance enabled for this host, read from the signed
//! manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_device::protocol::targets::enables_host;
use systemprompt_engine::run::extension::EnabledExtensions;
use systemprompt_identifiers::engine::ExtensionId;
use systemprompt_models::bridge::host::HostKind;

/// The enablement policy: the manifest's `host_extensions` entry for a host.
///
/// It fails closed. A manifest that does not enable the host, or has no
/// `host_extensions` entry for it, enables nothing (`None`), so only required
/// extensions run.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtensionPolicy;

impl ExtensionPolicy {
    pub fn enabled(manifest: &Manifest) -> Option<EnabledExtensions> {
        Self::enabled_for(manifest, HostKind::Harness)
    }

    pub fn enabled_for(manifest: &Manifest, host: HostKind) -> Option<EnabledExtensions> {
        if !enables_host(&manifest.enabled_hosts, host) {
            return None;
        }
        let listed = manifest.extensions_for(host)?;
        listed
            .iter()
            .map(|raw| ExtensionId::parse(raw.as_str()).ok())
            .collect()
    }
}
