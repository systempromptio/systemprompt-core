//! `AuthProvider` chain: PAT, session, and mTLS credential sources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::auth::types::HelperOutput;
use crate::config;
use async_trait::async_trait;
use systemprompt_identifiers::SessionId;
use thiserror::Error;

pub mod mtls;
pub mod pat;
pub mod session;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("not configured")]
    NotConfigured,
    #[error("{provider}: {source}")]
    Failed {
        provider: &'static str,
        #[source]
        source: AuthFailedSource,
    },
}

#[derive(Debug, Error)]
pub enum AuthFailedSource {
    #[error(transparent)]
    Keystore(#[from] crate::auth::keystore::KeystoreError),
    #[error(transparent)]
    Loopback(#[from] crate::auth::loopback::LoopbackError),
    #[error(transparent)]
    Gateway(#[from] crate::gateway::GatewayError),
    #[error(transparent)]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl AuthFailedSource {
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        match self {
            Self::Keystore(_) | Self::Loopback(_) | Self::Custom(_) => true,
            Self::Gateway(g) => {
                use crate::gateway::GatewayError as G;
                matches!(
                    g,
                    G::PubkeyMissing
                        | G::UnsafePath(_)
                        | G::PubkeyDecode(_)
                        | G::ManifestDecode(_)
                        | G::WhoamiDecode(_)
                        | G::ProfileDecode(_)
                        | G::AuthDecode(_)
                        | G::Serialize(_)
                )
            },
        }
    }
}

// Why #[async_trait]: providers are dispatched as `dyn AuthProvider`.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    fn name(&self) -> &'static str;
    /// Minted JWT binds to `session_id` to match the `x-session-id` the bridge
    /// presents.
    async fn authenticate(&self, session_id: &SessionId) -> Result<HelperOutput, AuthError>;
}

#[derive(Debug, Clone, Copy)]
pub struct AuthProviderRegistration {
    pub factory: fn(&config::Config) -> Box<dyn AuthProvider>,
    pub priority: i32,
}

inventory::collect!(AuthProviderRegistration);

/// Register an [`AuthProvider`] into the credential chain.
///
/// `factory` builds the provider from config; a higher `priority` (default 0)
/// runs earlier, so a white-label crate can insert its own credential source
/// ahead of the built-ins without editing core.
#[macro_export]
macro_rules! register_auth_provider {
    ($factory:expr, priority = $p:expr $(,)?) => {
        ::inventory::submit! {
            $crate::auth::providers::AuthProviderRegistration { factory: $factory, priority: $p }
        }
    };
    ($factory:expr $(,)?) => {
        $crate::register_auth_provider!($factory, priority = 0);
    };
}

register_auth_provider!(|cfg| Box::new(mtls::MtlsProvider::new(cfg)), priority = 30);
register_auth_provider!(
    |cfg| Box::new(session::SessionProvider::new(cfg)),
    priority = 20
);
register_auth_provider!(|cfg| Box::new(pat::PatProvider::new(cfg)), priority = 10);
