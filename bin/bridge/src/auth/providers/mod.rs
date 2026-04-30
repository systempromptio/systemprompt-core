use crate::auth::types::HelperOutput;
use async_trait::async_trait;
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
}

impl AuthFailedSource {
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        match self {
            Self::Keystore(_) | Self::Loopback(_) => true,
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

#[async_trait]
pub trait AuthProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn authenticate(&self) -> Result<HelperOutput, AuthError>;
}
