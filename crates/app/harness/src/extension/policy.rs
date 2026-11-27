//! Which extensions an instance enabled for this host, read from the signed
//! manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use harness_engine::extension::EnabledExtensions;
use harness_ids::ExtensionId;
use sp_protocol::host::{HOST_ID, enables_host};
use sp_protocol::wire::manifest::Manifest;

/// The enablement policy: the manifest's `host_extensions` entry for a host.
///
/// It fails closed. A manifest that does not enable the host, or has no
/// `host_extensions` entry for it, enables nothing (`None`), so only required
/// extensions run; an entry that is not a well-formed extension id enables
/// nothing and is logged.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtensionPolicy;

impl ExtensionPolicy {
    pub fn enabled(manifest: &Manifest) -> Option<EnabledExtensions> {
        Self::enabled_for(manifest, HOST_ID)
    }

    pub fn enabled_for(manifest: &Manifest, host: &str) -> Option<EnabledExtensions> {
        if !enables_host(&manifest.enabled_hosts, host) {
            return None;
        }
        let listed = manifest.extensions_for(host)?;
        Some(
            listed
                .iter()
                .filter_map(|raw| match ExtensionId::parse(raw) {
                    Ok(id) => Some(id),
                    Err(error) => {
                        tracing::warn!(host, error = %error, "manifest names a malformed extension id; not enabled");
                        None
                    },
                })
                .collect(),
        )
    }
}
