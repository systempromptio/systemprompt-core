//! How a refusal or failure of the MCP backends becomes a proxy error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::router::ProxyBackendError;
use systemprompt_identifiers::ServiceName;

use super::errors::ProxyError;

impl ProxyError {
    pub fn from_backend(service: &ServiceName, error: ProxyBackendError) -> Self {
        let service = service.to_string();
        match error {
            ProxyBackendError::AuthenticationRequired => Self::AuthenticationRequired { service },
            ProxyBackendError::ProviderNotConnected => Self::ProviderNotConnected { service },
            ProxyBackendError::Unavailable { status } => {
                Self::ServiceNotRunning { service, status }
            },
            ProxyBackendError::ResolveFailed(source) => {
                Self::ExternalResolveFailed { service, source }
            },
            ProxyBackendError::Forbidden => Self::Forbidden { service },
            ProxyBackendError::RegistryUnavailable(source) => {
                Self::RegistryUnavailable { service, source }
            },
            ProxyBackendError::SessionLookupFailed(source) => {
                Self::SessionLookupFailed { service, source }
            },
        }
    }
}
