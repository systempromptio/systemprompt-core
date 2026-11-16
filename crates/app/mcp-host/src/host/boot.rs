//! Booting a [`HostContext`] from a spawned server's environment.
//!
//! [`HostKernel::boot`] admits the composition against the child's profile,
//! resolves its kernel and composes the handles; the host context reads the
//! resolved kernel state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_runtime::{AdmitFn, HostBootError, HostKernel};

use super::HostContext;

impl HostContext {
    pub async fn from_process_env(admit: AdmitFn) -> Result<Self, HostBootError> {
        Ok(Self::over(HostKernel::from_process_env(admit).await?))
    }

    pub async fn boot(
        lookup: impl Fn(&str) -> Option<String> + Send + Sync,
        admit: AdmitFn,
    ) -> Result<Self, HostBootError> {
        Ok(Self::over(HostKernel::boot(lookup, admit).await?))
    }

    fn over(parts: systemprompt_runtime::HostParts) -> Self {
        Self {
            parts,
            provider_tokens: Arc::default(),
        }
    }
}
