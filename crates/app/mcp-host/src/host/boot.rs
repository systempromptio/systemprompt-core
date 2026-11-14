//! Booting a [`HostContext`] from a spawned server's environment.
//!
//! [`HostKernel::boot`] composes the handles; the host context then binds the
//! extension state and assembles notifications over itself, because both run
//! extension factories that take the finished context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{HostBootError, HostKernel, HostParts};

use super::HostContext;

impl HostContext {
    pub async fn from_process_env() -> Result<Self, HostBootError> {
        let (parts, kernel) = HostKernel::from_process_env().await?;
        Self::finish(parts, &kernel)
    }

    pub async fn boot(
        lookup: impl Fn(&str) -> Option<String> + Send + Sync,
    ) -> Result<Self, HostBootError> {
        let (parts, kernel) = HostKernel::boot(lookup).await?;
        Self::finish(parts, &kernel)
    }

    fn finish(parts: HostParts, kernel: &HostKernel) -> Result<Self, HostBootError> {
        let mut host = Self {
            notifications: kernel.notifications_without_seams(),
            extension_state: Dependencies::new(),
            provider_tokens: Arc::default(),
            parts,
        };
        host.extension_state = kernel.bind_extension_state(&host, host.parts.kernel_state())?;
        host.notifications = kernel.notifications(&host);
        Ok(host)
    }
}
